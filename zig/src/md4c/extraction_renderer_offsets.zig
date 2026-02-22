// Offset recovery functions for ExtractionRenderer.
//
// Pure source-scanning helpers that recover byte offsets for extracted elements.
// Each function takes source text and a cursor pointer, scanning forward to find
// the markdown syntax for the corresponding element type.
//
// Extracted from extraction_renderer.zig (marky-7kmo) for module size management.

/// Find the offset of the next code span (backtick-delimited) in the source.
/// Scans for a matching backtick run pair (1, 2, or 3 backticks).
/// Advances `cursor` past the closing backticks.
pub fn findCodeSpanOffset(src: []const u8, cursor: *u32) u32 {
    var pos: u32 = cursor.*;

    // Find the opening backtick run
    while (pos < src.len) {
        if (src[pos] == '`') {
            const open_start = pos;
            // Count opening backtick run length
            var open_len: u32 = 0;
            while (pos < src.len and src[pos] == '`') : (pos += 1) {
                open_len += 1;
            }
            // Scan for closing backtick run of exactly the same length
            while (pos < src.len) {
                if (src[pos] == '`') {
                    var close_len: u32 = 0;
                    while (pos < src.len and src[pos] == '`') : (pos += 1) {
                        close_len += 1;
                    }
                    if (close_len == open_len) {
                        // Found matching closing backticks
                        cursor.* = pos;
                        return open_start;
                    }
                    // Not matching — continue scanning (pos already advanced past these backticks)
                } else {
                    pos += 1;
                }
            }
            // No matching close found — return opening position, advance cursor
            cursor.* = pos;
            return open_start;
        }
        pos += 1;
    }

    // Fallback: no backtick found
    return cursor.*;
}

/// Scan forward from cursor to find the '[' of a task checkbox [x].
/// Advances cursor past the checkbox and task text.
pub fn findTaskOffset(src: []const u8, cursor: *u32) u32 {
    var pos: u32 = cursor.*;
    while (pos + 2 < src.len) {
        if (src[pos] == '[' and src[pos + 2] == ']') {
            // Advance cursor past the checkbox marker "[ ] " or "[x] "
            cursor.* = @intCast(@min(@as(u64, pos) + 4, src.len));
            return pos;
        }
        pos += 1;
    }
    return cursor.*;
}

