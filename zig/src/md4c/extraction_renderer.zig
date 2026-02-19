// ExtractionRenderer: single-pass md4c Renderer that extracts headings, links,
// and wiki links with byte offsets.  Created for marky-v03o.

const std = @import("std");
const Allocator = std.mem.Allocator;

const types = @import("./types.zig");
const BlockType = types.BlockType;
const SpanType = types.SpanType;
const TextType = types.TextType;
const Renderer = types.Renderer;
const SpanDetail = types.SpanDetail;
const CallbackError = types.CallbackError;

const helpers = @import("./helpers.zig");
const root = @import("./root.zig");
const parser_mod = @import("./parser.zig");

// ── Result types ─────────────────────────────────────────────────────

pub const ExtractedHeading = struct {
    text: []const u8, // owned
    offset: u32,
    level: u8,
};

pub const ExtractedLink = struct {
    text: []const u8, // owned
    target: []const u8, // owned
    offset: u32,
    is_wiki: bool,
};

pub const ExtractionResult = struct {
    headings: []ExtractedHeading,
    links: []ExtractedLink,
    allocator: Allocator,

    pub fn deinit(self: *ExtractionResult) void {
        for (self.headings) |h| {
            self.allocator.free(h.text);
        }
        self.allocator.free(self.headings);
        for (self.links) |l| {
            self.allocator.free(l.text);
            self.allocator.free(l.target);
        }
        self.allocator.free(self.links);
    }
};

// ── ExtractionRenderer ───────────────────────────────────────────────

