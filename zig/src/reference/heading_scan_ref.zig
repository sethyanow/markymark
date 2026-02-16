const std = @import("std");

/// Result of scanning a heading in markdown text.
/// Matches C ABI layout per brza-markymark.md Section 4.2.
pub const HeadingScan = extern struct {
    /// Byte offset of the heading text start (after "# ")
    offset: u32,
    /// Length of the heading text in bytes
    length: u16,
    /// Heading level (1-6)
    level: u8,
    /// Padding for 8-byte alignment
    _padding: u8 = 0,
};

/// Scalar (byte-by-byte) heading scanner. Serves as the correctness reference
/// for verifying the SIMD implementation.
///
/// Scans `text[0..len]` for ATX headings: `#` at line start followed by a space.
/// Writes results into `out[0..cap]`. Returns the number of headings found,
/// or `cap` if the buffer filled (caller should check `written < cap` to know
/// if there may be more).
pub fn scan_headings_scalar(
    text: [*]const u8,
    len: u32,
    out: [*]HeadingScan,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var pos: u32 = 0;

    // First line starts at position 0 — treat as "at line start"
    if (buf[0] == '#') {
        if (try_parse_heading(buf, 0, len)) |h| {
            out[written] = h;
            written += 1;
            if (written >= cap) return written;
        }
    }

    // Scan for newlines, check the byte after each for '#'
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '\n') {
            const next = pos + 1;
            if (next < len and buf[next] == '#') {
                if (try_parse_heading(buf, next, len)) |h| {
                    out[written] = h;
                    written += 1;
                    if (written >= cap) return written;
                }
            }
        }
    }

    return written;
}

/// Try to parse a heading starting at `start` in `buf[0..len]`.
/// Returns null if this isn't a valid heading (level > 6, no space after #s, etc).
pub fn try_parse_heading(buf: []const u8, start: u32, len: u32) ?HeadingScan {
    var pos = start;

    // Count consecutive '#' characters
    var level: u8 = 0;
    while (pos < len and buf[pos] == '#' and level < 7) {
        level += 1;
        pos += 1;
    }

    // Level must be 1-6
    if (level == 0 or level > 6) return null;

    // Must be followed by a space (or end of line for empty heading)
    if (pos >= len or buf[pos] == '\n') {
        // Empty heading (e.g., "##\n") — valid per CommonMark
        return HeadingScan{
            .offset = pos,
            .length = 0,
            .level = level,
        };
    }

    if (buf[pos] != ' ') return null;

    // Skip the space
    pos += 1;
    const text_start = pos;

    // Find end of line
    while (pos < len and buf[pos] != '\n') {
        pos += 1;
    }

    // text_start..pos is the raw heading text (may include ATX closing hashes)
    var text_end = pos;

    // Trim ATX closing hashes: trailing whitespace, then trailing '#', then trailing whitespace
    text_end = trim_right(buf, text_start, text_end, ' ');
    const after_space_trim = text_end;
    text_end = trim_right(buf, text_start, text_end, '#');
    // Only trim if we actually removed some '#' and there's a space before them
    // (or the entire text was hashes). Per CommonMark, closing sequence must be
    // preceded by a space (or be the entire content).
    if (text_end < after_space_trim) {
        // We did remove hashes. Check if what remains ends with space or is empty.
        if (text_end > text_start and buf[text_end - 1] != ' ') {
            // The '#' was part of the heading text, not a closing sequence
            text_end = after_space_trim;
        } else {
            // Valid closing sequence — trim the trailing space too
            text_end = trim_right(buf, text_start, text_end, ' ');
        }
    }

    const text_len = text_end - text_start;
    const clamped: u16 = if (text_len > std.math.maxInt(u16)) std.math.maxInt(u16) else @intCast(text_len);

    return HeadingScan{
        .offset = text_start,
        .length = clamped,
        .level = level,
    };
}

/// Trim trailing occurrences of `ch` from buf[start..end], returning new end.
fn trim_right(buf: []const u8, start: u32, end: u32, ch: u8) u32 {
    var e = end;
    while (e > start and buf[e - 1] == ch) {
        e -= 1;
    }
    return e;
}

// ============================================================================
// Tests
// ============================================================================

test "scalar: single h1" {
    const text = "# Hello\n";
    var out: [8]HeadingScan = undefined;
    const n = scan_headings_scalar(text.ptr, text.len, &out, 8);
    try std.testing.expectEqual(@as(u32, 1), n);
    try std.testing.expectEqual(@as(u32, 2), out[0].offset); // "Hello" starts at byte 2
    try std.testing.expectEqual(@as(u16, 5), out[0].length);
    try std.testing.expectEqual(@as(u8, 1), out[0].level);
}

test "scalar: all levels" {
    const text = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n";
    var out: [8]HeadingScan = undefined;
    const n = scan_headings_scalar(text.ptr, text.len, &out, 8);
    try std.testing.expectEqual(@as(u32, 6), n);
    var i: u8 = 0;
    while (i < 6) : (i += 1) {
        try std.testing.expectEqual(i + 1, out[i].level);
    }
}

test "scalar: atx closing hashes" {
    const text = "## Heading ##\n";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), n);
    try std.testing.expectEqual(@as(u16, 7), out[0].length); // "Heading"
    try std.testing.expectEqual(@as(u8, 2), out[0].level);
}

test "scalar: empty input" {
    const text = "";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings_scalar(text.ptr, 0, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), n);
}
