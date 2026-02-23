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

// ── Multiple tags on same line (block-level HTML) ───────────────────

test "xml_tags: inline HTML tags not extracted (not block-level)" {
    // <agent>content</agent> on one line is inline HTML (not block-level).
    // md4c's inline HTML content may point to internal buffers, not source text.
    // These tags are correctly skipped by the block-level HTML extraction pipeline.
    const input = "# Heading\n\n<agent>content</agent>\n\n<goal>win</goal>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    // Inline HTML tags are NOT extracted — only block-level HTML is
    try testing.expectEqual(@as(usize, 0), result.xml_tags.len);
}

test "xml_tags: multiple block-level tags on separate lines" {
    // Tags on their own lines with blank lines = proper block HTML
    const input = "<agent>\n\ncontent\n\n</agent>\n\n<goal>\n\nwin\n\n</goal>\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.xml_tags.len);
    try testing.expectEqualStrings("agent", result.xml_tags[0].tag_name);
    try testing.expectEqualStrings("goal", result.xml_tags[1].tag_name);
}

// ── Investigation: md4c inline HTML callback behavior ────────────────
//
// These tests verify md4c's behavior for inline HTML to inform the design
// of inline XML tag extraction (marky-s64n). They use a spy renderer to
// observe raw text callbacks.

const root = @import("root.zig");
const parser_mod = @import("parser.zig");
const types = @import("types.zig");

/// Spy renderer that captures TextType.html callbacks with pointer metadata.
/// Content is duped so it survives parser cleanup (md4c internal buffers freed on deinit).
const HtmlSpy = struct {
    const Entry = struct {
        content: []const u8, // owned copy
        points_to_source: bool,
    };

    entries: std.ArrayListUnmanaged(Entry) = .{},
    allocator: std.mem.Allocator,
    src_text: []const u8,

    fn init(allocator: std.mem.Allocator, src_text: []const u8) HtmlSpy {
        return .{
            .allocator = allocator,
            .src_text = src_text,
        };
    }

    fn deinit(self: *HtmlSpy) void {
        for (self.entries.items) |entry| {
            self.allocator.free(entry.content);
        }
        self.entries.deinit(self.allocator);
    }

    fn renderer(self: *HtmlSpy) types.Renderer {
        return .{ .ptr = self, .vtable = &vtable };
    }

    const vtable: types.Renderer.VTable = .{
        .enterBlock = struct {
            fn f(_: *anyopaque, _: types.BlockType, _: u32, _: u32) types.CallbackError!void {}
        }.f,
        .leaveBlock = struct {
            fn f(_: *anyopaque, _: types.BlockType, _: u32) types.CallbackError!void {}
        }.f,
        .enterSpan = struct {
            fn f(_: *anyopaque, _: types.SpanType, _: types.SpanDetail) types.CallbackError!void {}
        }.f,
        .leaveSpan = struct {
            fn f(_: *anyopaque, _: types.SpanType) types.CallbackError!void {}
        }.f,
        .text = textImpl,
    };

    fn textImpl(ptr: *anyopaque, text_type: types.TextType, content: []const u8) types.CallbackError!void {
        if (text_type != .html) return;
        const self: *HtmlSpy = @ptrCast(@alignCast(ptr));
        const src_start = @intFromPtr(self.src_text.ptr);
        const src_end = src_start + self.src_text.len;
        const content_start = @intFromPtr(content.ptr);
        const content_end = content_start + content.len;
        const points_to_source = (content.len > 0 and content_start >= src_start and content_end <= src_end);
        // Dupe content so it survives parser cleanup
        const owned = self.allocator.dupe(u8, content) catch return error.OutOfMemory;
        self.entries.append(self.allocator, .{
            .content = owned,
            .points_to_source = points_to_source,
        }) catch {
            self.allocator.free(owned);
            return error.OutOfMemory;
        };
    }
};

test "investigation: md4c fires TextType.html for inline HTML" {
    // Inline HTML: <agent>content</agent> within a paragraph
    const input = "Hello <agent>content</agent> world\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // Q1: Does md4c fire TextType.html for inline HTML?
    // If entries.len > 0, md4c does fire html callbacks for inline HTML.
    try testing.expect(spy.entries.items.len > 0);

    // Q2: What content does md4c pass?
    // Log the entries for inspection.
    // Expected: separate callbacks for <agent>, </agent> (not the content between them).
    for (spy.entries.items) |entry| {
        // Q3: Do inline HTML pointers point to source text or md4c internal buffer?
        // The design assumed inline HTML points to internal buffer (points_to_source = false).
        // This test empirically verifies that assumption.
        _ = entry;
    }

    // Verify at least 2 entries (open tag + close tag)
    try testing.expect(spy.entries.items.len >= 2);

    // Verify the content matches expected tag text
    try testing.expectEqualStrings("<agent>", spy.entries.items[0].content);
    try testing.expectEqualStrings("</agent>", spy.entries.items[1].content);
}

