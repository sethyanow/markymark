const std = @import("std");
const ref = @import("../reference/multi_scan_ref.zig");

pub const ScanResult = ref.ScanResult;
pub const ScanType = ref.ScanType;
pub const NUM_SCAN_TYPES = ref.NUM_SCAN_TYPES;

/// SIMD-accelerated single-pass multi-pattern scanner using Aho-Corasick automaton.
///
/// Uses @Vector(16, u8) to scan 16 bytes at a time. When the automaton is in
/// state 0 (root), checks if the chunk contains any pattern-prefix bytes
/// (#, [, \n, ^). If none are present, skips the entire chunk. Otherwise
/// falls back to scalar automaton transitions for that chunk.
///
/// Parameters:
///   text, len: input document
///   out, cap:  output buffer for ScanResult entries
///
/// Returns: number of results written (capped at cap)
pub fn scan_multi(
    text: [*]const u8,
    len: u32,
    out: [*]ScanResult,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var state: u8 = ref.STATE_BOF; // BOF = after-newline
    var pos: u32 = 0;

    const chunk_size: u32 = 16;

    // SIMD comparison vectors for root-leaving bytes
    const hash_vec: @Vector(16, u8) = @splat('#'); // 0x23
    const bracket_vec: @Vector(16, u8) = @splat('['); // 0x5B
    const newline_vec: @Vector(16, u8) = @splat('\n'); // 0x0A
    const caret_vec: @Vector(16, u8) = @splat('^'); // 0x5E

    while (pos + chunk_size <= len) {
        if (state == 0) {
            // Fast path: check if chunk has any interesting bytes
            const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
            const m_hash = chunk == hash_vec;
            const m_bracket = chunk == bracket_vec;
            const m_newline = chunk == newline_vec;
            const m_caret = chunk == caret_vec;

            // OR all match masks together
            const any_match = @reduce(.Or, m_hash) or @reduce(.Or, m_bracket) or
                @reduce(.Or, m_newline) or @reduce(.Or, m_caret);

            if (!any_match) {
                // No interesting bytes in this chunk — state stays 0, skip ahead
                pos += chunk_size;
                continue;
            }
        }

        // Slow path: process byte-by-byte through automaton for this chunk
        const chunk_end = @min(pos + chunk_size, len);
        while (pos < chunk_end) {
            state = ref.automaton.step(state, buf[pos]);
            const output = ref.automaton.matches(state);

            if (output != 0) {
                inline for (0..NUM_SCAN_TYPES) |t| {
                    if (output & (@as(u8, 1) << t) != 0) {
                        if (written >= cap) return written;
                        out[written] = ScanResult{
                            .offset = pos,
                            .length = 0,
                            .scan_type = @intCast(t),
                            .extra = 0,
                        };
                        written += 1;
                    }
                }
            }
            pos += 1;
        }
    }

    // Scalar tail: remaining bytes that don't fill a full vector
    while (pos < len) {
        state = ref.automaton.step(state, buf[pos]);
        const output = ref.automaton.matches(state);

        if (output != 0) {
            inline for (0..NUM_SCAN_TYPES) |t| {
                if (output & (@as(u8, 1) << t) != 0) {
                    if (written >= cap) return written;
                    out[written] = ScanResult{
                        .offset = pos,
                        .length = 0,
                        .scan_type = @intCast(t),
                        .extra = 0,
                    };
                    written += 1;
                }
            }
        }
        pos += 1;
    }

    return written;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_simd_scalar_parity" {
    // Build a string that exercises SIMD chunk boundaries
    const text = "some padding text..\n# Heading after pad\nmore text here......\n## Second heading\n[[wiki]] and [link](url)\ntext ^block-id\n```\ncode\n```\ntext #tag1 #tag2\n~~~\ntilde\n~~~\n";
    var simd_out: [64]ScanResult = undefined;
    var scalar_out: [64]ScanResult = undefined;

    const simd_n = scan_multi(text.ptr, text.len, &simd_out, 64);
    const scalar_n = ref.scan_multi_scalar(text.ptr, text.len, &scalar_out, 64);

    try testing.expectEqual(scalar_n, simd_n);

    var i: u32 = 0;
    while (i < simd_n) : (i += 1) {
        try testing.expectEqual(scalar_out[i].offset, simd_out[i].offset);
        try testing.expectEqual(scalar_out[i].scan_type, simd_out[i].scan_type);
    }
}

