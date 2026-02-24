// Tests for ExtractionRenderer — code span extraction (marky-pdyo).
// Split from extraction_renderer_tests.zig to keep files under 1000 lines.

const std = @import("std");
const testing = std.testing;
const extraction_renderer = @import("./extraction_renderer.zig");
const extractFromMarkdown = extraction_renderer.extractFromMarkdown;

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

test "code_span_after_fenced_block: offset skips fence delimiters (marky-4tqn)" {
    // Regression: findCodeSpanOffset must skip fenced code block delimiters.
    // Without fix, scanner matches fence triple-backticks as the code span pair.
    // "```\ncode\n```\n\nText `inline` here\n"
    // Byte layout:
    //   "```\n"     = 0..4
    //   "code\n"    = 4..9
    //   "```\n"     = 9..13
    //   "\n"        = 13..14
    //   "Text "     = 14..19
    //   "`inline`"  = 19..27  (opening backtick at 19)
    //   " here\n"   = 27..33
    const input = "```\ncode\n```\n\nText `inline` here\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    try testing.expectEqualStrings("inline", result.code_spans[0].text);
    try testing.expectEqual(@as(u32, 19), result.code_spans[0].offset);
    try testing.expect(input[result.code_spans[0].offset] == '`');
}

test "code_span_single_space: minimal code span content" {
    // "` `\n" — code span with single space (md4c normalizes whitespace)
    // This is the smallest valid code span in CommonMark.
    const input = "` `\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    try testing.expectEqualStrings(" ", result.code_spans[0].text);
    try testing.expectEqual(@as(u32, 0), result.code_spans[0].offset);
}

test "code_span_multiple: two code spans in order" {
    // "`first` and `second`\n"
    // offset 0: `first` (0..7), offset 12: `second` (12..20)
    const input = "`first` and `second`\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.code_spans.len);
    try testing.expectEqualStrings("first", result.code_spans[0].text);
    try testing.expectEqual(@as(u32, 0), result.code_spans[0].offset);
    try testing.expectEqualStrings("second", result.code_spans[1].text);
    try testing.expectEqual(@as(u32, 12), result.code_spans[1].offset);
}

test "code_span_entity_decoded: entity inside code span" {
    // md4c fires TextType.code (not .entity) for code span content,
    // so entities should NOT be decoded inside code spans per CommonMark spec.
    const input = "`&amp;`\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    // Code spans are verbatim — entity should NOT be decoded
    try testing.expectEqualStrings("&amp;", result.code_spans[0].text);
}

test "code_span_interleaved_with_heading_and_link: all offsets correct" {
    // "# Title\n\n`code` and [link](url)\n"
    // Heading: "Title" at offset 0 (the '#')
    // Code span: "code" at offset 10 (the '`' after newlines)
    // Link: "link" at offset 18 (the '[')
    const input = "# Title\n\n`code` and [link](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqualStrings("Title", result.headings[0].text);

    try testing.expectEqual(@as(usize, 1), result.code_spans.len);
    try testing.expectEqualStrings("code", result.code_spans[0].text);
    try testing.expectEqual(@as(u32, 9), result.code_spans[0].offset);
    try testing.expect(input[result.code_spans[0].offset] == '`');

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqualStrings("link", result.links[0].text);
    try testing.expect(input[result.links[0].offset] == '[');

    // Offsets ascending: heading < code_span < link
    try testing.expect(result.code_spans[0].offset > result.headings[0].offset);
    try testing.expect(result.links[0].offset > result.code_spans[0].offset);
}