test "investigation: inline HTML pointers are NOT in source text range" {
    const input = "Hello <agent>content</agent> world\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // The design states inline HTML content pointers point to md4c's internal buffer,
    // NOT the source text. Verify this critical assumption.
    for (spy.entries.items) |entry| {
        // If this fails (points_to_source = true), then inline HTML CAN be extracted
        // using the existing pointer-bounds approach, and the design needs revision.
        try testing.expect(!entry.points_to_source);
    }
}

test "investigation: block-level HTML pointers ARE in source text range" {
    // Block-level HTML: tag on own line with blank lines around it
    const input = "<agent>\n\ncontent\n\n</agent>\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // Block-level HTML should have at least some pointers into source text
    try testing.expect(spy.entries.items.len > 0);
    var source_count: usize = 0;
    for (spy.entries.items) |entry| {
        if (entry.points_to_source) source_count += 1;
    }
    // At least the tag lines should point to source
    try testing.expect(source_count > 0);
}

test "investigation: inline HTML inside code span NOT fired" {
    // Inline code: `<agent>` — should NOT fire TextType.html
    const input = "Hello `<agent>` world\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // md4c should NOT fire TextType.html for HTML inside code spans.
    // It fires TextType.code instead.
    try testing.expectEqual(@as(usize, 0), spy.entries.items.len);
}

test "investigation: inline HTML inside fenced code block NOT fired" {
    // Fenced code block
    const input = "```\n<agent>content</agent>\n```\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // md4c should NOT fire TextType.html for HTML inside fenced code blocks.
    try testing.expectEqual(@as(usize, 0), spy.entries.items.len);
}

test "investigation: multiple inline tags on same line" {
    const input = "Text <a>one</a> and <b>two</b> end\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // Should fire 4 html callbacks: <a>, </a>, <b>, </b>
    try testing.expectEqual(@as(usize, 4), spy.entries.items.len);
    try testing.expectEqualStrings("<a>", spy.entries.items[0].content);
    try testing.expectEqualStrings("</a>", spy.entries.items[1].content);
    try testing.expectEqualStrings("<b>", spy.entries.items[2].content);
    try testing.expectEqualStrings("</b>", spy.entries.items[3].content);
}

test "investigation: inline HTML callback content detail" {
    // Verify exact content of each inline HTML callback
    const input = "Hello <agent>content</agent> world\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // Verify md4c fires separate callbacks for open and close tags.
    // The content between tags (<agent>content</agent>) is NOT fired as TextType.html —
    // it's fired as TextType.normal.
    try testing.expectEqual(@as(usize, 2), spy.entries.items.len);
    try testing.expectEqualStrings("<agent>", spy.entries.items[0].content);
    try testing.expectEqualStrings("</agent>", spy.entries.items[1].content);

    // Both should be internal buffer pointers (not source text)
    try testing.expect(!spy.entries.items[0].points_to_source);
    try testing.expect(!spy.entries.items[1].points_to_source);
}

test "investigation: block-level HTML callback content includes full lines" {
    // Block-level HTML: lines include newlines
    const input = "<agent>\n\ncontent\n\n</agent>\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // Block-level HTML: md4c fires one callback per line of the HTML block,
    // with the full line content including newline.
    try testing.expect(spy.entries.items.len >= 1);

    // Check that the first entry contains the opening tag line
    var found_open = false;
    var found_close = false;
    for (spy.entries.items) |entry| {
        if (std.mem.indexOf(u8, entry.content, "<agent>") != null) found_open = true;
        if (std.mem.indexOf(u8, entry.content, "</agent>") != null) found_close = true;
    }
    try testing.expect(found_open);
    try testing.expect(found_close);
}

test "investigation: mixed inline and block-level HTML" {
    // Block-level tag first, then paragraph with inline tag
    const input = "<div>\n\nblock content\n\n</div>\n\nParagraph with <span>inline</span> tag\n";

    var spy = HtmlSpy.init(testing.allocator, input);
    defer spy.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, testing.allocator, opts, spy.renderer());

    // Should have both block-level (source pointers) and inline (internal buffer) entries
    var has_source = false;
    var has_internal = false;
    for (spy.entries.items) |entry| {
        if (entry.points_to_source) has_source = true else has_internal = true;
    }
    try testing.expect(has_source); // block-level <div>...</div>
    try testing.expect(has_internal); // inline <span>...</span>
}

// NOTE: DocumentEngine integration and blob serialization tests live in
// engine/document_test.zig (cannot cross module path boundary from md4c/).
