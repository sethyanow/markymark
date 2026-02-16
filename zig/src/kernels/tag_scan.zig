const std = @import("std");
const ref = @import("../reference/tag_scan_ref.zig");

pub const TagScan = ref.TagScan;

/// SIMD-accelerated tag scanner.
///
/// Uses @Vector(16, u8) to scan 16 bytes at a time for '#' characters.
/// When found, delegates to scalar parsing for boundary validation and
/// tag name extraction.
///
/// Falls back to scalar for the tail (< 16 bytes remaining).
pub fn scan_tags(
    text: [*]const u8,
    len: u32,
    out: [*]TagScan,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;

    // SIMD scan: process 16 bytes at a time looking for '#'
    const hash_vec: @Vector(16, u8) = @splat('#');
    const chunk_size: u32 = 16;
    var pos: u32 = 0;

    // Process aligned chunks
    while (pos + chunk_size <= len) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == hash_vec;

        // Check each lane for '#' matches
        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const hash_pos = pos + @as(u32, lane);
                if (ref.try_parse_tag(buf, hash_pos, len)) |tag| {
                    out[written] = tag;
                    written += 1;
                    if (written >= cap) return written;
                }
            }
        }
    }

    // Scalar tail: remaining bytes that don't fill a full vector
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '#') {
            if (ref.try_parse_tag(buf, pos, len)) |tag| {
                out[written] = tag;
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

test "simd: tag at line start" {
    const text = "#hello";
    var out: [4]TagScan = undefined;
    const w = scan_tags(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 0), out[0].offset);
    try std.testing.expectEqual(@as(u16, 5), out[0].length);
}

test "simd: tag after space" {
    const text = "text #tag rest";
    var out: [4]TagScan = undefined;
    const w = scan_tags(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
    try std.testing.expectEqual(@as(u16, 3), out[0].length);
}

test "simd: tag in mid-word rejected" {
    const text = "word#tag";
    var out: [4]TagScan = undefined;
    const w = scan_tags(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "simd: heading not a tag" {
    const text = "# heading";
    var out: [4]TagScan = undefined;
    const w = scan_tags(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "simd: multiple tags across SIMD boundary" {
    // Create text that puts tags across 16-byte boundaries
    const text = "0123456789abcde #tag1 0123456789abcde #tag2";
    var out: [8]TagScan = undefined;
    const w = scan_tags(text.ptr, text.len, &out, 8);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "simd: buffer overflow" {
    const text = "#a #b #c #d";
    var out: [2]TagScan = undefined;
    const w = scan_tags(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "simd: large input with many tags" {
    // 64 bytes of text with tags spread throughout
    const text = "#tag1 some filler text goes here #tag2 more filler #tag3 end!!";
    var out: [16]TagScan = undefined;
    const w = scan_tags(text.ptr, text.len, &out, 16);
    try std.testing.expectEqual(@as(u32, 3), w);
}

test "simd vs scalar parity" {
    const inputs = [_][]const u8{
        "#hello",
        "text #tag rest",
        "word#tag",
        "# heading",
        "#tag1 #tag2 #tag3",
        "line one\n#tag",
        "\t#tab-tag",
        "#a #b #c #d #e #f #g #h",
        "no tags here at all",
        "",
        // Long input crossing SIMD boundaries
        "0123456789abcdef#tag1 0123456789abcdef#tag2 0123456789abcdef #tag3",
    };

    for (inputs) |text| {
        var simd_out: [16]TagScan = undefined;
        var scalar_out: [16]TagScan = undefined;

        const len: u32 = @intCast(text.len);
        const simd_w = scan_tags(text.ptr, len, &simd_out, 16);
        const scalar_w = ref.scan_tags_scalar(text.ptr, len, &scalar_out, 16);

        try std.testing.expectEqual(scalar_w, simd_w);

        var i: u32 = 0;
        while (i < simd_w) : (i += 1) {
            try std.testing.expectEqual(scalar_out[i].offset, simd_out[i].offset);
            try std.testing.expectEqual(scalar_out[i].length, simd_out[i].length);
        }
    }
}
