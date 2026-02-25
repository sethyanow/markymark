// Tests for ExtractionRenderer — offset regressions, OOM handling, end_offset accuracy,
// and fence tracking (lzd5). Split from extraction_renderer_tests.zig.

const std = @import("std");
const testing = std.testing;
const extraction_renderer = @import("./extraction_renderer.zig");
const extractFromMarkdown = extraction_renderer.extractFromMarkdown;

// --- T1-5 regression: offset correctness with code fences and mid-line markers ---

test "T1-5: ATX heading offset not corrupted by hash inside fenced code block" {
    // "```\n# not a heading\n```\n\n# Real Heading\n"
    // byte layout: "```\n"(4) + "# not a heading\n"(16) + "```\n"(4) + "\n"(1) = 25
    // "# Real Heading" starts at byte 25
    const input = "```\n# not a heading\n```\n\n# Real Heading\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Real Heading", result.headings[0].text);
    // offset must point into "# Real Heading", not the fake one inside the fence
    try testing.expectEqual(@as(u32, 25), result.headings[0].offset);
    try testing.expect(input[result.headings[0].offset] == '#');
}

test "T1-5: link offset not corrupted by bracket inside fenced code block" {
    // "```\n[not a link](url)\n```\n\n[real link](url)\n"
    // byte layout: "```\n"(4) + "[not a link](url)\n"(18) + "```\n"(4) + "\n"(1) = 27
    // "[real link](url)" starts at byte 27
    const input = "```\n[not a link](url)\n```\n\n[real link](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("real link", result.links[0].text);
    try testing.expectEqual(@as(u32, 27), result.links[0].offset);
    try testing.expect(input[result.links[0].offset] == '[');
}

test "T1-5: mid-line hash not treated as ATX heading offset" {
    // "Some text # not-a-heading\n# Real Heading\n"
    // "Some text # not-a-heading\n" = 26 bytes; '#' of heading at 26
    const input = "Some text # not-a-heading\n# Real Heading\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Real Heading", result.headings[0].text);
    try testing.expectEqual(@as(u32, 26), result.headings[0].offset);
    try testing.expect(input[result.headings[0].offset] == '#');
}

// --- T1-4 regression: OOM during rendering propagated, not swallowed ---

test "T1-4: OOM from parser is propagated as error.OutOfMemory" {
    // Use a FailingAllocator that fails immediately (fail_index=0).
    // Parser init allocations fail → renderWithRenderer returns OutOfMemory.
    // Verifies that the error pathway from renderWithRenderer is intact.
    // (Callback-phase OOM is handled by the oom flag; parser-phase OOM by this path.)
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    var failing = std.testing.FailingAllocator.init(gpa.allocator(), .{ .fail_index = 0 });
    const input = "# Hello World\n";
    const result = extractFromMarkdown(input, failing.allocator());
    try testing.expectError(error.OutOfMemory, result);
}

// --- end_offset accuracy for all link syntaxes ---

test "extract inline link end_offset" {
    // [Hello](world) = 14 chars; scan_cursor lands past ')'
    const input = "[Hello](world)";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(@as(u32, 0), result.links[0].offset);
    try testing.expectEqual(@as(u32, 14), result.links[0].end_offset);
}

test "extract reference link end_offset" {
    // [Hello][ref] = 12 chars; scan_cursor lands past second ']'
    // Previously the heuristic used text_len+target_len+4, giving a large wrong value
    // because target gets resolved to the full URL, not "ref".
    const input = "[Hello][ref]\n\n[ref]: https://example.com\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(@as(u32, 0), result.links[0].offset);
    try testing.expectEqual(@as(u32, 12), result.links[0].end_offset);
}

test "extract autolink end_offset" {
    // <https://example.com> = 21 chars; scan_cursor lands past '>'
    const input = "<https://example.com>";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(@as(u32, 0), result.links[0].offset);
    try testing.expectEqual(@as(u32, 21), result.links[0].end_offset);
}

test "extract wiki link end_offset" {
    // [[target]] = 10 chars; scan_cursor lands past ']]'
    const input = "[[target]]";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(@as(u32, 0), result.links[0].offset);
    try testing.expectEqual(@as(u32, 10), result.links[0].end_offset);
}

test "processLeafBlock multi-line setext heading merges lines correctly" {
    // Setext headings have 2+ block_lines: the text line(s) and the underline.
    // processLeafBlock merges them with '\n' via buffer.append/appendSlice.
    // Previously, catch {} silently swallowed OOM on those appends; now try propagates.
    // This test verifies correct behavior on the success path (no OOM).
    const input = "Multi Line Heading\n==================\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Multi Line Heading", result.headings[0].text);
}

// --- marky-gmny: OOM-loop double-free and leak regression ---

test "marky-gmny: extractFromMarkdown OOM loop — no double-free or leak" {
    // Exercises every OOM failure point in extractFromMarkdown by iterating
    // fail_index from 0..N. At each index, exactly one allocation fails.
    // GPA detects double-free (fills freed memory with 0xaa → segfault).
    // GPA leak check (.check() returning .leak) detects missing frees.
    //
    // Uses a document with headings, links, and code spans so that the
    // partially-transferred ownership paths (toOwnedSlice cascade) are
    // exercised at various fail_index values.
    const input = "# Heading One\n\n[Link Text](https://example.com)\n\n## Heading `code` Two\n";

    var fail_index: usize = 0;
    // Upper bound: enough to cover all allocation sites. If we get 5
    // consecutive successes, all failure points have been covered.
    var consecutive_successes: usize = 0;
    while (consecutive_successes < 5) : (fail_index += 1) {
        // Safety valve: prevent infinite loop if something is very wrong
        if (fail_index > 200) break;

        var gpa = std.heap.GeneralPurposeAllocator(.{}){};
        var failing = std.testing.FailingAllocator.init(gpa.allocator(), .{ .fail_index = fail_index });

        const result = extractFromMarkdown(input, failing.allocator());
        if (result) |*ok| {
            // Success path: must have valid data, free it
            var r = ok.*;
            r.deinit();
            consecutive_successes += 1;
        } else |err| {
            // Error path: must be OutOfMemory, nothing else
            try testing.expectEqual(error.OutOfMemory, err);
            consecutive_successes = 0;
        }

        // GPA leak check: .ok means no leaks, .leak means memory leaked
        const check = gpa.deinit();
        try testing.expect(check == .ok);
    }

    // Verify we actually tested multiple failure points (not just index 0)
    try testing.expect(fail_index > 5);
}