/// Find the offset of the next heading in the source.
/// For setext headings, scans for a line followed by === or ---.
/// For ATX headings, scans for # at line start with matching level.
/// Tracks fenced code blocks to avoid false matches inside them.
/// Advances `cursor` past the heading.
pub fn findHeadingOffset(src: []const u8, cursor: *u32, is_setext: bool, level: u8) u32 {
    var pos: u32 = cursor.*;

    if (is_setext) {
        // Setext: find a line followed by === (level 1) or --- (level 2).
        // Return offset of the text line start.
        // Track fenced code blocks to avoid matching underlines inside them.
        var in_fence_s = false;
        var fence_char_s: u8 = 0;
        var fence_len_s: u32 = 0;
        while (pos < src.len) {
            // Find the start of a text line
            const line_start = pos;
            // Skip to end of this line
            while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
            const line_end = pos;
            // Skip newline
            if (pos < src.len) pos += 1;

            // Detect fence open/close at this line
            {
                var fp = line_start;
                var sp: u32 = 0;
                while (fp < line_end and src[fp] == ' ' and sp < 3) {
                    fp += 1;
                    sp += 1;
                }
                if (fp < line_end and (src[fp] == '`' or src[fp] == '~')) {
                    const fc = src[fp];
                    var flen: u32 = 0;
                    while (fp + flen < line_end and src[fp + flen] == fc) : (flen += 1) {}
                    if (flen >= 3) {
                        if (!in_fence_s) {
                            in_fence_s = true;
                            fence_char_s = fc;
                            fence_len_s = flen;
                        } else if (fc == fence_char_s and flen >= fence_len_s) {
                            in_fence_s = false;
                            fence_char_s = 0;
                            fence_len_s = 0;
                        }
                        continue; // skip fence lines
                    }
                }
            }

            // If inside a fence, skip this line entirely
            if (in_fence_s) continue;

            // Check if NEXT line is the underline
            if (pos < src.len) {
                const underline_char: u8 = if (level == 1) '=' else '-';
                var underline_start = pos;
                // Skip optional leading spaces (up to 3)
                var leading_spaces: u32 = 0;
                while (underline_start < src.len and src[underline_start] == ' ' and leading_spaces < 3) {
                    underline_start += 1;
                    leading_spaces += 1;
                }
                if (underline_start < src.len and src[underline_start] == underline_char) {
                    var underline_end = underline_start;
                    while (underline_end < src.len and src[underline_end] == underline_char) : (underline_end += 1) {}
                    // Must have at least 1 underline char and rest of line is blank
                    if (underline_end > underline_start) {
                        var trailing = underline_end;
                        while (trailing < src.len and (src[trailing] == ' ' or src[trailing] == '\t')) : (trailing += 1) {}
                        if (trailing >= src.len or src[trailing] == '\n' or src[trailing] == '\r') {
                            // Only if the text line is non-empty
                            if (line_end > line_start) {
                                // Advance cursor past the underline
                                cursor.* = @intCast(@min(trailing + 1, src.len));
                                return @intCast(line_start);
                            }
                        }
                    }
                }
            }
        }
    } else {
        // ATX: '#' must appear at line start (0-3 leading spaces + optional '>' blockquote).
        // Scan line-by-line; track code fences to skip false '#' matches inside fenced blocks.
        var in_fence = false;
        var fence_char: u8 = 0;
        var fence_len: u32 = 0;
        while (pos < src.len) {
            const line_start = pos;
            var line_end = pos;
            while (line_end < src.len and src[line_end] != '\n') : (line_end += 1) {}
            const next_line: u32 = @intCast(if (line_end < src.len) line_end + 1 else src.len);

            // Detect fence open/close: 0-3 spaces then 3+ identical backticks or tildes.
            var fp = pos;
            var sp: u32 = 0;
            while (fp < line_end and src[fp] == ' ' and sp < 3) {
                fp += 1;
                sp += 1;
            }
            if (fp < line_end and (src[fp] == '`' or src[fp] == '~')) {
                const fc = src[fp];
                var flen: u32 = 0;
                while (fp + flen < line_end and src[fp + flen] == fc) : (flen += 1) {}
                if (flen >= 3) {
                    if (!in_fence) {
                        in_fence = true;
                        fence_char = fc;
                        fence_len = flen;
                    } else if (fc == fence_char and flen >= fence_len) {
                        in_fence = false;
                        fence_char = 0;
                        fence_len = 0;
                    }
                    // else: different char or shorter fence — stay in fence
                    pos = next_line;
                    continue;
                }
            }

            if (!in_fence) {
                // Check for 0-3 leading spaces, optional '>' blockquote prefix, then '#'.
                var lp = line_start;
                var lsp: u32 = 0;
                while (lp < line_end and src[lp] == ' ' and lsp < 3) {
                    lp += 1;
                    lsp += 1;
                }
                while (lp < line_end and src[lp] == '>') {
                    lp += 1;
                    if (lp < line_end and src[lp] == ' ') lp += 1;
                }
                if (lp < line_end and src[lp] == '#') {
                    const hash_start = lp;
                    var hash_count: u8 = 0;
                    var p = lp;
                    while (p < line_end and src[p] == '#') : (p += 1) {
                        hash_count += 1;
                    }
                    if (hash_count == level and
                        (p >= line_end or src[p] == ' ' or src[p] == '\t'))
                    {
                        cursor.* = next_line;
                        return @intCast(hash_start);
                    }
                }
            }

            pos = next_line;
        }
    }

    // Fallback: use current cursor
    return cursor.*;
}

