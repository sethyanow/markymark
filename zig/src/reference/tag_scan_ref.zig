const std = @import("std");

/// Result of scanning a tag in markdown text.
/// Matches C ABI layout per brza-markymark.md Section 4.2.
pub const TagScan = extern struct {
    /// Byte offset of the '#' character
    offset: u32,
    /// Tag name length (without the leading '#')
    length: u16,
    /// Padding for 8-byte alignment
    _padding: [2]u8 = .{ 0, 0 },
};

/// Returns true if `ch` is a valid tag name character: [a-zA-Z0-9_-]
fn is_tag_char(ch: u8) bool {
    return switch (ch) {
        'a'...'z', 'A'...'Z', '0'...'9', '_', '-' => true,
        else => false,
    };
}

/// Returns true if `ch` is a whitespace character (space, tab, or newline variants).
fn is_whitespace(ch: u8) bool {
    return switch (ch) {
        ' ', '\t', '\n', '\r' => true,
        else => false,
    };
}

/// Try to parse a tag at position `pos` in `buf[0..len]`.
/// `pos` must point to a '#' character.
/// Returns a TagScan if a valid tag is found, null otherwise.
///
/// Tag rules:
/// - Must be preceded by whitespace or be at start of line (pos == 0 or after \n)
/// - '#' must be followed by at least one tag name character (not space — that's a heading)
/// - Tag name is [a-zA-Z0-9_-]+
pub fn try_parse_tag(buf: []const u8, pos: u32, len: u32) ?TagScan {
    _ = len;

    // Check boundary: '#' must be at start or preceded by whitespace
    if (pos > 0) {
        const prev = buf[pos - 1];
        if (!is_whitespace(prev)) return null;
    }

    // Must have at least one char after '#'
    const name_start = pos + 1;
    if (name_start >= buf.len) return null;

    // First char after '#' must be a tag char (not space — space means heading)
    if (!is_tag_char(buf[name_start])) return null;

    // Consume tag name characters
    var end = name_start + 1;
    while (end < buf.len and is_tag_char(buf[end])) : (end += 1) {}

    const name_len = end - name_start;
    return TagScan{
        .offset = pos,
        .length = @intCast(name_len),
    };
}

/// Scalar (byte-by-byte) tag scanner. Serves as the correctness reference
/// for verifying the SIMD implementation.
///
/// Scans `text[0..len]` for #tag patterns.
/// Writes results into `out[0..cap]`. Returns the number of tags found.
pub fn scan_tags_scalar(
    text: [*]const u8,
    len: u32,
    out: [*]TagScan,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var pos: u32 = 0;

    while (pos < len) : (pos += 1) {
        if (buf[pos] == '#') {
            if (try_parse_tag(buf, pos, len)) |tag| {
                out[written] = tag;
                written += 1;
                if (written >= cap) return written;
                // Skip past the tag to avoid re-scanning
                pos += 1 + tag.length;
                // The loop will increment pos again, so subtract 1
                pos -= 1;
            }
        }
    }

    return written;
}

// ============================================================================
// Tests
// ============================================================================

test "scalar: tag at line start" {
    const text = "#hello";
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 0), out[0].offset);
    try std.testing.expectEqual(@as(u16, 5), out[0].length);
}

test "scalar: tag after space" {
    const text = "text #tag rest";
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
    try std.testing.expectEqual(@as(u16, 3), out[0].length);
}

test "scalar: tag in mid-word rejected" {
    const text = "word#tag";
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "scalar: heading not a tag" {
    const text = "# heading";
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "scalar: tag with hyphen and underscore" {
    const text = "#my-tag_2";
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 8), out[0].length);
}

test "scalar: multiple tags on one line" {
    const text = "#tag1 #tag2 #tag3";
    var out: [8]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 8);
    try std.testing.expectEqual(@as(u32, 3), w);
    try std.testing.expectEqual(@as(u32, 0), out[0].offset);
    try std.testing.expectEqual(@as(u32, 6), out[1].offset);
    try std.testing.expectEqual(@as(u32, 12), out[2].offset);
}

test "scalar: tag after newline" {
    const text = "line one\n#tag";
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 9), out[0].offset);
}

test "scalar: empty input" {
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar("".ptr, 0, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "scalar: only digits tag" {
    const text = "#123";
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].length);
}

test "scalar: tag after tab" {
    const text = "text\t#tab-tag";
    var out: [4]TagScan = undefined;
    const w = scan_tags_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
}