// --- marky-lzd5: offset scan hardening tests ---

test "lzd5-F1: backtick fence not closed by tilde line — ATX heading" {
    // ``` opens fence, ~~~ should NOT close it (different char), then ``` closes it.
    // # Real Heading should get correct offset.
    // "```\n# fake\n~~~\n# also fake\n```\n\n# Real Heading\n"
    // bytes: "```\n"(4) "# fake\n"(7) "~~~\n"(4) "# also fake\n"(12) "```\n"(4) "\n"(1) = 32
    const input = "```\n# fake\n~~~\n# also fake\n```\n\n# Real Heading\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Real Heading", result.headings[0].text);
    try testing.expectEqual(@as(u32, 32), result.headings[0].offset);
    try testing.expect(input[result.headings[0].offset] == '#');
}

test "lzd5-F2: tilde fence not closed by backtick line — ATX heading" {
    // ~~~ opens fence, ``` should NOT close it (different char), then ~~~ closes it.
    // "~~~\n# fake\n```\n# also fake\n~~~\n\n# Real Heading\n"
    // bytes: "~~~\n"(4) "# fake\n"(7) "```\n"(4) "# also fake\n"(12) "~~~\n"(4) "\n"(1) = 32
    const input = "~~~\n# fake\n```\n# also fake\n~~~\n\n# Real Heading\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Real Heading", result.headings[0].text);
    try testing.expectEqual(@as(u32, 32), result.headings[0].offset);
    try testing.expect(input[result.headings[0].offset] == '#');
}

test "lzd5-F3: 4-backtick fence not closed by 3-backtick line" {
    // ```` opens fence (4 chars), ``` should NOT close it (shorter), then ```` closes it.
    // "````\n# fake\n```\n# also fake\n````\n\n# Real Heading\n"
    // bytes: "````\n"(5) "# fake\n"(7) "```\n"(4) "# also fake\n"(12) "````\n"(5) "\n"(1) = 34
    const input = "````\n# fake\n```\n# also fake\n````\n\n# Real Heading\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Real Heading", result.headings[0].text);
    try testing.expectEqual(@as(u32, 34), result.headings[0].offset);
    try testing.expect(input[result.headings[0].offset] == '#');
}

test "lzd5-F3b: setext heading (level 2) after code block containing --- line" {
    // Code block contains "---" which should NOT be treated as setext underline.
    // After code block, real setext heading (level 2, ---) should get correct offset.
    // "```\nfake\n---\nfake\n```\n\nReal Heading\n---\n"
    // bytes: "```\n"(4) "fake\n"(5) "---\n"(4) "fake\n"(5) "```\n"(4) "\n"(1) = 23
    // "Real Heading" starts at 23
    const input = "```\nfake\n---\nfake\n```\n\nReal Heading\n---\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Real Heading", result.headings[0].text);
    try testing.expectEqual(@as(u32, 23), result.headings[0].offset);
    try testing.expect(input[result.headings[0].offset] == 'R');
}

test "lzd5-F4a: link with parenthesized URL (Wikipedia style)" {
    // [link](https://en.wikipedia.org/wiki/Foo_(bar))
    // The URL contains (bar) — naive ) scan truncates.
    const input = "[link](https://en.wikipedia.org/wiki/Foo_(bar))";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("link", result.links[0].text);
    // end_offset should cover the entire construct including (bar))
    try testing.expectEqual(@as(u32, 47), result.links[0].end_offset);
}

test "lzd5-F4b: link with nested parentheses in URL" {
    // [link](url(a(b)))
    // 0123456789012345678
    // len = 17
    const input = "[link](url(a(b)))";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("link", result.links[0].text);
    try testing.expectEqual(@as(u32, 17), result.links[0].end_offset);
}

test "lzd5-F4c: link with escaped parentheses in URL" {
    // [link](url\(not-paren\))
    // Escaped parens should not be counted. end_offset covers everything.
    // [  l  i  n  k  ]  (  u  r  l  \  (  n  o  t  -  p  a  r  e  n  \  )  )
    // 0  1  2  3  4  5  6  7  8  9  10 11 12 13 14 15 16 17 18 19 20 21 22 23
    // len = 24
    const input = "[link](url\\(not-paren\\))";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("link", result.links[0].text);
    try testing.expectEqual(@as(u32, 24), result.links[0].end_offset);
}

test "lzd5-F2b: backtick fence not closed by tilde line — link scan" {
    // Same as F1 but for links: ``` fence should not be closed by ~~~
    // "```\n[fake](url)\n~~~\n[also fake](url)\n```\n\n[real](url)\n"
    // bytes: "```\n"(4) "[fake](url)\n"(12) "~~~\n"(4) "[also fake](url)\n"(17) "```\n"(4) "\n"(1) = 42
    const input = "```\n[fake](url)\n~~~\n[also fake](url)\n```\n\n[real](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("real", result.links[0].text);
    try testing.expectEqual(@as(u32, 42), result.links[0].offset);
    try testing.expect(input[result.links[0].offset] == '[');
}
