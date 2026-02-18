const std = @import("std");
const ref = @import("../reference/fence_map_ref.zig");

pub const FenceRange = ref.FenceRange;

/// SIMD-accelerated fence map builder.
///
/// Uses @Vector(16, u8) to scan 16 bytes at a time for newline characters.
/// When newlines are found, checks the next byte for '`' or '~' and delegates
/// to scalar fence parsing logic. Fence state tracking (open/close) is inherently
/// sequential.
///
/// Falls back to scalar for the tail (<16 bytes remaining) and for the
/// first-line check.
pub fn build_fence_map(
    text: [*]const u8,
    len: u32,
    out: [*]FenceRange,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var count: u32 = 0;

    // Fence state
    var fence_open = false;
    var fence_char: u8 = 0;
    var fence_count: u32 = 0;
    var fence_start: u32 = 0;

    // Check first line (starts at position 0)
    if (buf[0] == '`' or buf[0] == '~') {
        const result = process_line_start(buf, 0, len, &fence_open, &fence_char, &fence_count, &fence_start, out, &count, cap);
        if (result.done) return count;
    }

    // SIMD scan: start from byte 0, looking for '\n' to find line boundaries.
    // The first line was handled above; the SIMD will find '\n' at its end
    // and check subsequent lines.
    var pos: u32 = 0;
    const newline_vec: @Vector(16, u8) = @splat('\n');
    const chunk_size: u32 = 16;

    while (pos + chunk_size <= len) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == newline_vec;

        // Check each lane for newline matches
        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const nl_pos = pos + @as(u32, lane);
                const next = nl_pos + 1;
                if (next < len and (buf[next] == '`' or buf[next] == '~')) {
                    const result = process_line_start(buf, next, len, &fence_open, &fence_char, &fence_count, &fence_start, out, &count, cap);
                    if (result.done) return count;
                }
            }
        }

        pos += chunk_size;
    }

    // Scalar tail: remaining bytes that don't fill a full vector
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '\n') {
            const next = pos + 1;
            if (next < len and (buf[next] == '`' or buf[next] == '~')) {
                const result = process_line_start(buf, next, len, &fence_open, &fence_char, &fence_count, &fence_start, out, &count, cap);
                if (result.done) return count;
            }
        }
    }

    // Unclosed fence at EOF
    if (fence_open and count < cap) {
        out[count] = FenceRange{
            .start = fence_start,
            .end = len,
        };
        count += 1;
    }

    return count;
}

const ProcessResult = struct {
    next_pos: u32,
    done: bool,
};

