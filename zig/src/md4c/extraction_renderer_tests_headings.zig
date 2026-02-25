// Tests for ExtractionRenderer — headings, links, wiki links, entities, and mixed documents.
// Split from extraction_renderer_tests.zig to keep files under 1000 lines.

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
