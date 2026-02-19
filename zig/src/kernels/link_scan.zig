const std = @import("std");
const ref = @import("../reference/link_scan_ref.zig");

pub const LinkScan = ref.LinkScan;

/// SIMD-accelerated link scanner.
///
/// Uses @Vector(16, u8) to scan 16 bytes at a time for '[' characters.
/// When brackets are found, delegates to scalar parsing for link validation
/// and text/target extraction. This follows the same SIMD-find + scalar-validate
/// pattern as heading_scan.
///
/// Handles:
///   - Markdown links: [text](url)
///   - Wiki-links: [[target]] and [[target|display]]
///   - Nested brackets in link text
///   - Escaped brackets (\[)
///   - Image links: ![alt](url)
///
/// Falls back to scalar for the tail (< 16 bytes remaining).
pub fn scan_links(
    text: [*]const u8,
    len: u32,
    out: [*]LinkScan,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var pos: u32 = 0;

    // SIMD scan: process 16 bytes at a time looking for '['
    const bracket_vec: @Vector(16, u8) = @splat('[');
    const chunk_size: u32 = 16;

    while (pos + chunk_size <= len) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == bracket_vec;

        // Check if any lane matched
        var any_match = false;
        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                any_match = true;
            }
        }

        if (!any_match) {
            pos += chunk_size;
            continue;
        }

        // Found '[' in this chunk — process each match.
        // Note: process_bracket checks buf[bracket_pos] == '[' internally via
        // the parse functions, so calling it on positions inside an already-consumed
        // link is safe (they won't start with unescaped '[').
        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const bracket_pos = pos + @as(u32, lane);
                _ = process_bracket(buf, bracket_pos, len, out, &written, cap);
                if (written >= cap) return written;
            }
        }
        pos += chunk_size;
    }

    // Scalar tail: remaining bytes that don't fill a full vector
    while (pos < len) {
        if (buf[pos] == '[') {
            _ = process_bracket(buf, pos, len, out, &written, cap);
            if (written >= cap) return written;
        }
        pos += 1;
    }

    return written;
}

/// Process a '[' found at `bracket_pos`. Returns number of characters consumed
/// beyond the bracket (0 if not a link).
fn process_bracket(
    buf: []const u8,
    bracket_pos: u32,
    len: u32,
    out: [*]LinkScan,
    written: *u32,
    cap: u32,
) u32 {
    if (written.* >= cap) return 0;

    // Check for escaped bracket
    if (ref.is_escaped(buf, bracket_pos)) return 0;

    // Check for wiki-link: [[
    if (bracket_pos + 1 < len and buf[bracket_pos + 1] == '[') {
        if (ref.try_parse_wiki_link(buf, bracket_pos, len)) |link| {
            out[written.*] = link;
            written.* += 1;
            // Return advance past the closing ]]
            const end = find_wiki_close(buf, link.target_offset + link.target_length, len);
            if (end > bracket_pos) return end - bracket_pos;
            return 0;
        }
    }

    // Check for markdown link: [text](url)
    // Also handles ![alt](url) — the '!' precedes the '[' which we already found
    if (ref.try_parse_markdown_link(buf, bracket_pos, len)) |link| {
        var result = link;
        // Check for image link prefix
        if (bracket_pos > 0 and buf[bracket_pos - 1] == '!') {
            result.offset = bracket_pos - 1;
        }
        out[written.*] = result;
        written.* += 1;
        const end = link.target_offset + link.target_length + 1; // +1 for ')'
        if (end > bracket_pos) return end - bracket_pos;
        return 0;
    }

    return 0;
}

/// Find the position after ']]' starting from `start`.
fn find_wiki_close(buf: []const u8, start: u32, len: u32) u32 {
    var pos = start;
    while (pos < len -| 1) {
        if (buf[pos] == ']' and buf[pos + 1] == ']') {
            return pos + 2;
        }
        pos += 1;
    }
    return start;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_empty_input" {
    const text = "";
    var out: [4]LinkScan = undefined;
    const n = scan_links(text.ptr, 0, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_single_markdown_link" {
    const text = "[hello](https://example.com)";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].offset);
    try testing.expectEqual(@as(u32, 1), out[0].text_offset);
    try testing.expectEqual(@as(u16, 5), out[0].text_length);
    try testing.expectEqual(@as(u32, 8), out[0].target_offset);
    try testing.expectEqual(@as(u16, 19), out[0].target_length);
    try testing.expectEqual(@as(u8, 0), out[0].link_type);
}

test "test_single_wiki_link" {
    const text = "[[my page]]";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].offset);
    try testing.expectEqual(@as(u16, 7), out[0].text_length);
    try testing.expectEqual(@as(u8, 1), out[0].link_type);
}

test "test_wiki_link_with_pipe" {
    const text = "[[target page|display text]]";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 2), out[0].target_offset);
    try testing.expectEqual(@as(u16, 11), out[0].target_length);
    try testing.expectEqual(@as(u32, 14), out[0].text_offset);
    try testing.expectEqual(@as(u16, 12), out[0].text_length);
    try testing.expectEqual(@as(u8, 1), out[0].link_type);
}

test "test_nested_brackets" {
    const text = "[text [with] brackets](url)";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u16, 20), out[0].text_length); // "text [with] brackets"
    try testing.expectEqual(@as(u16, 3), out[0].target_length); // "url"
    try testing.expectEqual(@as(u8, 0), out[0].link_type);
}

test "test_escaped_bracket" {
    const text = "\\[not a link\\](url)";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_empty_text" {
    const text = "[](url)";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u16, 0), out[0].text_length);
    try testing.expectEqual(@as(u16, 3), out[0].target_length);
}

