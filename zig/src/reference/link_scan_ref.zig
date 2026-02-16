const std = @import("std");

/// Result of scanning a link in markdown text.
/// Matches C ABI layout per brza-markymark.md Section 4.2.
pub const LinkScan = extern struct {
    /// Byte offset of link start (the '[' or first '[' of '[[')
    offset: u32,
    /// Byte offset of link text content
    text_offset: u32,
    /// Length of link text in bytes
    text_length: u16,
    /// Byte offset of link target
    target_offset: u32,
    /// Length of link target in bytes
    target_length: u16,
    /// 0 = markdown [text](url), 1 = wiki-link [[target]]
    link_type: u8,
    /// Padding for alignment
    _padding: u8 = 0,
};

/// Scalar (byte-by-byte) link scanner. Serves as the correctness reference
/// for verifying the SIMD implementation.
///
/// Scans `text[0..len]` for markdown links `[text](url)` and wiki-links
/// `[[target]]` or `[[target|display]]`. Writes results into `out[0..cap]`.
/// Returns the number of links found.
pub fn scan_links_scalar(
    text: [*]const u8,
    len: u32,
    out: [*]LinkScan,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var pos: u32 = 0;

    while (pos < len) {
        if (buf[pos] == '[' and !is_escaped(buf, pos)) {
            // Check for wiki-link: [[
            if (pos + 1 < len and buf[pos + 1] == '[') {
                if (try_parse_wiki_link(buf, pos, len)) |link| {
                    out[written] = link;
                    written += 1;
                    if (written >= cap) return written;
                    // Skip past the closing ]]
                    pos = link.target_offset + link.target_length;
                    // Skip past ']]' or '|display]]'
                    if (link.link_type == 1) {
                        // Find the closing ]]
                        while (pos < len -| 1) {
                            if (buf[pos] == ']' and buf[pos + 1] == ']') {
                                pos += 2;
                                break;
                            }
                            pos += 1;
                        }
                    }
                    continue;
                }
            }

            // Check for image link: ![alt](url) — detect as a link
            // Back up to check for preceding '!'
            const is_image = pos > 0 and buf[pos - 1] == '!';

            if (try_parse_markdown_link(buf, pos, len)) |link| {
                var result = link;
                if (is_image) {
                    result.offset = pos - 1; // Include the '!' in offset
                }
                out[written] = result;
                written += 1;
                if (written >= cap) return written;
                // Skip past the closing ')'
                pos = link.target_offset + link.target_length + 1;
                continue;
            }
        }
        pos += 1;
    }

    return written;
}

/// Check if character at `pos` is escaped by a preceding backslash.
pub fn is_escaped(buf: []const u8, pos: u32) bool {
    if (pos == 0) return false;
    // Count consecutive backslashes before pos
    var count: u32 = 0;
    var i = pos;
    while (i > 0) {
        i -= 1;
        if (buf[i] == '\\') {
            count += 1;
        } else {
            break;
        }
    }
    // Odd number of backslashes means the char is escaped
    return count % 2 == 1;
}

/// Try to parse a markdown link [text](url) starting at the '[' at position `start`.
/// Returns null if not a valid markdown link.
pub fn try_parse_markdown_link(buf: []const u8, start: u32, len: u32) ?LinkScan {
    if (start >= len or buf[start] != '[') return null;

    // Find matching ']' with bracket depth tracking
    const text_start = start + 1;
    var depth: i32 = 1;
    var pos = text_start;

    while (pos < len and depth > 0) {
        if (buf[pos] == '\\' and pos + 1 < len) {
            pos += 2; // Skip escaped character
            continue;
        }
        if (buf[pos] == '[') {
            depth += 1;
        } else if (buf[pos] == ']') {
            depth -= 1;
        }
        if (depth > 0) pos += 1;
    }

    if (depth != 0) return null; // Unmatched bracket

    const text_end = pos;
    pos += 1; // Skip ']'

    // Must be immediately followed by '('
    if (pos >= len or buf[pos] != '(') return null;
    pos += 1; // Skip '('

    const target_start = pos;

    // Find matching ')' with paren depth tracking
    var paren_depth: i32 = 1;
    while (pos < len and paren_depth > 0) {
        if (buf[pos] == '\\' and pos + 1 < len) {
            pos += 2; // Skip escaped character
            continue;
        }
        if (buf[pos] == '(') {
            paren_depth += 1;
        } else if (buf[pos] == ')') {
            paren_depth -= 1;
        }
        if (paren_depth > 0) pos += 1;
    }

    if (paren_depth != 0) return null; // Unmatched paren

    const target_end = pos;

    const text_len = text_end - text_start;
    const target_len = target_end - target_start;

    return LinkScan{
        .offset = start,
        .text_offset = text_start,
        .text_length = clamp_u16(text_len),
        .target_offset = target_start,
        .target_length = clamp_u16(target_len),
        .link_type = 0, // markdown link
    };
}

