// Tests for ExtractionRenderer — extracted from extraction_renderer.zig
// to keep the production module under 1000 lines.

const std = @import("std");
const testing = std.testing;
const extraction_renderer = @import("./extraction_renderer.zig");
const extractFromMarkdown = extraction_renderer.extractFromMarkdown;

// --- Heading tests ---

test "extract ATX heading level 1" {
    const input = "# Hello\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Hello", result.headings[0].text);
    try testing.expectEqual(@as(u8, 1), result.headings[0].level);
    try testing.expectEqual(@as(u32, 0), result.headings[0].offset);
}

test "extract ATX headings levels 1 through 6" {
    const input = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 6), result.headings.len);
    for (result.headings, 0..) |h, i| {
        try testing.expectEqual(@as(u8, @intCast(i + 1)), h.level);
    }
    try testing.expectEqualStrings("H1", result.headings[0].text);
    try testing.expectEqualStrings("H6", result.headings[5].text);
}

test "extract ATX heading byte offset after text" {
    const input = "Some text\n\n# Heading\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    // '#' starts at byte 11 ("Some text\n\n" = 10 bytes, then '#')
    try testing.expectEqual(@as(u32, 11), result.headings[0].offset);
    try testing.expect(input[result.headings[0].offset] == '#');
}

test "extract setext heading level 1" {
    const input = "Hello\n=====\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Hello", result.headings[0].text);
    try testing.expectEqual(@as(u8, 1), result.headings[0].level);
    try testing.expectEqual(@as(u32, 0), result.headings[0].offset);
}

test "extract setext heading level 2" {
    const input = "Hello\n-----\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Hello", result.headings[0].text);
    try testing.expectEqual(@as(u8, 2), result.headings[0].level);
}

test "heading with inline formatting" {
    const input = "# Hello **bold** world\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    // md4c strips inline markup; text callback gets decoded text
    try testing.expectEqualStrings("Hello bold world", result.headings[0].text);
}

test "duplicate headings get distinct offsets" {
    const input = "# Same\n\n# Same\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.headings.len);
    try testing.expectEqualStrings("Same", result.headings[0].text);
    try testing.expectEqualStrings("Same", result.headings[1].text);
    try testing.expect(result.headings[1].offset > result.headings[0].offset);
}

test "heading in blockquote" {
    const input = "> # Quoted\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Quoted", result.headings[0].text);
    try testing.expectEqual(@as(u8, 1), result.headings[0].level);
    // '#' is at position 2 (after "> ")
    try testing.expect(input[result.headings[0].offset] == '#');
}

test "empty heading" {
    const input = "# \n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqual(@as(u8, 1), result.headings[0].level);
}

// --- Link tests ---

test "extract inline link" {
    const input = "[click](https://example.com)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("click", result.links[0].text);
    try testing.expectEqualStrings("https://example.com", result.links[0].target);
    try testing.expectEqual(false, result.links[0].is_wiki);
}

test "extract inline link byte offset" {
    const input = "Hello [click](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expect(input[result.links[0].offset] == '[');
}

test "extract autolink" {
    const input = "<https://example.com>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("https://example.com", result.links[0].text);
}

test "extract reference link" {
    const input = "[text][ref]\n\n[ref]: https://example.com\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("text", result.links[0].text);
}

test "image not extracted as link" {
    const input = "![alt](img.png)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.links.len);
}

test "link inside heading" {
    const input = "# See [here](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("See here", result.headings[0].text);
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("here", result.links[0].text);
}

test "link inside heading has correct offsets" {
    // Regression: shared scan_cursor caused heading offset to be corrupted when
    // finalizeLink (called first) advanced the cursor past the link syntax.
    // "# See [here](url)\n": '#' at byte 0, '[' at byte 6, end of [here](url) at byte 17.
    const input = "# See [here](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqual(@as(u32, 0), result.headings[0].offset); // '#' is at byte 0
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(@as(u32, 6), result.links[0].offset); // '[' is at byte 6
    try testing.expectEqual(@as(u32, 17), result.links[0].end_offset); // past ')' at byte 16
}

test "wiki link inside heading has correct offsets" {
    // "# See [[target]]\n": '#' at byte 0, '[[' at byte 6, past ']]' at byte 17.
    const input = "# See [[target]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqual(@as(u32, 0), result.headings[0].offset);
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(@as(u32, 6), result.links[0].offset);
}

test "autolink inside heading has correct offsets" {
    // "# See <https://x.com>\n": '#' at byte 0, '<' at byte 6.
    const input = "# See <https://x.com>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqual(@as(u32, 0), result.headings[0].offset);
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(@as(u32, 6), result.links[0].offset);
}

// --- Wiki link tests ---

test "extract wiki link" {
    const input = "[[Target]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(true, result.links[0].is_wiki);
    try testing.expectEqualStrings("Target", result.links[0].target);
}

test "extract wiki link with alias" {
    const input = "[[Target|Display]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(true, result.links[0].is_wiki);
    try testing.expectEqualStrings("Target", result.links[0].target);
    try testing.expectEqualStrings("Display", result.links[0].text);
}

test "extract wiki link byte offset" {
    const input = "Text [[Target]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expect(input[result.links[0].offset] == '[');
    try testing.expect(input[result.links[0].offset + 1] == '[');
}

// --- Code block exclusion tests ---

test "heading in code block not extracted" {
    const input = "```\n# Not a heading\n```\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.headings.len);
}

test "link in code block not extracted" {
    const input = "```\n[not](a-link)\n```\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.links.len);
}

// --- Mixed document test ---

