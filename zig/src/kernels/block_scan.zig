const std = @import("std");
const ref = @import("../reference/block_scan_ref.zig");

pub const BlockIdScan = ref.BlockIdScan;

/// SIMD-accelerated block ID scanner.
///
/// Uses @Vector(16, u8) to scan 16 bytes at a time for '^' characters.
/// When found, delegates to scalar parsing for block ID extraction and
/// end-of-line verification.
///
/// Falls back to scalar for the tail (< 16 bytes remaining).
pub fn scan_block_ids(
    text: [*]const u8,
    len: u32,
    out: [*]BlockIdScan,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;

    // SIMD scan: process 16 bytes at a time looking for '^'
    const caret_vec: @Vector(16, u8) = @splat('^');
    const chunk_size: u32 = 16;
    var pos: u32 = 0;

    // Process aligned chunks
    while (pos + chunk_size <= len) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == caret_vec;

        // Check each lane for '^' matches
        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const caret_pos = pos + @as(u32, lane);
                if (ref.try_parse_block_id(buf, caret_pos)) |block| {
                    out[written] = block;
                    written += 1;
                    if (written >= cap) return written;
                }
            }
        }
    }

    // Scalar tail: remaining bytes that don't fill a full vector
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '^') {
            if (ref.try_parse_block_id(buf, pos)) |block| {
                out[written] = block;
                written += 1;
                if (written >= cap) return written;
            }
        }
    }

    return written;
}

// ============================================================================
// Tests
// ============================================================================

test "simd: block ID at end of line" {
    const text = "some text ^block-id\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 10), out[0].offset);
    try std.testing.expectEqual(@as(u16, 8), out[0].length);
}

test "simd: block ID at EOF" {
    const text = "text ^myid";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
    try std.testing.expectEqual(@as(u16, 4), out[0].length);
}

test "simd: block ID not at end of line" {
    const text = "^id more text\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "simd: buffer overflow" {
    const text = "a ^id1\nb ^id2\nc ^id3\n";
    var out: [1]BlockIdScan = undefined;
    const w = scan_block_ids(text.ptr, text.len, &out, 1);
    try std.testing.expectEqual(@as(u32, 1), w);
}

test "simd: block ID across SIMD boundary" {
    // Push the '^' past 16-byte boundary
    const text = "0123456789abcdef ^block\n";
    var out: [4]BlockIdScan = undefined;
    const w = scan_block_ids(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
}

test "simd vs scalar parity" {
    const inputs = [_][]const u8{
        "some text ^block-id\n",
        "text ^myid",
        "^id more text\n",
        "^first-block\n",
        "line1 ^id1\nline2 ^id2\n",
        "",
        "text ^ not valid\n",
        "para ^123\n",
        "text ^block-id\r\n",
        // Long input crossing SIMD boundaries
        "0123456789abcdef ^block1\n0123456789abcdef ^block2\n",
        "no carets here at all\n",
    };

    for (inputs) |text| {
        var simd_out: [16]BlockIdScan = undefined;
        var scalar_out: [16]BlockIdScan = undefined;

        const len: u32 = @intCast(text.len);
        const simd_w = scan_block_ids(text.ptr, len, &simd_out, 16);
        const scalar_w = ref.scan_block_ids_scalar(text.ptr, len, &scalar_out, 16);

        try std.testing.expectEqual(scalar_w, simd_w);

        var i: u32 = 0;
        while (i < simd_w) : (i += 1) {
            try std.testing.expectEqual(scalar_out[i].offset, simd_out[i].offset);
            try std.testing.expectEqual(scalar_out[i].length, simd_out[i].length);
        }
    }
}