/// Process a potential fence at a line start position.
/// Updates fence state and writes ranges as needed.
/// Returns whether processing should stop (buffer full).
fn process_line_start(
    buf: []const u8,
    line_start: u32,
    len: u32,
    fence_open: *bool,
    fence_char: *u8,
    fence_count: *u32,
    fence_start: *u32,
    out: [*]FenceRange,
    count: *u32,
    cap: u32,
) ProcessResult {
    const ch = buf[line_start];

    // Count consecutive fence characters
    var ch_count: u32 = 0;
    var scan = line_start;
    while (scan < len and buf[scan] == ch) {
        ch_count += 1;
        scan += 1;
    }

    if (ch_count < 3) {
        return .{ .next_pos = ref.skip_to_next_line(buf, line_start, len), .done = false };
    }

    if (!fence_open.*) {
        // Potential opening fence
        if (ref.is_valid_opening(buf, ch, scan, len)) {
            fence_open.* = true;
            fence_char.* = ch;
            fence_count.* = ch_count;
            fence_start.* = line_start;
        }
    } else if (ch == fence_char.* and ch_count >= fence_count.*) {
        // Potential closing fence
        if (ref.is_only_whitespace_after(buf, scan, len)) {
            const line_end = ref.skip_to_next_line(buf, line_start, len);
            // Guard BEFORE write: if buffer is already full, return immediately
            // without writing out[count.*] which would be out-of-bounds.
            if (count.* >= cap) {
                return .{ .next_pos = line_end, .done = true };
            }
            out[count.*] = FenceRange{
                .start = fence_start.*,
                .end = line_end,
            };
            count.* += 1;
            fence_open.* = false;
            if (count.* >= cap) {
                return .{ .next_pos = line_end, .done = true };
            }
        }
    }

    return .{ .next_pos = ref.skip_to_next_line(buf, line_start, len), .done = false };
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_empty_input" {
    const text = "";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, 0, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_no_fences" {
    const text = "# Heading\nSome regular text\nMore text\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_single_fence_pair" {
    const text = "```\ncode here\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "test_unclosed_fence" {
    const text = "```\ncode here\nmore code\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "test_multiple_fences" {
    const text = "```\nblock1\n```\ntext between\n```\nblock2\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 2), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expect(out[1].start > out[0].end);
}

test "test_tilde_fences" {
    const text = "~~~\ntilde code\n~~~\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "test_mixed_fence_types" {
    const text = "```\ncode\n~~~\nstill in backtick fence\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "test_inline_code_not_fence" {
    const text = "Use `code` inline\nAnd `more` here\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_fence_with_language" {
    const text = "```python\ndef foo():\n    pass\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "test_four_backtick_fence" {
    const text = "````\ncode with ``` inside\n````\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "test_indented_backticks" {
    const text = "    ```\nnot fenced\n    ```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_adjacent_empty_fences" {
    const text = "```\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "test_buffer_overflow" {
    const text = "```\na\n```\n```\nb\n```\n```\nc\n```\n";
    var out: [1]FenceRange = undefined;
    const n = build_fence_map(text.ptr, text.len, &out, 1);
    try testing.expectEqual(@as(u32, 1), n);
}

test "test_buffer_full_before_closing_fence_write" {
    // Regression test for marky-wpl: cap check must occur BEFORE writing out[count].
    // With cap=1, after the first fence pair fills slot 0, count==cap.
    // The second fence pair's closing fence must not write out[1] (OOB).
    // The fix moves the cap guard before the write in process_line_start.
    const text = "```\na\n```\n```\nb\n```\n";
    var out: [1]FenceRange = undefined;
    @memset(std.mem.asBytes(&out), 0xAA); // poison sentinel
    const n = build_fence_map(text.ptr, text.len, &out, 1);
    // Only the first fence pair should be recorded
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
}

test "test_simd_scalar_parity" {
    // Build a string that exercises SIMD chunk boundaries
    const text = "some padding text...\n```python\ncode block one\nwith multiple lines\n```\nmore regular text here and more padding to cross chunk\n~~~\ntilde block\n~~~\nfinal text\n";
    var simd_out: [16]FenceRange = undefined;
    var scalar_out: [16]FenceRange = undefined;

    const simd_n = build_fence_map(text.ptr, text.len, &simd_out, 16);
    const scalar_n = ref.build_fence_map_scalar(text.ptr, text.len, &scalar_out, 16);

    try testing.expectEqual(scalar_n, simd_n);

    var i: u32 = 0;
    while (i < simd_n) : (i += 1) {
        try testing.expectEqual(scalar_out[i].start, simd_out[i].start);
        try testing.expectEqual(scalar_out[i].end, simd_out[i].end);
    }
}

test "test_simd_scalar_parity_large" {
    // Larger input that definitely crosses multiple SIMD boundaries
    const line = "This is regular markdown text that has no fence characters at all.\n";
    const fence_open = "```rust\n";
    const code_line = "fn main() { println!(\"hello\"); }\n";
    const fence_close = "```\n";
    comptime var text: []const u8 = "";
    comptime {
        // ~5KB of content with fences at various positions
        var i = 0;
        while (i < 50) : (i += 1) {
            text = text ++ line;
            if (i % 10 == 5) {
                text = text ++ fence_open ++ code_line ++ code_line ++ fence_close;
            }
        }
    }

    var simd_out: [16]FenceRange = undefined;
    var scalar_out: [16]FenceRange = undefined;

    const simd_n = build_fence_map(text.ptr, text.len, &simd_out, 16);
    const scalar_n = ref.build_fence_map_scalar(text.ptr, text.len, &scalar_out, 16);

    try testing.expectEqual(scalar_n, simd_n);
    var i: u32 = 0;
    while (i < simd_n) : (i += 1) {
        try testing.expectEqual(scalar_out[i].start, simd_out[i].start);
        try testing.expectEqual(scalar_out[i].end, simd_out[i].end);
    }
}

// ============================================================================
// Benchmark
// ============================================================================

test "bench_simd_vs_scalar" {
    const line = "This is a regular line of markdown text that contains no fences.\n";
    const fence_block = "```python\ndef example():\n    return 42\n```\n";
    comptime var markdown: []const u8 = "";
    comptime {
        var i = 0;
        while (i < 143) : (i += 1) {
            if (i % 20 == 0) {
                markdown = markdown ++ fence_block;
            } else {
                markdown = markdown ++ line;
            }
        }
    }
    const len: u32 = @intCast(markdown.len);
    var out: [64]FenceRange = undefined;

    // Warm up
    _ = build_fence_map(markdown.ptr, len, &out, 64);
    _ = ref.build_fence_map_scalar(markdown.ptr, len, &out, 64);

    const iterations: u32 = 1000;
    var timer = std.time.Timer.start() catch return;

    // Benchmark SIMD
    timer.reset();
    var i: u32 = 0;
    while (i < iterations) : (i += 1) {
        _ = build_fence_map(markdown.ptr, len, &out, 64);
    }
    const simd_ns = timer.read();

    // Benchmark scalar
    timer.reset();
    i = 0;
    while (i < iterations) : (i += 1) {
        _ = ref.build_fence_map_scalar(markdown.ptr, len, &out, 64);
    }
    const scalar_ns = timer.read();

    std.debug.print("\n[bench] {d}KB markdown, {d} iterations\n", .{ len / 1024, iterations });
    std.debug.print("[bench] SIMD:   {d}ns total, {d}ns/iter\n", .{ simd_ns, simd_ns / iterations });
    std.debug.print("[bench] Scalar: {d}ns total, {d}ns/iter\n", .{ scalar_ns, scalar_ns / iterations });
    if (scalar_ns > 0) {
        std.debug.print("[bench] Speedup: {d:.1}x\n", .{@as(f64, @floatFromInt(scalar_ns)) / @as(f64, @floatFromInt(simd_ns))});
    }

    // Both should find the same number of fences
    const simd_count = build_fence_map(markdown.ptr, len, &out, 64);
    const scalar_count = ref.build_fence_map_scalar(markdown.ptr, len, &out, 64);
    try testing.expectEqual(scalar_count, simd_count);
}
