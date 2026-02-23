// Tests for XML tag extraction via the ExtractionRenderer pipeline.
// Covers paired tags, self-closing, void elements, unclosed tags, case insensitivity,
// fenced code block exclusion, HTML comments/PI/CDATA filtering, and FFI roundtrip.

const std = @import("std");
const testing = std.testing;
const extraction_renderer = @import("extraction_renderer.zig");
const extractFromMarkdown = extraction_renderer.extractFromMarkdown;

// ── Basic paired tags ──────────────────────────────────────────────

test "xml_tags: paired custom tag (block-level)" {
    // md4c requires block-level HTML: opening tag on its own line, blank lines around
    const input = "<custom>\n\ncontent\n\n</custom>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("custom", result.xml_tags[0].tag_name);
    try testing.expect(!result.xml_tags[0].is_self_closing);
    try testing.expect(!result.xml_tags[0].is_unclosed);
    // end_offset should include closing tag
    try testing.expect(result.xml_tags[0].end_offset > result.xml_tags[0].offset);
}

test "xml_tags: paired div tag (block-level)" {
    const input = "<div>\n\ntext here\n\n</div>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("div", result.xml_tags[0].tag_name);
    try testing.expect(!result.xml_tags[0].is_self_closing);
    try testing.expect(!result.xml_tags[0].is_unclosed);
}

// ── Self-closing tags ──────────────────────────────────────────────

test "xml_tags: self-closing with slash" {
    const input = "<br/>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("br", result.xml_tags[0].tag_name);
    try testing.expect(result.xml_tags[0].is_self_closing);
    try testing.expect(!result.xml_tags[0].is_unclosed);
}

test "xml_tags: self-closing with space before slash" {
    const input = "<custom />\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("custom", result.xml_tags[0].tag_name);
    try testing.expect(result.xml_tags[0].is_self_closing);
}

// ── Void elements ──────────────────────────────────────────────────

test "xml_tags: void element br without slash" {
    const input = "<br>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("br", result.xml_tags[0].tag_name);
    try testing.expect(result.xml_tags[0].is_self_closing);
}

test "xml_tags: void element hr" {
    const input = "<hr>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("hr", result.xml_tags[0].tag_name);
    try testing.expect(result.xml_tags[0].is_self_closing);
}

test "xml_tags: void element img" {
    const input = "<img src=\"photo.jpg\">\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("img", result.xml_tags[0].tag_name);
    try testing.expect(result.xml_tags[0].is_self_closing);
}

// ── Unclosed tags ──────────────────────────────────────────────────

test "xml_tags: unclosed orphan tag" {
    const input = "<orphan>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("orphan", result.xml_tags[0].tag_name);
    try testing.expect(!result.xml_tags[0].is_self_closing);
    try testing.expect(result.xml_tags[0].is_unclosed);
}

// ── Nested same-name tags ──────────────────────────────────────────

test "xml_tags: nested same-name div tags" {
    const input = "<div>\n\n<div>\n\ninner\n\n</div>\n\n</div>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.xml_tags.len);
    // Both should be paired (not unclosed)
    for (result.xml_tags) |xt| {
        try testing.expectEqualStrings("div", xt.tag_name);
        try testing.expect(!xt.is_self_closing);
        try testing.expect(!xt.is_unclosed);
    }
}

// ── Case insensitivity ─────────────────────────────────────────────

test "xml_tags: case insensitive matching" {
    const input = "<DIV>\n\ntext\n\n</div>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    // Tag name preserved as-is from opening tag
    try testing.expectEqualStrings("DIV", result.xml_tags[0].tag_name);
    try testing.expect(!result.xml_tags[0].is_unclosed);
}

// ── HTML comments, PI, CDATA, DOCTYPE (should be ignored) ──────────

test "xml_tags: HTML comment ignored" {
    const input = "<!-- this is a comment -->\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.xml_tags.len);
}

test "xml_tags: processing instruction ignored" {
    const input = "<?xml version=\"1.0\"?>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.xml_tags.len);
}

// ── Tags inside fenced code blocks (should be excluded) ────────────

test "xml_tags: tags inside fenced code block excluded" {
    const input = "```\n<custom>not a tag</custom>\n```\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.xml_tags.len);
}

// ── Attributes preserved in raw_html ───────────────────────────────

test "xml_tags: raw_html includes attributes" {
    const input = "<tag attr=\"val\" class=\"foo\">\n\ncontent\n\n</tag>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("tag", result.xml_tags[0].tag_name);
    // raw_html should contain the full opening tag text
    try testing.expect(result.xml_tags[0].raw_html.len > 0);
    try testing.expect(std.mem.startsWith(u8, result.xml_tags[0].raw_html, "<tag"));
}

// ── Empty document ─────────────────────────────────────────────────

test "xml_tags: empty document produces no tags" {
    const input = "";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.xml_tags.len);
}

// ── Mixed with other markdown elements ─────────────────────────────

test "xml_tags: mixed with headings and links" {
    // Block-level HTML (blank lines around) coexists with other markdown
    const input = "# Heading\n\n<custom>\n\ntext\n\n</custom>\n\n[Link](url)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("custom", result.xml_tags[0].tag_name);
    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqual(@as(usize, 1), result.links.len);
}

// ── Block-level HTML extraction ────────────────────────────────────

test "xml_tags: block-level div extracted" {
    const input = "<div>\n\nparagraph text\n\n</div>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.xml_tags.len);
    try testing.expectEqualStrings("div", result.xml_tags[0].tag_name);
    try testing.expect(!result.xml_tags[0].is_self_closing);
    try testing.expect(!result.xml_tags[0].is_unclosed);
}

// ── FFI roundtrip via marky_md4c_extract ───────────────────────────

test "xml_tags: FFI roundtrip via marky_md4c_extract" {
    const exports = @import("exports.zig");
    const CMd4cResult = exports.CMd4cResult;

    const input = "<custom attr=\"val\">\n\ncontent\n\n</custom>\n";
    var result: CMd4cResult = undefined;
    const rc = exports.marky_md4c_extract(input.ptr, @intCast(input.len), &result);
    try testing.expectEqual(@as(i32, 0), rc);
    defer exports.marky_md4c_free(&result);

    try testing.expectEqual(@as(u32, 1), result.xml_tags_count);
    const xt = result.xml_tags.?[0];

    // Verify tag name can be extracted from text_blob
    const tag_name = result.text_blob.?[xt.tag_name_offset..][0..xt.tag_name_length];
    try testing.expectEqualStrings("custom", tag_name);

    // Verify raw_html starts with opening tag
    const raw_html = result.text_blob.?[xt.raw_html_offset..][0..xt.raw_html_length];
    try testing.expect(std.mem.startsWith(u8, raw_html, "<custom"));

    // Verify flags
    try testing.expectEqual(@as(u8, 0), xt.is_self_closing);
    try testing.expectEqual(@as(u8, 0), xt.is_unclosed);
}

// NOTE: DocumentEngine integration and blob serialization tests live in
// engine/document_test.zig (cannot cross module path boundary from md4c/).
