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
    end_offset: u32, // byte offset past the link's closing character
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

    heading_scan_cursor: u32 = 0,
    link_scan_cursor: u32 = 0,

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

    // set to true if any allocation fails inside a callback; checked after rendering
    oom: bool = false,

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
                    self.link_href_buf.appendSlice(self.allocator, detail.href) catch { self.oom = true; };
                }
            },
            .wikilink => {
                self.in_link = true;
                self.link_is_wiki = true;
                self.link_is_autolink = false;
                self.link_text_buf.clearRetainingCapacity();
                self.link_href_buf.clearRetainingCapacity();
                self.link_href_buf.appendSlice(self.allocator, detail.href) catch { self.oom = true; };
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

    fn text(self: *ExtractionRenderer, text_type: TextType, content: []const u8) void {
        if (self.in_code_block) return;

        // Decode HTML entity references to UTF-8; fall back to raw text if unknown
        var decode_buf: [8]u8 = undefined;
        const effective = if (text_type == .entity)
            helpers.decodeEntityToUtf8(content, &decode_buf) orelse content
        else
            content;

        if (self.in_heading) {
            self.heading_text_buf.appendSlice(self.allocator, effective) catch { self.oom = true; };
        }
        if (self.in_link and !self.in_image) {
            self.link_text_buf.appendSlice(self.allocator, effective) catch { self.oom = true; };
        }
    }

    // ── Finalization helpers ─────────────────────────────────────────

    fn finalizeHeading(self: *ExtractionRenderer) void {
        const owned_text = self.heading_text_buf.toOwnedSlice(self.allocator) catch {
            self.oom = true;
            return;
        };

        const offset = self.findHeadingOffset();
        self.headings.append(self.allocator, .{
            .text = owned_text,
            .offset = offset,
            .level = self.heading_level,
        }) catch {
            self.oom = true;
            self.allocator.free(owned_text);
        };
    }

    fn finalizeLink(self: *ExtractionRenderer) void {
        const owned_text = self.link_text_buf.toOwnedSlice(self.allocator) catch {
            self.oom = true;
            return;
        };
        const owned_target = self.link_href_buf.toOwnedSlice(self.allocator) catch {
            self.oom = true;
            self.allocator.free(owned_text);
            return;
        };

        const offset = self.findLinkOffset();
        const end_offset: u32 = self.link_scan_cursor;
        self.links.append(self.allocator, .{
            .text = owned_text,
            .target = owned_target,
            .offset = offset,
            .end_offset = end_offset,
            .is_wiki = self.link_is_wiki,
        }) catch {
            self.oom = true;
            self.allocator.free(owned_text);
            self.allocator.free(owned_target);
        };
    }

    // ── Offset recovery via forward scan ─────────────────────────────

    fn findHeadingOffset(self: *ExtractionRenderer) u32 {
        const src = self.src_text;
        var pos: u32 = self.heading_scan_cursor;

        if (self.heading_is_setext) {
            // Setext: find a line followed by === (level 1) or --- (level 2).
            // Return offset of the text line start.
            // Track fenced code blocks to avoid matching underlines inside them.
            var in_fence_s = false;
            var fence_char_s: u8 = 0;
            var fence_len_s: u32 = 0;
            while (pos < src.len) {
                // Find the start of a text line
                const line_start = pos;
                // Skip to end of this line
                while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
                const line_end = pos;
                // Skip newline
                if (pos < src.len) pos += 1;

                // Detect fence open/close at this line
                {
                    var fp = line_start;
                    var sp: u32 = 0;
                    while (fp < line_end and src[fp] == ' ' and sp < 3) { fp += 1; sp += 1; }
                    if (fp < line_end and (src[fp] == '`' or src[fp] == '~')) {
                        const fc = src[fp];
                        var flen: u32 = 0;
                        while (fp + flen < line_end and src[fp + flen] == fc) : (flen += 1) {}
                        if (flen >= 3) {
                            if (!in_fence_s) {
                                in_fence_s = true;
                                fence_char_s = fc;
                                fence_len_s = flen;
                            } else if (fc == fence_char_s and flen >= fence_len_s) {
                                in_fence_s = false;
                                fence_char_s = 0;
                                fence_len_s = 0;
                            }
                            continue; // skip fence lines
                        }
                    }
                }

                // If inside a fence, skip this line entirely
                if (in_fence_s) continue;

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
                                    self.heading_scan_cursor = @intCast(@min(trailing + 1, src.len));
                                    return @intCast(line_start);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // ATX: '#' must appear at line start (0-3 leading spaces + optional '>' blockquote).
            // Scan line-by-line; track code fences to skip false '#' matches inside fenced blocks.
            var in_fence = false;
            var fence_char: u8 = 0;
            var fence_len: u32 = 0;
            while (pos < src.len) {
                const line_start = pos;
                var line_end = pos;
                while (line_end < src.len and src[line_end] != '\n') : (line_end += 1) {}
                const next_line: u32 = @intCast(if (line_end < src.len) line_end + 1 else src.len);

                // Detect fence open/close: 0-3 spaces then 3+ identical backticks or tildes.
                var fp = pos;
                var sp: u32 = 0;
                while (fp < line_end and src[fp] == ' ' and sp < 3) { fp += 1; sp += 1; }
                if (fp < line_end and (src[fp] == '`' or src[fp] == '~')) {
                    const fc = src[fp];
                    var flen: u32 = 0;
                    while (fp + flen < line_end and src[fp + flen] == fc) : (flen += 1) {}
                    if (flen >= 3) {
                        if (!in_fence) {
                            in_fence = true;
                            fence_char = fc;
                            fence_len = flen;
                        } else if (fc == fence_char and flen >= fence_len) {
                            in_fence = false;
                            fence_char = 0;
                            fence_len = 0;
                        }
                        // else: different char or shorter fence — stay in fence
                        pos = next_line;
                        continue;
                    }
                }

                if (!in_fence) {
                    // Check for 0-3 leading spaces, optional '>' blockquote prefix, then '#'.
                    var lp = line_start;
                    var lsp: u32 = 0;
                    while (lp < line_end and src[lp] == ' ' and lsp < 3) { lp += 1; lsp += 1; }
                    while (lp < line_end and src[lp] == '>') {
                        lp += 1;
                        if (lp < line_end and src[lp] == ' ') lp += 1;
                    }
                    if (lp < line_end and src[lp] == '#') {
                        const hash_start = lp;
                        var hash_count: u8 = 0;
                        var p = lp;
                        while (p < line_end and src[p] == '#') : (p += 1) { hash_count += 1; }
                        if (hash_count == self.heading_level and
                            (p >= line_end or src[p] == ' ' or src[p] == '\t'))
                        {
                            self.heading_scan_cursor = next_line;
                            return @intCast(hash_start);
                        }
                    }
                }

                pos = next_line;
            }
        }

        // Fallback: use current cursor
        return self.heading_scan_cursor;
    }

    fn findLinkOffset(self: *ExtractionRenderer) u32 {
        const src = self.src_text;
        var pos: u32 = self.link_scan_cursor;

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
                    self.link_scan_cursor = @intCast(end);
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
                    self.link_scan_cursor = @intCast(end);
                    return @intCast(pos);
                }
                pos += 1;
            }
        } else {
            // Search for '[' (inline or reference link), skipping fenced code blocks.
            var in_fence = false;
            var fence_char: u8 = 0;
            var fence_len: u32 = 0;
            while (pos < src.len) {
                // Detect fence at line start: 0-3 spaces then 3+ backticks or tildes.
                if (pos == 0 or src[pos - 1] == '\n') {
                    var fp = pos;
                    var sp: u32 = 0;
                    while (fp < src.len and src[fp] == ' ' and sp < 3) { fp += 1; sp += 1; }
                    if (fp < src.len and (src[fp] == '`' or src[fp] == '~')) {
                        const fc = src[fp];
                        var flen: u32 = 0;
                        while (fp + flen < src.len and src[fp + flen] == fc) : (flen += 1) {}
                        if (flen >= 3) {
                            if (!in_fence) {
                                in_fence = true;
                                fence_char = fc;
                                fence_len = flen;
                            } else if (fc == fence_char and flen >= fence_len) {
                                in_fence = false;
                                fence_char = 0;
                                fence_len = 0;
                            }
                            // else: different char or shorter fence — stay in fence
                            while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
                            if (pos < src.len) pos += 1;
                            continue;
                        }
                    }
                }
                if (!in_fence and src[pos] == '[') {
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
                    // Skip past (url) if present, tracking paren depth for URLs like
                    // https://en.wikipedia.org/wiki/Foo_(bar) and handling backslash escapes.
                    if (end < src.len and src[end] == '(') {
                        end += 1;
                        var paren_depth: u32 = 1;
                        while (end < src.len and paren_depth > 0) {
                            if (src[end] == '\\' and end + 1 < src.len) {
                                end += 2; // skip escaped character
                                continue;
                            }
                            if (src[end] == '(') paren_depth += 1;
                            if (src[end] == ')') paren_depth -= 1;
                            if (paren_depth > 0) end += 1;
                        }
                        if (paren_depth == 0) end += 1; // skip final ')'
                    } else if (end < src.len and src[end] == '[') {
                        // Reference link [text][ref]
                        end += 1;
                        while (end < src.len and src[end] != ']') : (end += 1) {}
                        if (end < src.len) end += 1;
                    }
                    self.link_scan_cursor = @intCast(end);
                    return @intCast(pos);
                }
                pos += 1;
            }
        }

        // Fallback
        return self.link_scan_cursor;
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

    // If any callback allocation failed during rendering, propagate OOM now.
    // Callbacks cannot return errors through the C vtable, so failures are tracked
    // via the oom flag and converted to an error here.
    if (ext.oom) {
        return error.OutOfMemory;
    }

    // Transfer ownership: take slices from ArrayLists, free temporary buffers.
    // On failure, errdefer ext.deinit() (line 496) handles cleanup since
    // ext.headings still owns its items.
    const headings = ext.headings.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    // After toOwnedSlice succeeds, ext.headings is empty — the heading structs
    // (including their .text allocations) are now in the local `headings` slice.
    // This errdefer ensures they're freed if links.toOwnedSlice fails below.
    errdefer {
        for (headings) |h| allocator.free(h.text);
        allocator.free(headings);
    }
    const links = ext.links.toOwnedSlice(allocator) catch {
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
    // Uses a document with both headings and links so that the partially-
    // transferred-headings path (links.toOwnedSlice fails after headings
    // transferred) is exercised at some fail_index values.
    const input = "# Heading One\n\n[Link Text](https://example.com)\n\n## Heading Two\n";

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
