const std = @import("std");

/// Result of scanning a block ID in markdown text.
/// Matches C ABI layout per brza-markymark.md Section 4.2.
pub const BlockIdScan = extern struct {
    /// Byte offset of the '^' character
    offset: u32,
    /// Block ID length (without the leading '^')
    length: u16,
    /// Padding for 8-byte alignment
    _padding: [2]u8 = .{ 0, 0 },
};

/// Returns true if `ch` is a valid block ID character: [a-zA-Z0-9-]
fn is_block_id_char(ch: u8) bool {
    return switch (ch) {
        'a'...'z', 'A'...'Z', '0'...'9', '-' => true,
        else => false,
    };
}

/// Try to parse a block ID at position `pos` in `buf[0..len]`.
/// `pos` must point to a '^' character.
/// Returns a BlockIdScan if a valid block ID is found at end-of-line, null otherwise.
///
/// Block ID rules:
/// - '^' followed by [a-zA-Z0-9-]+
/// - Must be at end of line (followed by \n, \r\n, or EOF)
/// - Typically preceded by a space, but we don't enforce that strictly
pub fn try_parse_block_id(buf: []const u8, pos: u32) ?BlockIdScan {
    // Must have at least one char after '^'
    const name_start = pos + 1;
    if (name_start >= buf.len) return null;

    // First char must be a valid block ID char
    if (!is_block_id_char(buf[name_start])) return null;

    // Consume block ID characters
    var end = name_start + 1;
    while (end < buf.len and is_block_id_char(buf[end])) : (end += 1) {}

    // Must be at end of line: next char is \n, \r, or we're at EOF
    if (end < buf.len) {
        const next = buf[end];
        if (next != '\n' and next != '\r') return null;
    }

    const name_len = end - name_start;
    return BlockIdScan{
        .offset = pos,
        .length = @intCast(name_len),
    };
}

/// Scalar (byte-by-byte) block ID scanner. Serves as the correctness reference
/// for verifying the SIMD implementation.
///
/// Scans `text[0..len]` for ^block-id patterns at end of line.
/// Writes results into `out[0..cap]`. Returns the number of block IDs found.
pub fn scan_block_ids_scalar(
    text: [*]const u8,
    len: u32,
    out: [*]BlockIdScan,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var pos: u32 = 0;

    while (pos < len) : (pos += 1) {
        if (buf[pos] == '^') {
            if (try_parse_block_id(buf, pos)) |block| {
                out[written] = block;
                written += 1;
                if (written >= cap) return written;
                // Skip past the block ID
                pos += 1 + block.length;
                pos -= 1; // loop increments
            }
        }
    }

    return written;
}

// ============================================================================
// Tests
// ============================================================================

test "scalar: block ID at end of line" {
    const text = "some text ^block-id\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 10), out[0].offset);
    try std.testing.expectEqual(@as(u16, 8), out[0].length);
}

test "scalar: block ID at EOF" {
    const text = "text ^myid";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
    try std.testing.expectEqual(@as(u16, 4), out[0].length);
}

test "scalar: block ID not at end of line" {
    const text = "^id more text\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "scalar: block ID at line start and EOL" {
    const text = "^first-block\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 0), out[0].offset);
    try std.testing.expectEqual(@as(u16, 11), out[0].length);
}

test "scalar: multiple block IDs" {
    const text = "line1 ^id1\nline2 ^id2\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u32, 6), out[0].offset);
    try std.testing.expectEqual(@as(u32, 17), out[1].offset);
}

test "scalar: empty input" {
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar("".ptr, 0, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "scalar: caret with no valid chars" {
    const text = "text ^ not valid\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "scalar: block ID with only digits" {
    const text = "para ^123\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].length);
}

test "scalar: CRLF line ending" {
    const text = "text ^block-id\r\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids_scalar(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 8), out[0].length);
}