/// Try to parse a wiki-link [[target]] or [[target|display]] starting at
/// the first '[' at position `start`.
/// Returns null if not a valid wiki-link.
pub fn try_parse_wiki_link(buf: []const u8, start: u32, len: u32) ?LinkScan {
    if (start + 2 >= len or buf[start] != '[' or buf[start + 1] != '[') return null;

    const content_start = start + 2;
    var pos = content_start;

    // Find closing ]]
    while (pos < len -| 1) {
        if (buf[pos] == ']' and buf[pos + 1] == ']') {
            break;
        }
        // Wiki-links don't span newlines typically, but we allow it per spec
        pos += 1;
    }

    if (pos >= len -| 1 or buf[pos] != ']') return null; // No closing ]]

    const content_end = pos;

    // Check for pipe separator: [[target|display]]
    var pipe_pos: ?u32 = null;
    var scan = content_start;
    while (scan < content_end) {
        if (buf[scan] == '|') {
            pipe_pos = scan;
            break;
        }
        scan += 1;
    }

    if (pipe_pos) |pipe| {
        // [[target|display text]]
        const target_len = pipe - content_start;
        const display_start = pipe + 1;
        const display_len = content_end - display_start;

        return LinkScan{
            .offset = start,
            .text_offset = display_start,
            .text_length = clamp_u16(display_len),
            .target_offset = content_start,
            .target_length = clamp_u16(target_len),
            .link_type = 1, // wiki-link
        };
    } else {
        // [[target]] — target and display text are the same
        const target_len = content_end - content_start;

        return LinkScan{
            .offset = start,
            .text_offset = content_start,
            .text_length = clamp_u16(target_len),
            .target_offset = content_start,
            .target_length = clamp_u16(target_len),
            .link_type = 1, // wiki-link
        };
    }
}

/// Clamp a u32 value to u16 range.
fn clamp_u16(val: u32) u16 {
    if (val > std.math.maxInt(u16)) return std.math.maxInt(u16);
    return @intCast(val);
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "scalar: single markdown link" {
    const text = "[hello](https://example.com)";
    var out: [8]LinkScan = undefined;
    const n = scan_links_scalar(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].offset); // '[' at 0
    try testing.expectEqual(@as(u32, 1), out[0].text_offset); // "hello" at 1
    try testing.expectEqual(@as(u16, 5), out[0].text_length); // "hello"
    try testing.expectEqual(@as(u32, 8), out[0].target_offset); // url starts at 8
    try testing.expectEqual(@as(u16, 19), out[0].target_length); // "https://example.com"
    try testing.expectEqual(@as(u8, 0), out[0].link_type);
}

test "scalar: single wiki-link" {
    const text = "[[my page]]";
    var out: [8]LinkScan = undefined;
    const n = scan_links_scalar(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].offset); // first '[' at 0
    try testing.expectEqual(@as(u32, 2), out[0].text_offset); // "my page" at 2
    try testing.expectEqual(@as(u16, 7), out[0].text_length); // "my page"
    try testing.expectEqual(@as(u32, 2), out[0].target_offset); // same as text
    try testing.expectEqual(@as(u16, 7), out[0].target_length);
    try testing.expectEqual(@as(u8, 1), out[0].link_type);
}

test "scalar: wiki-link with pipe" {
    const text = "[[target page|display text]]";
    var out: [8]LinkScan = undefined;
    const n = scan_links_scalar(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].offset);
    try testing.expectEqual(@as(u32, 2), out[0].target_offset); // "target page" at 2
    try testing.expectEqual(@as(u16, 11), out[0].target_length); // "target page"
    try testing.expectEqual(@as(u32, 14), out[0].text_offset); // "display text" at 14
    try testing.expectEqual(@as(u16, 12), out[0].text_length); // "display text"
    try testing.expectEqual(@as(u8, 1), out[0].link_type);
}

test "scalar: empty input" {
    const text = "";
    var out: [4]LinkScan = undefined;
    const n = scan_links_scalar(text.ptr, 0, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}