test "test_empty_target" {
    const text = "[text]()";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u16, 4), out[0].text_length);
    try testing.expectEqual(@as(u16, 0), out[0].target_length);
}

test "test_adjacent_links" {
    const text = "[a](b)[c](d)";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 2), n);
    try testing.expectEqual(@as(u32, 0), out[0].offset);
    try testing.expectEqual(@as(u16, 1), out[0].text_length); // "a"
    try testing.expectEqual(@as(u16, 1), out[0].target_length); // "b"
    try testing.expectEqual(@as(u32, 6), out[1].offset);
    try testing.expectEqual(@as(u16, 1), out[1].text_length); // "c"
    try testing.expectEqual(@as(u16, 1), out[1].target_length); // "d"
}

test "test_unclosed_bracket" {
    const text = "[text without closing";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_url_with_parens" {
    const text = "[text](url(with)parens)";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u16, 15), out[0].target_length); // "url(with)parens"
}

test "test_markdown_and_wiki_mixed" {
    const text = "[md link](url) and [[wiki link]]";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 2), n);
    try testing.expectEqual(@as(u8, 0), out[0].link_type); // markdown
    try testing.expectEqual(@as(u8, 1), out[1].link_type); // wiki
}

test "test_buffer_overflow" {
    const text = "[a](b) [c](d) [e](f)";
    var out: [1]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 1);
    try testing.expectEqual(@as(u32, 1), n);
}

test "test_image_link" {
    const text = "![alt text](image.png)";
    var out: [8]LinkScan = undefined;
    const n = scan_links(text.ptr, text.len, &out, 8);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].offset); // includes '!'
    try testing.expectEqual(@as(u16, 8), out[0].text_length); // "alt text"
    try testing.expectEqual(@as(u16, 9), out[0].target_length); // "image.png"
}

test "test_simd_scalar_parity" {
    // Build a string that exercises SIMD chunk boundaries
    const text = "some padding text..\n[Link One](https://example.com/one) more text here......\n[[Wiki Link]] and [Link Two](url2)\nfinal line with [[wiki|display text]] end.\n";
    var simd_out: [16]LinkScan = undefined;
    var scalar_out: [16]LinkScan = undefined;

    const simd_n = scan_links(text.ptr, text.len, &simd_out, 16);
    const scalar_n = ref.scan_links_scalar(text.ptr, text.len, &scalar_out, 16);

    try testing.expectEqual(scalar_n, simd_n);

    var i: u32 = 0;
    while (i < simd_n) : (i += 1) {
        try testing.expectEqual(scalar_out[i].offset, simd_out[i].offset);
        try testing.expectEqual(scalar_out[i].text_offset, simd_out[i].text_offset);
        try testing.expectEqual(scalar_out[i].text_length, simd_out[i].text_length);
        try testing.expectEqual(scalar_out[i].target_offset, simd_out[i].target_offset);
        try testing.expectEqual(scalar_out[i].target_length, simd_out[i].target_length);
        try testing.expectEqual(scalar_out[i].link_type, simd_out[i].link_type);
    }
}

// ============================================================================
// Benchmark
// ============================================================================

test "bench_simd_vs_scalar" {
    // Generate ~10KB of markdown with sparse links
    const line = "This is a regular line of markdown text that contains no links at all.\n";
    const md_link = "Check out [this example](https://example.com/path) for details.\n";
    const wiki_link = "See also [[Related Page|related info]] for more.\n";
    comptime var markdown: []const u8 = "";
    comptime {
        var i = 0;
        while (i < 143) : (i += 1) {
            if (i % 15 == 0) {
                markdown = markdown ++ md_link;
            } else if (i % 15 == 7) {
                markdown = markdown ++ wiki_link;
            } else {
                markdown = markdown ++ line;
            }
        }
    }
    const len: u32 = @intCast(markdown.len);
    var out: [64]LinkScan = undefined;

    // Warm up
    _ = scan_links(markdown.ptr, len, &out, 64);
    _ = ref.scan_links_scalar(markdown.ptr, len, &out, 64);

    const iterations: u32 = 1000;
    var timer = std.time.Timer.start() catch return;

    // Benchmark SIMD
    timer.reset();
    var i: u32 = 0;
    while (i < iterations) : (i += 1) {
        _ = scan_links(markdown.ptr, len, &out, 64);
    }
    const simd_ns = timer.read();

    // Benchmark scalar
    timer.reset();
    i = 0;
    while (i < iterations) : (i += 1) {
        _ = ref.scan_links_scalar(markdown.ptr, len, &out, 64);
    }
    const scalar_ns = timer.read();

    std.debug.print("\n[bench] {d}KB markdown, {d} iterations\n", .{ len / 1024, iterations });
    std.debug.print("[bench] SIMD:   {d}ns total, {d}ns/iter\n", .{ simd_ns, simd_ns / iterations });
    std.debug.print("[bench] Scalar: {d}ns total, {d}ns/iter\n", .{ scalar_ns, scalar_ns / iterations });
    if (scalar_ns > 0) {
        std.debug.print("[bench] Speedup: {d:.1}x\n", .{@as(f64, @floatFromInt(scalar_ns)) / @as(f64, @floatFromInt(simd_ns))});
    }

    // Both should find the same number of links
    const simd_count = scan_links(markdown.ptr, len, &out, 64);
    const scalar_count = ref.scan_links_scalar(markdown.ptr, len, &out, 64);
    try testing.expectEqual(scalar_count, simd_count);
}