test "test_short_document" {
    // Document shorter than SIMD vector width (pure scalar fallback)
    const text = "# Hi\n[x](y)";
    var simd_out: [16]ScanResult = undefined;
    var scalar_out: [16]ScanResult = undefined;

    const simd_n = scan_multi(text.ptr, text.len, &simd_out, 16);
    const scalar_n = ref.scan_multi_scalar(text.ptr, text.len, &scalar_out, 16);

    try testing.expectEqual(scalar_n, simd_n);

    var i: u32 = 0;
    while (i < simd_n) : (i += 1) {
        try testing.expectEqual(scalar_out[i].offset, simd_out[i].offset);
        try testing.expectEqual(scalar_out[i].scan_type, simd_out[i].scan_type);
    }
}

test "test_many_matches" {
    // Document with many matches to test buffer management
    comptime var text: []const u8 = "";
    comptime {
        var i = 0;
        while (i < 50) : (i += 1) {
            text = text ++ "# H\n";
        }
    }
    var simd_out: [256]ScanResult = undefined;
    var scalar_out: [256]ScanResult = undefined;

    const simd_n = scan_multi(text.ptr, text.len, &simd_out, 256);
    const scalar_n = ref.scan_multi_scalar(text.ptr, text.len, &scalar_out, 256);

    try testing.expectEqual(scalar_n, simd_n);

    var i: u32 = 0;
    while (i < simd_n) : (i += 1) {
        try testing.expectEqual(scalar_out[i].offset, simd_out[i].offset);
        try testing.expectEqual(scalar_out[i].scan_type, simd_out[i].scan_type);
    }
}

test "test_pattern_at_chunk_boundary" {
    // Place patterns exactly at 16-byte boundaries
    // 15 bytes of padding + \n to hit boundary, then # at byte 16
    const text = "aaaaaaaaaaaaaaa\n# Heading at boundary\n";
    var simd_out: [16]ScanResult = undefined;
    var scalar_out: [16]ScanResult = undefined;

    const simd_n = scan_multi(text.ptr, text.len, &simd_out, 16);
    const scalar_n = ref.scan_multi_scalar(text.ptr, text.len, &scalar_out, 16);

    try testing.expectEqual(scalar_n, simd_n);

    var i: u32 = 0;
    while (i < simd_n) : (i += 1) {
        try testing.expectEqual(scalar_out[i].offset, simd_out[i].offset);
        try testing.expectEqual(scalar_out[i].scan_type, simd_out[i].scan_type);
    }
}

test "test_empty_input" {
    const text = "";
    var out: [4]ScanResult = undefined;
    const n = scan_multi(text.ptr, 0, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_no_matches_plain_text" {
    // 48 bytes of plain text with no interesting chars
    const text = "This is plain text without any special markdown.";
    var out: [4]ScanResult = undefined;
    const n = scan_multi(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_buffer_overflow_stops" {
    const text = "# A\n# B\n# C\n# D\n";
    var out: [2]ScanResult = undefined;
    const n = scan_multi(text.ptr, text.len, &out, 2);
    try testing.expectEqual(@as(u32, 2), n);
}

test "test_all_element_types" {
    const text = "# Title\nSome text #tag1\n[[wiki]] and [link](url)\ntext ^block-id\n```\ncode\n```\n~~~\ntilde\n~~~\n";
    var simd_out: [64]ScanResult = undefined;
    var scalar_out: [64]ScanResult = undefined;

    const simd_n = scan_multi(text.ptr, text.len, &simd_out, 64);
    const scalar_n = ref.scan_multi_scalar(text.ptr, text.len, &scalar_out, 64);

    try testing.expectEqual(scalar_n, simd_n);

    // Verify all types found
    var counts = [_]u32{0} ** NUM_SCAN_TYPES;
    for (simd_out[0..simd_n]) |r| {
        counts[r.scan_type] += 1;
    }

    try testing.expect(counts[@intFromEnum(ScanType.heading)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.tag)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.wiki_link)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.link_open)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.block_id)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.fence_backtick)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.fence_tilde)] >= 1);
}