test "mixed document: headings, links, wiki links" {
    const input =
        \\# Title
        \\
        \\Some [link](url) text.
        \\
        \\## Section
        \\
        \\See [[Wiki Page]] for details.
        \\
    ;
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.headings.len);
    try testing.expectEqualStrings("Title", result.headings[0].text);
    try testing.expectEqualStrings("Section", result.headings[1].text);

    try testing.expectEqual(@as(usize, 2), result.links.len);
    try testing.expectEqual(false, result.links[0].is_wiki);
    try testing.expectEqual(true, result.links[1].is_wiki);

    // Offsets should be ascending
    try testing.expect(result.headings[1].offset > result.headings[0].offset);
}

// --- Edge case tests ---

test "empty input" {
    var result = try extractFromMarkdown("", testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.headings.len);
    try testing.expectEqual(@as(usize, 0), result.links.len);
}

test "no headings or links" {
    var result = try extractFromMarkdown("Just plain text.\n", testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.headings.len);
    try testing.expectEqual(@as(usize, 0), result.links.len);
}

test "entity in heading decoded" {
    const input = "# Hello &amp; World\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    // Entity references should be decoded to their UTF-8 representation
    try testing.expectEqualStrings("Hello & World", result.headings[0].text);
    // Offset should point to '#'
    try testing.expect(input[result.headings[0].offset] == '#');
}

test "numeric entity in heading decoded" {
    const input = "# A &#38; B\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("A & B", result.headings[0].text);
}

test "hex entity in heading decoded" {
    const input = "# &#x3C;tag&#x3E;\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("<tag>", result.headings[0].text);
}

test "entity in link text decoded" {
    const input = "[A &amp; B](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("A & B", result.links[0].text);
}

test "multiple entities in heading decoded" {
    const input = "# &lt;div&gt; &amp; &quot;test&quot;\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("<div> & \"test\"", result.headings[0].text);
}

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

// --- Code span tests (marky-pdyo) ---

test "code_span_basic: single backtick code span" {
    // "here is `hello` world\n"
    // offset of opening backtick: 8
    const input = "here is `hello` world\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    try testing.expectEqualStrings("hello", result.code_spans[0].text);
    try testing.expectEqual(@as(u32, 8), result.code_spans[0].offset);
    try testing.expect(input[result.code_spans[0].offset] == '`');
    // end_offset past closing backtick: 8 + 1(`hello`) -> backtick at 14, past it = 15
    try testing.expectEqual(@as(u32, 15), result.code_spans[0].end_offset);
}

test "code_span_double_backtick: double backtick delimiters" {
    // "``code with `backtick``` "
    const input = "``code with `backtick``\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    try testing.expectEqualStrings("code with `backtick", result.code_spans[0].text);
    try testing.expectEqual(@as(u32, 0), result.code_spans[0].offset);
    try testing.expect(input[result.code_spans[0].offset] == '`');
}

test "code_span_in_heading: code span inside heading" {
    // "# Title `code`\n" — heading text should include code span text
    const input = "# Title `code`\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Title code", result.headings[0].text);
    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    try testing.expectEqualStrings("code", result.code_spans[0].text);
    try testing.expectEqual(@as(u32, 8), result.code_spans[0].offset);
}

test "code_span_in_link: code span inside link text" {
    // "[`code`](url)\n" — link text includes code span text
    const input = "[`code`](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("code", result.links[0].text);
    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    try testing.expectEqualStrings("code", result.code_spans[0].text);
}

test "code_span_in_fenced_block_not_extracted" {
    // Code spans inside fenced code blocks are NOT inline code spans —
    // md4c does not fire SpanType.code inside fenced blocks.
    const input = "```\nsome `code` here\n```\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.code_spans.len);
}

test "code_span_single_space: minimal code span content" {
    // "` `\n" — code span with single space (md4c normalizes whitespace)
    // This is the smallest valid code span in CommonMark.
    const input = "` `\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    try testing.expectEqual(@as(u32, 0), result.code_spans[0].offset);
    try testing.expect(input[result.code_spans[0].offset] == '`');
}

test "code_span_multiple: two code spans in order" {
    // "`a` then `b`\n"
    // `a` at offset 0, `b` at offset 10
    const input = "`a` then `b`\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.code_spans.len);
    try testing.expectEqualStrings("a", result.code_spans[0].text);
    try testing.expectEqualStrings("b", result.code_spans[1].text);
    // Offsets must be ascending
    try testing.expect(result.code_spans[1].offset > result.code_spans[0].offset);
    try testing.expectEqual(@as(u32, 0), result.code_spans[0].offset);
    try testing.expectEqual(@as(u32, 9), result.code_spans[1].offset);
}

test "code_span_entity_decoded: entity inside code span" {
    // "`a &amp; b`\n" — entity should be decoded to "a & b"
    const input = "`a &amp; b`\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    // Note: md4c may or may not decode entities inside code spans.
    // In CommonMark, code spans are verbatim — entities are NOT decoded.
    // md4c fires TextType.code (not .entity) for code span content.
    // So the text should be the raw content: "a &amp; b"
    try testing.expectEqualStrings("a &amp; b", result.code_spans[0].text);
}

test "code_span_interleaved_with_heading_and_link: all offsets correct" {
    // "# Title `code` [link](url)\n" — heading, code span, and link coexist
    const input = "# Title `code` [link](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(@as(usize, 1), result.code_spans.len);

    // All offsets should be valid and point to correct characters
    try testing.expect(input[result.headings[0].offset] == '#');
    try testing.expect(input[result.code_spans[0].offset] == '`');
    try testing.expect(input[result.links[0].offset] == '[');

    // Offsets ascending: heading < code_span < link
    try testing.expect(result.code_spans[0].offset > result.headings[0].offset);
    try testing.expect(result.links[0].offset > result.code_spans[0].offset);
}