/// Find the offset of the next link in the source.
/// Handles wiki links ([[...]]), autolinks (<...>), and standard links ([...](...)
/// or [...][...]). Tracks fenced code blocks for standard links.
/// Advances `cursor` past the link.
pub fn findLinkOffset(src: []const u8, cursor: *u32, is_wiki: bool, is_autolink: bool) u32 {
    var pos: u32 = cursor.*;

    if (is_wiki) {
        // Search for '[['
        while (pos + 1 < src.len) {
            if (src[pos] == '[' and src[pos + 1] == '[') {
                // Advance cursor past the wiki link ]]
                var end = pos + 2;
                while (end + 1 < src.len) {
                    if (src[end] == ']' and src[end + 1] == ']') {
                        end += 2;
                        break;
                    }
                    end += 1;
                }
                cursor.* = @intCast(end);
                return @intCast(pos);
            }
            pos += 1;
        }
    } else if (is_autolink) {
        // Search for '<'
        while (pos < src.len) {
            if (src[pos] == '<') {
                var end = pos + 1;
                while (end < src.len and src[end] != '>') : (end += 1) {}
                if (end < src.len) end += 1; // past '>'
                cursor.* = @intCast(end);
                return @intCast(pos);
            }
            pos += 1;
        }
    } else {
        // Search for '[' (inline or reference link), skipping fenced code blocks.
        var in_fence = false;
        var fence_char: u8 = 0;
        var fence_len: u32 = 0;
        while (pos < src.len) {
            // Detect fence at line start: 0-3 spaces then 3+ backticks or tildes.
            if (pos == 0 or src[pos - 1] == '\n') {
                var fp = pos;
                var sp: u32 = 0;
                while (fp < src.len and src[fp] == ' ' and sp < 3) {
                    fp += 1;
                    sp += 1;
                }
                if (fp < src.len and (src[fp] == '`' or src[fp] == '~')) {
                    const fc = src[fp];
                    var flen: u32 = 0;
                    while (fp + flen < src.len and src[fp + flen] == fc) : (flen += 1) {}
                    if (flen >= 3) {
                        if (!in_fence) {
                            in_fence = true;
                            fence_char = fc;
                            fence_len = flen;
                        } else if (fc == fence_char and flen >= fence_len) {
                            in_fence = false;
                            fence_char = 0;
                            fence_len = 0;
                        }
                        // else: different char or shorter fence — stay in fence
                        while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
                        if (pos < src.len) pos += 1;
                        continue;
                    }
                }
            }
            if (!in_fence and src[pos] == '[') {
                // Skip image links — they start with ![ and are tracked by in_image
                if (pos > 0 and src[pos - 1] == '!') {
                    pos += 1;
                    continue;
                }
                // Advance cursor past the closing ) or ]
                var end = pos + 1;
                var bracket_depth: u32 = 1;
                while (end < src.len and bracket_depth > 0) {
                    if (src[end] == '[') bracket_depth += 1;
                    if (src[end] == ']') bracket_depth -= 1;
                    end += 1;
                }
                // Skip past (url) if present, tracking paren depth for URLs like
                // https://en.wikipedia.org/wiki/Foo_(bar) and handling backslash escapes.
                if (end < src.len and src[end] == '(') {
                    end += 1;
                    var paren_depth: u32 = 1;
                    while (end < src.len and paren_depth > 0) {
                        if (src[end] == '\\' and end + 1 < src.len) {
                            end += 2; // skip escaped character
                            continue;
                        }
                        if (src[end] == '(') paren_depth += 1;
                        if (src[end] == ')') paren_depth -= 1;
                        if (paren_depth > 0) end += 1;
                    }
                    if (paren_depth == 0) end += 1; // skip final ')'
                } else if (end < src.len and src[end] == '[') {
                    // Reference link [text][ref]
                    end += 1;
                    while (end < src.len and src[end] != ']') : (end += 1) {}
                    if (end < src.len) end += 1;
                }
                cursor.* = @intCast(end);
                return @intCast(pos);
            }
            pos += 1;
        }
    }

    // Fallback
    return cursor.*;
}
