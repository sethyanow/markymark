const std = @import("std");
const ref = @import("../reference/heading_scan_ref.zig");

pub const HeadingScan = ref.HeadingScan;

/// SIMD-accelerated heading scanner.
///
/// Uses @Vector(16, u8) to scan 16 bytes at a time for newline characters.
/// When newlines are found, checks the byte after each for '#' and delegates
/// to scalar parsing for heading validation and text extraction.
///
/// Falls back to scalar for the tail (< 16 bytes remaining) and for the
/// first-line check.
pub fn scan_headings(
    text: [*]const u8,
    len: u32,
    out: [*]HeadingScan,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;

    // Check first line (no preceding newline needed)
    if (buf[0] == '#') {
        if (ref.try_parse_heading(buf, 0, len)) |h| {
            out[written] = h;
            written += 1;
            if (written >= cap) return written;
        }
    }

    // SIMD scan: process 16 bytes at a time looking for '\n'
    const newline_vec: @Vector(16, u8) = @splat('\n');
    const chunk_size: u32 = 16;
    var pos: u32 = 0;

    // Process aligned chunks
    while (pos + chunk_size <= len) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == newline_vec;

        // Check each lane for newline matches
        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const nl_pos = pos + @as(u32, lane);
                const next = nl_pos + 1;
                if (next < len and buf[next] == '#') {
                    if (ref.try_parse_heading(buf, next, len)) |h| {
                        out[written] = h;
                        written += 1;
                        if (written >= cap) return written;
                    }
                }
            }
        }
    }

    // Scalar tail: remaining bytes that don't fill a full vector
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '\n') {
            const next = pos + 1;
            if (next < len and buf[next] == '#') {
                if (ref.try_parse_heading(buf, next, len)) |h| {
                    out[written] = h;
                    written += 1;
                    if (written >= cap) return written;
                }
            }
        }
    }

    return written;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_empty_input" {
    const text = "";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings(text.ptr, 0, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_single_h1" {
    const text = "# Hello\n";
    var out: [8]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 2), out[0].offset);
    try testing.expectEqual(@as(u16, 5), out[0].length);
    try testing.expectEqual(@as(u8, 1), out[0].level);
}

test "test_all_levels" {
    const text = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n";
    var out: [8]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 6), n);
    var i: u8 = 0;
    while (i < 6) : (i += 1) {
        try testing.expectEqual(i + 1, out[i].level);
    }
}

test "test_heading_first_line" {
    const text = "## First line heading";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u8, 2), out[0].level);
    try testing.expectEqual(@as(u32, 3), out[0].offset);
    try testing.expectEqual(@as(u16, 18), out[0].length); // "First line heading"
}

test "test_heading_at_eof" {
    const text = "some text\n# EOF heading";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u8, 1), out[0].level);
    try testing.expectEqual(@as(u16, 11), out[0].length); // "EOF heading"
}

test "test_consecutive_headings" {
    const text = "# One\n## Two\n### Three\n";
    var out: [8]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 3), n);
    try testing.expectEqual(@as(u8, 1), out[0].level);
    try testing.expectEqual(@as(u8, 2), out[1].level);
    try testing.expectEqual(@as(u8, 3), out[2].level);
}

test "test_hash_in_middle_of_line" {
    const text = "foo # bar\nbaz ## qux\n";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_heading_in_code_block" {
    // Known false positive: heading_scan is context-unaware and will detect
    // headings inside code blocks. This is by design — tree-sitter handles
    // full AST context when needed.
    const text = "```\n# Not really a heading\n```\n";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 4);
    // Documents the false positive: scanner finds the heading
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u8, 1), out[0].level);
}

test "test_atx_closing_hashes" {
    const text = "## Heading ##\n";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u16, 7), out[0].length); // "Heading" not "Heading ##"
    try testing.expectEqual(@as(u8, 2), out[0].level);
}

test "test_buffer_overflow" {
    const text = "# A\n# B\n# C\n# D\n# E\n";
    var out: [1]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 1);
    try testing.expectEqual(@as(u32, 1), n);
    // Only first heading captured
    try testing.expectEqual(@as(u8, 1), out[0].level);
}

test "test_simd_scalar_parity" {
    // Build a string that exercises SIMD chunk boundaries
    // 20 bytes of padding + headings ensures we cross a 16-byte boundary
    const text = "some padding text..\n# Heading after padding\nmore text here......\n## Second heading\n";
    var simd_out: [16]HeadingScan = undefined;
    var scalar_out: [16]HeadingScan = undefined;

    const simd_n = scan_headings(text.ptr, text.len, &simd_out, 16);
    const scalar_n = ref.scan_headings_scalar(text.ptr, text.len, &scalar_out, 16);

    try testing.expectEqual(scalar_n, simd_n);

    var i: u32 = 0;
    while (i < simd_n) : (i += 1) {
        try testing.expectEqual(scalar_out[i].offset, simd_out[i].offset);
        try testing.expectEqual(scalar_out[i].length, simd_out[i].length);
        try testing.expectEqual(scalar_out[i].level, simd_out[i].level);
    }
}

test "test_level_seven_ignored" {
    const text = "####### not a heading\n";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_heading_no_space" {
    const text = "##no-space\n";
    var out: [4]HeadingScan = undefined;
    const n = scan_headings(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

// ============================================================================
// Benchmark
// ============================================================================

test "bench_simd_vs_scalar" {
    // Generate ~10KB of markdown with sparse headings
    const line = "This is a regular line of markdown text that contains no headings at all.\n";
    const heading = "## Section Title\n";
    comptime var markdown: []const u8 = "";
    comptime {
        // ~10KB: 130 regular lines (~9.5KB) + 13 headings (~220B)
        var i = 0;
        while (i < 143) : (i += 1) {
            if (i % 11 == 0) {
                markdown = markdown ++ heading;
            } else {
                markdown = markdown ++ line;
            }
        }
    }
    const len: u32 = @intCast(markdown.len);
    var out: [64]HeadingScan = undefined;

    // Warm up
    _ = scan_headings(markdown.ptr, len, &out, 64);
    _ = ref.scan_headings_scalar(markdown.ptr, len, &out, 64);

    const iterations: u32 = 1000;
    var timer = std.time.Timer.start() catch return;

    // Benchmark SIMD
    timer.reset();
    var i: u32 = 0;
    while (i < iterations) : (i += 1) {
        _ = scan_headings(markdown.ptr, len, &out, 64);
    }
    const simd_ns = timer.read();

    // Benchmark scalar
    timer.reset();
    i = 0;
    while (i < iterations) : (i += 1) {
        _ = ref.scan_headings_scalar(markdown.ptr, len, &out, 64);
    }
    const scalar_ns = timer.read();

    // Print results (visible with `zig build test --summary all`)
    std.debug.print("\n[bench] {d}KB markdown, {d} iterations\n", .{ len / 1024, iterations });
    std.debug.print("[bench] SIMD:   {d}ns total, {d}ns/iter\n", .{ simd_ns, simd_ns / iterations });
    std.debug.print("[bench] Scalar: {d}ns total, {d}ns/iter\n", .{ scalar_ns, scalar_ns / iterations });
    if (scalar_ns > 0) {
        std.debug.print("[bench] Speedup: {d:.1}x\n", .{@as(f64, @floatFromInt(scalar_ns)) / @as(f64, @floatFromInt(simd_ns))});
    }

    // Both should find the same number of headings
    const simd_count = scan_headings(markdown.ptr, len, &out, 64);
    const scalar_count = ref.scan_headings_scalar(markdown.ptr, len, &out, 64);
    try testing.expectEqual(scalar_count, simd_count);
}