test "test_bof_heading" {
    // Heading at beginning of file (no preceding newline)
    const text = "# Title";
    var simd_out: [8]ScanResult = undefined;
    var scalar_out: [8]ScanResult = undefined;

    const simd_n = scan_multi(text.ptr, text.len, &simd_out, 8);
    const scalar_n = ref.scan_multi_scalar(text.ptr, text.len, &scalar_out, 8);

    try testing.expectEqual(scalar_n, simd_n);

    var found_heading = false;
    for (simd_out[0..simd_n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.heading)) found_heading = true;
    }
    try testing.expect(found_heading);
}

// ============================================================================
// Benchmark
// ============================================================================

test "bench_simd_vs_scalar" {
    // Generate ~10KB of markdown with mixed elements
    const line = "This is a regular line of markdown text that contains no headings at all.\n";
    const heading = "## Section Title\n";
    const link_line = "Check out [this link](https://example.com) for details.\n";
    const wiki_line = "See also [[related page]] for more.\n";

    comptime var markdown: []const u8 = "";
    comptime {
        var i = 0;
        while (i < 143) : (i += 1) {
            if (i % 11 == 0) {
                markdown = markdown ++ heading;
            } else if (i % 17 == 0) {
                markdown = markdown ++ link_line;
            } else if (i % 23 == 0) {
                markdown = markdown ++ wiki_line;
            } else {
                markdown = markdown ++ line;
            }
        }
    }
    const len: u32 = @intCast(markdown.len);
    var out: [256]ScanResult = undefined;

    // Warm up
    _ = scan_multi(markdown.ptr, len, &out, 256);
    _ = ref.scan_multi_scalar(markdown.ptr, len, &out, 256);

    const iterations: u32 = 1000;
    var timer = std.time.Timer.start() catch return;

    // Benchmark SIMD
    timer.reset();
    var i: u32 = 0;
    while (i < iterations) : (i += 1) {
        _ = scan_multi(markdown.ptr, len, &out, 256);
    }
    const simd_ns = timer.read();

    // Benchmark scalar
    timer.reset();
    i = 0;
    while (i < iterations) : (i += 1) {
        _ = ref.scan_multi_scalar(markdown.ptr, len, &out, 256);
    }
    const scalar_ns = timer.read();

    // Print results
    std.debug.print("\n[bench] {d}KB markdown, {d} iterations\n", .{ len / 1024, iterations });
    std.debug.print("[bench] SIMD:   {d}ns total, {d}ns/iter\n", .{ simd_ns, simd_ns / iterations });
    std.debug.print("[bench] Scalar: {d}ns total, {d}ns/iter\n", .{ scalar_ns, scalar_ns / iterations });
    if (scalar_ns > 0) {
        std.debug.print("[bench] Speedup: {d:.1}x\n", .{@as(f64, @floatFromInt(scalar_ns)) / @as(f64, @floatFromInt(simd_ns))});
    }

    // Parity check
    const simd_count = scan_multi(markdown.ptr, len, &out, 256);
    var scalar_out: [256]ScanResult = undefined;
    const scalar_count = ref.scan_multi_scalar(markdown.ptr, len, &scalar_out, 256);
    try testing.expectEqual(scalar_count, simd_count);
}