pub const ExtractionRenderer = struct {
    src_text: []const u8,
    allocator: Allocator,

    headings: std.ArrayListUnmanaged(ExtractedHeading) = .{},
    links: std.ArrayListUnmanaged(ExtractedLink) = .{},

    scan_cursor: u32 = 0,

    // heading accumulation state
    in_heading: bool = false,
    heading_level: u8 = 0,
    heading_is_setext: bool = false,
    heading_text_buf: std.ArrayListUnmanaged(u8) = .{},

    // link accumulation state
    in_link: bool = false,
    in_image: bool = false,
    link_is_wiki: bool = false,
    link_is_autolink: bool = false,
    link_text_buf: std.ArrayListUnmanaged(u8) = .{},
    link_href_buf: std.ArrayListUnmanaged(u8) = .{},

    // code block tracking
    in_code_block: bool = false,

    pub fn init(allocator: Allocator, src_text: []const u8) ExtractionRenderer {
        return .{
            .src_text = src_text,
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *ExtractionRenderer) void {
        // Free any partially accumulated state (errdefer path)
        self.heading_text_buf.deinit(self.allocator);
        self.link_text_buf.deinit(self.allocator);
        self.link_href_buf.deinit(self.allocator);

        // Free owned strings in results
        for (self.headings.items) |h| {
            self.allocator.free(h.text);
        }
        self.headings.deinit(self.allocator);
        for (self.links.items) |l| {
            self.allocator.free(l.text);
            self.allocator.free(l.target);
        }
        self.links.deinit(self.allocator);
    }

    pub fn renderer(self: *ExtractionRenderer) Renderer {
        return .{ .ptr = self, .vtable = &vtable };
    }

    pub const vtable: Renderer.VTable = .{
        .enterBlock = enterBlockImpl,
        .leaveBlock = leaveBlockImpl,
        .enterSpan = enterSpanImpl,
        .leaveSpan = leaveSpanImpl,
        .text = textImpl,
    };

    // ── VTable trampoline functions ──────────────────────────────────

    fn enterBlockImpl(ptr: *anyopaque, block_type: BlockType, data: u32, flags: u32) CallbackError!void {
        const self: *ExtractionRenderer = @ptrCast(@alignCast(ptr));
        self.enterBlock(block_type, data, flags);
    }

    fn leaveBlockImpl(ptr: *anyopaque, block_type: BlockType, _: u32) CallbackError!void {
        const self: *ExtractionRenderer = @ptrCast(@alignCast(ptr));
        self.leaveBlock(block_type);
    }

    fn enterSpanImpl(ptr: *anyopaque, span_type: SpanType, detail: SpanDetail) CallbackError!void {
        const self: *ExtractionRenderer = @ptrCast(@alignCast(ptr));
        self.enterSpan(span_type, detail);
    }

    fn leaveSpanImpl(ptr: *anyopaque, span_type: SpanType) CallbackError!void {
        const self: *ExtractionRenderer = @ptrCast(@alignCast(ptr));
        self.leaveSpan(span_type);
    }

    fn textImpl(ptr: *anyopaque, text_type: TextType, content: []const u8) CallbackError!void {
        const self: *ExtractionRenderer = @ptrCast(@alignCast(ptr));
        self.text(text_type, content);
    }

    // ── Block callbacks ──────────────────────────────────────────────

    fn enterBlock(self: *ExtractionRenderer, block_type: BlockType, data: u32, flags: u32) void {
        switch (block_type) {
            .h => {
                self.in_heading = true;
                self.heading_level = @truncate(data);
                self.heading_is_setext = (flags & types.BLOCK_SETEXT_HEADER) != 0;
                self.heading_text_buf.clearRetainingCapacity();
            },
            .code => {
                self.in_code_block = true;
            },
            else => {},
        }
    }

    fn leaveBlock(self: *ExtractionRenderer, block_type: BlockType) void {
        switch (block_type) {
            .h => {
                self.finalizeHeading();
                self.in_heading = false;
            },
            .code => {
                self.in_code_block = false;
            },
            else => {},
        }
    }

    // ── Span callbacks ───────────────────────────────────────────────

    fn enterSpan(self: *ExtractionRenderer, span_type: SpanType, detail: SpanDetail) void {
        switch (span_type) {
            .a => {
                if (!self.in_image) {
                    self.in_link = true;
                    self.link_is_wiki = false;
                    self.link_is_autolink = detail.autolink or detail.permissive_autolink;
                    self.link_text_buf.clearRetainingCapacity();
                    self.link_href_buf.clearRetainingCapacity();
                    self.link_href_buf.appendSlice(self.allocator, detail.href) catch {};
                }
            },
            .wikilink => {
                self.in_link = true;
                self.link_is_wiki = true;
                self.link_is_autolink = false;
                self.link_text_buf.clearRetainingCapacity();
                self.link_href_buf.clearRetainingCapacity();
                self.link_href_buf.appendSlice(self.allocator, detail.href) catch {};
            },
            .img => {
                self.in_image = true;
            },
            else => {},
        }
    }

    fn leaveSpan(self: *ExtractionRenderer, span_type: SpanType) void {
        switch (span_type) {
            .a => {
                if (self.in_link and !self.in_image) {
                    self.finalizeLink();
                    self.in_link = false;
                }
            },
            .wikilink => {
                if (self.in_link) {
                    self.finalizeLink();
                    self.in_link = false;
                }
            },
            .img => {
                self.in_image = false;
            },
            else => {},
        }
    }

    // ── Text callback ────────────────────────────────────────────────

    fn text(self: *ExtractionRenderer, _: TextType, content: []const u8) void {
        if (self.in_code_block) return;

        if (self.in_heading) {
            self.heading_text_buf.appendSlice(self.allocator, content) catch {};
        }
        if (self.in_link and !self.in_image) {
            self.link_text_buf.appendSlice(self.allocator, content) catch {};
        }
    }

    // ── Finalization helpers ─────────────────────────────────────────

    fn finalizeHeading(self: *ExtractionRenderer) void {
        const owned_text = self.heading_text_buf.toOwnedSlice(self.allocator) catch return;

        const offset = self.findHeadingOffset();
        self.headings.append(self.allocator, .{
            .text = owned_text,
            .offset = offset,
            .level = self.heading_level,
        }) catch {
            self.allocator.free(owned_text);
        };
    }

    fn finalizeLink(self: *ExtractionRenderer) void {
        const owned_text = self.link_text_buf.toOwnedSlice(self.allocator) catch return;
        const owned_target = self.link_href_buf.toOwnedSlice(self.allocator) catch {
            self.allocator.free(owned_text);
            return;
        };

        const offset = self.findLinkOffset();
        self.links.append(self.allocator, .{
            .text = owned_text,
            .target = owned_target,
            .offset = offset,
            .is_wiki = self.link_is_wiki,
        }) catch {
            self.allocator.free(owned_text);
            self.allocator.free(owned_target);
        };
    }

    // ── Offset recovery via forward scan ─────────────────────────────

    fn findHeadingOffset(self: *ExtractionRenderer) u32 {
        const src = self.src_text;
        var pos: u32 = self.scan_cursor;

        if (self.heading_is_setext) {
            // Setext: find a line followed by === (level 1) or --- (level 2).
            // Return offset of the text line start.
            while (pos < src.len) {
                // Find the start of a text line
                const line_start = pos;
                // Skip to end of this line
                while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
                const line_end = pos;
                // Skip newline
                if (pos < src.len) pos += 1;

                // Check if NEXT line is the underline
                if (pos < src.len) {
                    const underline_char: u8 = if (self.heading_level == 1) '=' else '-';
                    var underline_start = pos;
                    // Skip optional leading spaces (up to 3)
                    var leading_spaces: u32 = 0;
                    while (underline_start < src.len and src[underline_start] == ' ' and leading_spaces < 3) {
                        underline_start += 1;
                        leading_spaces += 1;
                    }
                    if (underline_start < src.len and src[underline_start] == underline_char) {
                        var underline_end = underline_start;
                        while (underline_end < src.len and src[underline_end] == underline_char) : (underline_end += 1) {}
                        // Must have at least 1 underline char and rest of line is blank
                        if (underline_end > underline_start) {
                            var trailing = underline_end;
                            while (trailing < src.len and (src[trailing] == ' ' or src[trailing] == '\t')) : (trailing += 1) {}
                            if (trailing >= src.len or src[trailing] == '\n' or src[trailing] == '\r') {
                                // Only if the text line is non-empty
                                if (line_end > line_start) {
                                    // Advance cursor past the underline
                                    self.scan_cursor = @intCast(@min(trailing + 1, src.len));
                                    return @intCast(line_start);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // ATX: search for N '#' characters (possibly after whitespace/blockquote markers)
            while (pos < src.len) {
                // Find '#' character
                if (src[pos] == '#') {
                    // Count consecutive '#' characters
                    const hash_start = pos;
                    var hash_count: u8 = 0;
                    var p = pos;
                    while (p < src.len and src[p] == '#') : (p += 1) {
                        hash_count += 1;
                    }
                    if (hash_count == self.heading_level and (p >= src.len or src[p] == ' ' or src[p] == '\n' or src[p] == '\r' or src[p] == '\t')) {
                        // Skip past the heading line
                        while (p < src.len and src[p] != '\n') : (p += 1) {}
                        if (p < src.len) p += 1;
                        self.scan_cursor = @intCast(p);
                        return @intCast(hash_start);
                    }
                }
                pos += 1;
            }
        }

        // Fallback: use current cursor
        return self.scan_cursor;
    }

    fn findLinkOffset(self: *ExtractionRenderer) u32 {
        const src = self.src_text;
        var pos: u32 = self.scan_cursor;

        if (self.link_is_wiki) {
            // Search for '[['
            while (pos + 1 < src.len) {
                if (src[pos] == '[' and src[pos + 1] == '[') {
                    // Advance cursor past the wiki link ]]
                    var end = pos + 2;
                    while (end + 1 < src.len) {
                        if (src[end] == ']' and src[end + 1] == ']') {
                            end += 2;
                            break;
                        }
                        end += 1;
                    }
                    self.scan_cursor = @intCast(end);
                    return @intCast(pos);
                }
                pos += 1;
            }
        } else if (self.link_is_autolink) {
            // Search for '<'
            while (pos < src.len) {
                if (src[pos] == '<') {
                    var end = pos + 1;
                    while (end < src.len and src[end] != '>') : (end += 1) {}
                    if (end < src.len) end += 1; // past '>'
                    self.scan_cursor = @intCast(end);
                    return @intCast(pos);
                }
                pos += 1;
            }
        } else {
            // Search for '[' (inline or reference link)
            while (pos < src.len) {
                if (src[pos] == '[') {
                    // Skip image links — they start with ![ and are tracked by in_image
                    if (pos > 0 and src[pos - 1] == '!') {
                        pos += 1;
                        continue;
                    }
                    // Advance cursor past the closing ) or ]
                    var end = pos + 1;
                    var bracket_depth: u32 = 1;
                    while (end < src.len and bracket_depth > 0) {
                        if (src[end] == '[') bracket_depth += 1;
                        if (src[end] == ']') bracket_depth -= 1;
                        end += 1;
                    }
                    // Skip past (url) if present
                    if (end < src.len and src[end] == '(') {
                        end += 1;
                        while (end < src.len and src[end] != ')') : (end += 1) {}
                        if (end < src.len) end += 1;
                    } else if (end < src.len and src[end] == '[') {
                        // Reference link [text][ref]
                        end += 1;
                        while (end < src.len and src[end] != ']') : (end += 1) {}
                        if (end < src.len) end += 1;
                    }
                    self.scan_cursor = @intCast(end);
                    return @intCast(pos);
                }
                pos += 1;
            }
        }

        // Fallback
        return self.scan_cursor;
    }
};

// ── Public API ───────────────────────────────────────────────────────

pub fn extractFromMarkdown(
    text_input: []const u8,
    allocator: Allocator,
) parser_mod.Parser.Error!ExtractionResult {
    const input = helpers.skipUtf8Bom(text_input);
    var ext = ExtractionRenderer.init(allocator, input);
    errdefer ext.deinit();

    const opts = root.Options{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true,
    };
    try root.renderWithRenderer(input, allocator, opts, ext.renderer());

    // Transfer ownership: take slices from ArrayLists, free temporary buffers
    const headings = ext.headings.toOwnedSlice(allocator) catch {
        ext.deinit();
        return error.OutOfMemory;
    };
    const links = ext.links.toOwnedSlice(allocator) catch {
        allocator.free(headings);
        ext.deinit();
        return error.OutOfMemory;
    };

    // Free accumulation buffers only (results transferred)
    ext.heading_text_buf.deinit(allocator);
    ext.link_text_buf.deinit(allocator);
    ext.link_href_buf.deinit(allocator);
    // Deinit the now-empty ArrayLists (items transferred to owned slices)
    ext.headings.deinit(allocator);
    ext.links.deinit(allocator);

    return .{
        .headings = headings,
        .links = links,
        .allocator = allocator,
    };
}

// ── Tests ────────────────────────────────────────────────────────────

const testing = std.testing;

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
    // Use page_allocator due to known normalizeLabel leak (marky-i3fl)
    const input = "[text][ref]\n\n[ref]: https://example.com\n";
    var result = try extractFromMarkdown(input, std.heap.page_allocator);
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

// --- Wiki link tests ---

test "extract wiki link" {
    // page_allocator due to normalizeLabel leak (marky-i3fl)
    const input = "[[Target]]\n";
    var result = try extractFromMarkdown(input, std.heap.page_allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(true, result.links[0].is_wiki);
    try testing.expectEqualStrings("Target", result.links[0].target);
}

test "extract wiki link with alias" {
    // page_allocator due to normalizeLabel leak (marky-i3fl)
    const input = "[[Target|Display]]\n";
    var result = try extractFromMarkdown(input, std.heap.page_allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expectEqual(true, result.links[0].is_wiki);
    try testing.expectEqualStrings("Target", result.links[0].target);
    try testing.expectEqualStrings("Display", result.links[0].text);
}

test "extract wiki link byte offset" {
    // page_allocator due to normalizeLabel leak (marky-i3fl)
    const input = "Text [[Target]]\n";
    var result = try extractFromMarkdown(input, std.heap.page_allocator);
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
    // page_allocator due to normalizeLabel leak (marky-i3fl)
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
    var result = try extractFromMarkdown(input, std.heap.page_allocator);
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

test "entity in heading" {
    const input = "# Hello &amp; World\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.headings.len);
    // md4c sends entity references as raw text via TextType.entity callback.
    // ExtractionRenderer does NOT decode entities — passes through as-is.
    try testing.expectEqualStrings("Hello &amp; World", result.headings[0].text);
    // Offset should point to '#'
    try testing.expect(input[result.headings[0].offset] == '#');
}
