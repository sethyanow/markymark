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
const offsets = @import("./extraction_renderer_offsets.zig");
const scans = @import("./extraction_renderer_scans.zig");
const root = @import("./root.zig");
const parser_mod = @import("./parser.zig");

// ── Result types (re-exported from extraction_renderer_types.zig) ────
const result_types = @import("./extraction_renderer_types.zig");
pub const ExtractedHeading = result_types.ExtractedHeading;
pub const ExtractedLink = result_types.ExtractedLink;
pub const ExtractedCodeSpan = result_types.ExtractedCodeSpan;
pub const ExtractedTask = result_types.ExtractedTask;
pub const ExtractedEmbed = result_types.ExtractedEmbed;
pub const ExtractedCallout = result_types.ExtractedCallout;
pub const ExtractedBlockRef = result_types.ExtractedBlockRef;
pub const ExtractedQueryBlock = result_types.ExtractedQueryBlock;
pub const ExtractedLinkDefinition = result_types.ExtractedLinkDefinition;
pub const ExtractedProperty = result_types.ExtractedProperty;
pub const ExtractedXmlTag = result_types.ExtractedXmlTag;
pub const ExtractionResult = result_types.ExtractionResult;

// ── Internal types for XML tag extraction ────────────────────────────

const HtmlFragment = struct {
    content: []const u8, // slice into src_text
    offset: u32, // byte offset in source
};

const OpenXmlTag = struct {
    tag_name: []const u8, // slice into src_text
    raw_html: []const u8, // slice into src_text (opening tag)
    offset: u32, // start byte
};

// HTML5 void elements — self-closing even without />
const void_elements = [_][]const u8{
    "br", "hr", "img", "input", "meta", "link",
    "source", "track", "wbr", "area", "base",
    "col", "embed", "param",
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

    // code span accumulation state (SEPARATE cursor per marky-0rl6 lesson)
    code_spans: std.ArrayListUnmanaged(ExtractedCodeSpan) = .{},
    code_scan_cursor: u32 = 0,
    in_code_span: bool = false,
    code_text_buf: std.ArrayListUnmanaged(u8) = .{},

    // task accumulation state (SEPARATE cursor)
    tasks: std.ArrayListUnmanaged(ExtractedTask) = .{},
    in_task: bool = false,
    task_state: u8 = 0,
    task_text_buf: std.ArrayListUnmanaged(u8) = .{},
    task_scan_cursor: u32 = 0,

    // embed accumulation state
    embeds: std.ArrayListUnmanaged(ExtractedEmbed) = .{},

    // callout accumulation state (SEPARATE cursor per marky-0rl6 lesson)
    callouts: std.ArrayListUnmanaged(ExtractedCallout) = .{},
    quote_depth: u8 = 0,
    callout_type_buf: std.ArrayListUnmanaged(u8) = .{},
    callout_title_buf: std.ArrayListUnmanaged(u8) = .{},
    callout_scan_cursor: u32 = 0,

    // block ref accumulation state (SEPARATE cursor)
    block_refs: std.ArrayListUnmanaged(ExtractedBlockRef) = .{},
    block_ref_scan_cursor: u32 = 0,

    // query block + link definition + property results (raw source scan, no cursor needed)
    query_blocks: std.ArrayListUnmanaged(ExtractedQueryBlock) = .{},
    link_definitions: std.ArrayListUnmanaged(ExtractedLinkDefinition) = .{},
    properties: std.ArrayListUnmanaged(ExtractedProperty) = .{},

    // XML tag extraction (callback-based HTML fragment collection + finalization)
    xml_tags: std.ArrayListUnmanaged(ExtractedXmlTag) = .{},
    html_fragments: std.ArrayListUnmanaged(HtmlFragment) = .{},
    xml_tag_stack: std.ArrayListUnmanaged(OpenXmlTag) = .{},

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
        self.code_text_buf.deinit(self.allocator);
        self.task_text_buf.deinit(self.allocator);
        self.callout_type_buf.deinit(self.allocator);
        self.callout_title_buf.deinit(self.allocator);

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
        for (self.code_spans.items) |cs| {
            self.allocator.free(cs.text);
        }
        self.code_spans.deinit(self.allocator);
        for (self.tasks.items) |t| {
            self.allocator.free(t.text);
        }
        self.tasks.deinit(self.allocator);
        for (self.embeds.items) |e| {
            self.allocator.free(e.target);
        }
        self.embeds.deinit(self.allocator);
        for (self.callouts.items) |c| {
            self.allocator.free(c.callout_type);
            if (c.title) |t| self.allocator.free(t);
        }
        self.callouts.deinit(self.allocator);
        for (self.block_refs.items) |br| {
            self.allocator.free(br.uuid);
        }
        self.block_refs.deinit(self.allocator);
        for (self.query_blocks.items) |qb| {
            self.allocator.free(qb.query);
        }
        self.query_blocks.deinit(self.allocator);
        for (self.link_definitions.items) |ld| {
            self.allocator.free(ld.label);
            self.allocator.free(ld.url);
            if (ld.title) |t| self.allocator.free(t);
        }
        self.link_definitions.deinit(self.allocator);
        for (self.properties.items) |p| {
            self.allocator.free(p.key);
            self.allocator.free(p.value);
        }
        self.properties.deinit(self.allocator);
        for (self.xml_tags.items) |xt| {
            self.allocator.free(xt.tag_name);
            self.allocator.free(xt.raw_html);
        }
        self.xml_tags.deinit(self.allocator);
        self.html_fragments.deinit(self.allocator);
        self.xml_tag_stack.deinit(self.allocator);
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
            .li => {
                // Nested task handling: finalize current task if re-entering
                if (self.in_task) self.finalizeTask();
                const task_mark = types.taskMarkFromData(data);
                if (task_mark != 0) {
                    self.in_task = true;
                    self.task_state = task_mark;
                    self.task_text_buf.clearRetainingCapacity();
                } else {
                    self.in_task = false;
                }
            },
            .quote => {
                self.quote_depth += 1;
                if (self.quote_depth == 1) {
                    self.callout_type_buf.clearRetainingCapacity();
                    self.callout_title_buf.clearRetainingCapacity();
                    // Scan raw source for callout pattern > [!type] title
                    self.scanCalloutInSource();
                }
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
            .li => {
                if (self.in_task) {
                    self.finalizeTask();
                    self.in_task = false;
                }
            },
            .quote => {
                if (self.quote_depth == 1) {
                    self.finalizeCallout();
                }
                if (self.quote_depth > 0) self.quote_depth -= 1;
            },
            .doc => {
                // Process collected HTML fragments into XML tags
                self.processHtmlFragments();
                // Raw source scans for query blocks, link definitions, and properties
                self.scanQueryBlocks();
                self.scanLinkDefinitions();
                self.scanProperties();
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
            .code => {
                self.in_code_span = true;
                self.code_text_buf.clearRetainingCapacity();
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
            .code => {
                if (self.in_code_span) {
                    self.finalizeCodeSpan();
                    self.in_code_span = false;
                }
            },
            else => {},
        }
    }

    // ── Text callback ────────────────────────────────────────────────

    fn text(self: *ExtractionRenderer, text_type: TextType, content: []const u8) void {
        if (self.in_code_block) return;

        // Collect HTML fragments for XML tag extraction
        if (text_type == .html) {
            if (content.len > 0 and @intFromPtr(content.ptr) >= @intFromPtr(self.src_text.ptr)) {
                const byte_offset = @intFromPtr(content.ptr) - @intFromPtr(self.src_text.ptr);
                if (byte_offset <= std.math.maxInt(u32)) {
                    self.html_fragments.append(self.allocator, .{
                        .content = content,
                        .offset = @intCast(byte_offset),
                    }) catch {
                        self.oom = true;
                    };
                }
            }
        }

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
        if (self.in_code_span) {
            self.code_text_buf.appendSlice(self.allocator, effective) catch { self.oom = true; };
        }
        if (self.in_task) {
            self.task_text_buf.appendSlice(self.allocator, effective) catch { self.oom = true; };
        }

        // Block ref scanning: look for ((uuid)) in non-code content
        if (!self.in_code_span) {
            self.scanBlockRefs(effective);
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
            return;
        };

        // Detect embed: wikilink preceded by '!' in source
        if (self.link_is_wiki and offset > 0 and self.src_text[offset - 1] == '!' and owned_target.len > 0) {
            const embed_target = self.allocator.dupe(u8, owned_target) catch {
                self.oom = true;
                return;
            };
            self.embeds.append(self.allocator, .{
                .target = embed_target,
                .offset = offset - 1, // '!' position
                .end_offset = end_offset,
            }) catch {
                self.oom = true;
                self.allocator.free(embed_target);
            };
        }
    }

    fn finalizeCodeSpan(self: *ExtractionRenderer) void {
        const owned_text = self.code_text_buf.toOwnedSlice(self.allocator) catch {
            self.oom = true;
            return;
        };

        const offset = self.findCodeSpanOffset();
        const end_offset: u32 = self.code_scan_cursor;
        self.code_spans.append(self.allocator, .{
            .text = owned_text,
            .offset = offset,
            .end_offset = end_offset,
        }) catch {
            self.oom = true;
            self.allocator.free(owned_text);
        };
    }

    fn findCodeSpanOffset(self: *ExtractionRenderer) u32 {
        return offsets.findCodeSpanOffset(self.src_text, &self.code_scan_cursor);
    }

    fn finalizeTask(self: *ExtractionRenderer) void {
        const owned_text = self.task_text_buf.toOwnedSlice(self.allocator) catch {
            self.oom = true;
            return;
        };

        const offset = self.findTaskOffset();
        const end_offset: u32 = self.task_scan_cursor;
        self.tasks.append(self.allocator, .{
            .state = self.task_state,
            .text = owned_text,
            .offset = offset,
            .end_offset = end_offset,
        }) catch {
            self.oom = true;
            self.allocator.free(owned_text);
        };
    }

    fn findTaskOffset(self: *ExtractionRenderer) u32 {
        return offsets.findTaskOffset(self.src_text, &self.task_scan_cursor);
    }

    fn findHeadingOffset(self: *ExtractionRenderer) u32 {
        return offsets.findHeadingOffset(self.src_text, &self.heading_scan_cursor, self.heading_is_setext, self.heading_level);
    }

    fn findLinkOffset(self: *ExtractionRenderer) u32 {
        return offsets.findLinkOffset(self.src_text, &self.link_scan_cursor, self.link_is_wiki, self.link_is_autolink);
    }

    /// Scan raw source text from callout_scan_cursor for callout pattern: > [!type] title
    /// If found, populate callout_type_buf (lowercased) and callout_title_buf.
    fn scanCalloutInSource(self: *ExtractionRenderer) void {
        const src = self.src_text;
        var pos: u32 = self.callout_scan_cursor;

        // Find '>' at start of a line
        while (pos < src.len) {
            const at_line_start = (pos == 0 or src[pos - 1] == '\n');
            if (at_line_start) {
                var lp: u32 = pos;
                // Skip leading spaces (up to 3)
                var spaces: u32 = 0;
                while (lp < src.len and src[lp] == ' ' and spaces < 3) {
                    lp += 1;
                    spaces += 1;
                }
                if (lp < src.len and src[lp] == '>') {
                    lp += 1;
                    // Skip whitespace after '>'
                    while (lp < src.len and (src[lp] == ' ' or src[lp] == '\t')) : (lp += 1) {}
                    // Check for [!
                    if (lp + 2 < src.len and src[lp] == '[' and src[lp + 1] == '!') {
                        lp += 2;
                        const type_start = lp;
                        while (lp < src.len and std.ascii.isAlphabetic(src[lp])) : (lp += 1) {}
                        if (lp == type_start) return; // empty type [!]
                        if (lp >= src.len or src[lp] != ']') return; // no closing ]

                        // Store lowercased type
                        for (src[type_start..lp]) |ch| {
                            self.callout_type_buf.append(self.allocator, std.ascii.toLower(ch)) catch {
                                self.oom = true;
                                return;
                            };
                        }
                        lp += 1; // skip ']'

                        // Extract title: rest of line after ], trimmed
                        var line_end = lp;
                        while (line_end < src.len and src[line_end] != '\n') : (line_end += 1) {}
                        const raw_title = std.mem.trim(u8, src[lp..line_end], " \t");
                        if (raw_title.len > 0) {
                            self.callout_title_buf.appendSlice(self.allocator, raw_title) catch {
                                self.oom = true;
                            };
                        }
                        return;
                    }
                }
            }
            pos += 1;
        }
    }

    /// Finalize a callout if callout_type_buf is non-empty (i.e., [!type] was found).
    fn finalizeCallout(self: *ExtractionRenderer) void {
        if (self.callout_type_buf.items.len == 0) return;

        const owned_type = self.callout_type_buf.toOwnedSlice(self.allocator) catch {
            self.oom = true;
            return;
        };
        const owned_title: ?[]const u8 = if (self.callout_title_buf.items.len > 0)
            self.callout_title_buf.toOwnedSlice(self.allocator) catch {
                self.oom = true;
                self.allocator.free(owned_type);
                return;
            }
        else
            null;

        const co_offset = self.findCalloutOffset();
        // end_offset: use callout_scan_cursor (advanced past the callout in source)
        const end_offset: u32 = self.callout_scan_cursor;
        self.callouts.append(self.allocator, .{
            .callout_type = owned_type,
            .title = owned_title,
            .offset = co_offset,
            .end_offset = end_offset,
        }) catch {
            self.oom = true;
            self.allocator.free(owned_type);
            if (owned_title) |t| self.allocator.free(t);
        };
    }

    /// Scan forward from callout_scan_cursor for '>' then '[!' to find callout offset.
    fn findCalloutOffset(self: *ExtractionRenderer) u32 {
        return offsets.findCalloutOffset(self.src_text, &self.callout_scan_cursor);
    }

    // ── XML tag extraction ─────────────────────────────────────────

    /// Parse tag name from HTML fragment. Returns null for comments, PI, CDATA, DOCTYPE.
    fn parseTagName(html: []const u8) ?struct { name: []const u8, is_closing: bool } {
        if (html.len < 2 or html[0] != '<') return null;
        var i: usize = 1;
        // Skip comments <!-- -->, CDATA <![, processing instructions <?, DOCTYPE <!D
        if (i < html.len and (html[i] == '!' or html[i] == '?')) return null;
        const is_closing = i < html.len and html[i] == '/';
        if (is_closing) i += 1;
        // Tag name start: must be alphabetic (HTML5 rules)
        if (i >= html.len or !std.ascii.isAlphabetic(html[i])) return null;
        const name_start = i;
        while (i < html.len) : (i += 1) {
            const c = html[i];
            if (std.ascii.isAlphanumeric(c) or c == '_' or c == ':' or c == '-' or c == '.') continue;
            break;
        }
        if (i == name_start) return null;
        return .{ .name = html[name_start..i], .is_closing = is_closing };
    }

    fn isVoidElement(name: []const u8) bool {
        for (&void_elements) |v| {
            if (std.ascii.eqlIgnoreCase(name, v)) return true;
        }
        return false;
    }

    /// Process collected HTML fragments into structured XML tag entries.
    /// Called from leaveBlock(.doc) after all callbacks are done.
    fn processHtmlFragments(self: *ExtractionRenderer) void {
        if (self.oom) return;

        for (self.html_fragments.items) |frag| {
            if (frag.content.len == 0 or frag.content[0] != '<') continue;

            const parsed = parseTagName(frag.content) orelse continue;

            if (parsed.is_closing) {
                // Pop matching open tag from stack (innermost first, same-name)
                var match_idx: ?usize = null;
                var j = self.xml_tag_stack.items.len;
                while (j > 0) {
                    j -= 1;
                    if (std.ascii.eqlIgnoreCase(self.xml_tag_stack.items[j].tag_name, parsed.name)) {
                        match_idx = j;
                        break;
                    }
                }
                if (match_idx) |idx| {
                    const open = self.xml_tag_stack.orderedRemove(idx);
                    const end_offset = frag.offset +| @as(u32, @intCast(frag.content.len));

                    const owned_name = self.allocator.dupe(u8, open.tag_name) catch {
                        self.oom = true;
                        return;
                    };
                    errdefer self.allocator.free(owned_name);
                    const owned_html = self.allocator.dupe(u8, open.raw_html) catch {
                        self.oom = true;
                        return;
                    };

                    self.xml_tags.append(self.allocator, .{
                        .tag_name = owned_name,
                        .raw_html = owned_html,
                        .offset = open.offset,
                        .end_offset = end_offset,
                        .is_self_closing = false,
                        .is_unclosed = false,
                    }) catch {
                        self.allocator.free(owned_name);
                        self.allocator.free(owned_html);
                        self.oom = true;
                        return;
                    };
                }
                // Unmatched close tags silently ignored (same as Rust)
            } else {
                // Check self-closing: ends with /> or is void element
                const is_self_closing = (frag.content.len >= 2 and
                    frag.content[frag.content.len - 2] == '/' and
                    frag.content[frag.content.len - 1] == '>') or
                    isVoidElement(parsed.name);

                if (is_self_closing) {
                    const end_offset = frag.offset +| @as(u32, @intCast(frag.content.len));

                    const owned_name = self.allocator.dupe(u8, parsed.name) catch {
                        self.oom = true;
                        return;
                    };
                    errdefer self.allocator.free(owned_name);
                    const owned_html = self.allocator.dupe(u8, frag.content) catch {
                        self.oom = true;
                        return;
                    };

                    self.xml_tags.append(self.allocator, .{
                        .tag_name = owned_name,
                        .raw_html = owned_html,
                        .offset = frag.offset,
                        .end_offset = end_offset,
                        .is_self_closing = true,
                        .is_unclosed = false,
                    }) catch {
                        self.allocator.free(owned_name);
                        self.allocator.free(owned_html);
                        self.oom = true;
                        return;
                    };
                } else {
                    // Push to stack for matching
                    self.xml_tag_stack.append(self.allocator, .{
                        .tag_name = parsed.name,
                        .raw_html = frag.content,
                        .offset = frag.offset,
                    }) catch {
                        self.oom = true;
                        return;
                    };
                }
            }
        }

        // Finalize: remaining stack entries are unclosed tags
        for (self.xml_tag_stack.items) |open| {
            const end_offset = open.offset +| @as(u32, @intCast(open.raw_html.len));

            const owned_name = self.allocator.dupe(u8, open.tag_name) catch {
                self.oom = true;
                return;
            };
            errdefer self.allocator.free(owned_name);
            const owned_html = self.allocator.dupe(u8, open.raw_html) catch {
                self.oom = true;
                return;
            };

            self.xml_tags.append(self.allocator, .{
                .tag_name = owned_name,
                .raw_html = owned_html,
                .offset = open.offset,
                .end_offset = end_offset,
                .is_self_closing = false,
                .is_unclosed = true,
            }) catch {
                self.allocator.free(owned_name);
                self.allocator.free(owned_html);
                self.oom = true;
                return;
            };
        }
    }

    /// Scan text content for block refs matching ((uuid)) pattern with 8-4-4-4-12 validation.
    fn scanBlockRefs(self: *ExtractionRenderer, content: []const u8) void {
        var i: usize = 0;
        while (i + 1 < content.len) {
            if (content[i] == '(' and content[i + 1] == '(') {
                // Need at least 36 chars of UUID + '))'
                if (i + 2 + 36 + 2 <= content.len) {
                    const uuid_slice = content[i + 2 .. i + 2 + 36];
                    if (content[i + 2 + 36] == ')' and content[i + 2 + 36 + 1] == ')') {
                        if (isValidUuid(uuid_slice)) {
                            const owned_uuid = self.allocator.dupe(u8, uuid_slice) catch {
                                self.oom = true;
                                return;
                            };
                            const br_offset = offsets.findBlockRefOffset(self.src_text, &self.block_ref_scan_cursor);
                            self.block_refs.append(self.allocator, .{
                                .uuid = owned_uuid,
                                .offset = br_offset,
                            }) catch {
                                self.oom = true;
                                self.allocator.free(owned_uuid);
                                return;
                            };
                            i += 2 + 36 + 2;
                            continue;
                        }
                    }
                }
                i += 2;
            } else {
                i += 1;
            }
        }
    }

    /// Validate UUID format: 8-4-4-4-12 hex digits with dashes at positions 8,13,18,23.
    fn isValidUuid(s: []const u8) bool {
        if (s.len != 36) return false;
        for (s, 0..) |ch, idx| {
            if (idx == 8 or idx == 13 or idx == 18 or idx == 23) {
                if (ch != '-') return false;
            } else {
                if (!std.ascii.isHex(ch)) return false;
            }
        }
        return true;
    }

    /// Scan raw source for `{{query ...}}` patterns, skipping fenced code blocks.
    fn scanQueryBlocks(self: *ExtractionRenderer) void {
        if (scans.scanQueryBlocksInSource(self.src_text, self.allocator, &self.query_blocks))
            self.oom = true;
    }

    /// Scan raw source for `[label]: url "title"` link definitions, skipping fenced code blocks.
    fn scanLinkDefinitions(self: *ExtractionRenderer) void {
        if (scans.scanLinkDefinitionsInSource(self.src_text, self.allocator, &self.link_definitions))
            self.oom = true;
    }

    /// Scan raw source for `key:: value` properties at document start.
    fn scanProperties(self: *ExtractionRenderer) void {
        if (scans.scanPropertiesInSource(self.src_text, self.allocator, &self.properties))
            self.oom = true;
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
    errdefer {
        for (links) |l| {
            allocator.free(l.text);
            allocator.free(l.target);
        }
        allocator.free(links);
    }
    const code_spans = ext.code_spans.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    errdefer {
        for (code_spans) |cs| allocator.free(cs.text);
        allocator.free(code_spans);
    }
    const tasks = ext.tasks.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    errdefer {
        for (tasks) |t| allocator.free(t.text);
        allocator.free(tasks);
    }
    const embeds = ext.embeds.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    errdefer {
        for (embeds) |e| allocator.free(e.target);
        allocator.free(embeds);
    }
    const callouts = ext.callouts.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    errdefer {
        for (callouts) |c| {
            allocator.free(c.callout_type);
            if (c.title) |t| allocator.free(t);
        }
        allocator.free(callouts);
    }
    const block_refs = ext.block_refs.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    errdefer {
        for (block_refs) |br| allocator.free(br.uuid);
        allocator.free(block_refs);
    }
    const query_blocks = ext.query_blocks.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    errdefer {
        for (query_blocks) |qb| allocator.free(qb.query);
        allocator.free(query_blocks);
    }
    const link_definitions = ext.link_definitions.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    errdefer {
        for (link_definitions) |ld| {
            allocator.free(ld.label);
            allocator.free(ld.url);
            if (ld.title) |t| allocator.free(t);
        }
        allocator.free(link_definitions);
    }
    const properties = ext.properties.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };
    errdefer {
        for (properties) |p| {
            allocator.free(p.key);
            allocator.free(p.value);
        }
        allocator.free(properties);
    }
    const xml_tags = ext.xml_tags.toOwnedSlice(allocator) catch {
        return error.OutOfMemory;
    };

    // Free accumulation buffers only (results transferred)
    ext.heading_text_buf.deinit(allocator);
    ext.link_text_buf.deinit(allocator);
    ext.link_href_buf.deinit(allocator);
    ext.code_text_buf.deinit(allocator);
    ext.task_text_buf.deinit(allocator);
    ext.callout_type_buf.deinit(allocator);
    ext.callout_title_buf.deinit(allocator);
    // Deinit the now-empty ArrayLists (items transferred to owned slices)
    ext.headings.deinit(allocator);
    ext.links.deinit(allocator);
    ext.code_spans.deinit(allocator);
    ext.tasks.deinit(allocator);
    ext.embeds.deinit(allocator);
    ext.callouts.deinit(allocator);
    ext.block_refs.deinit(allocator);
    ext.query_blocks.deinit(allocator);
    ext.link_definitions.deinit(allocator);
    ext.properties.deinit(allocator);
    ext.xml_tags.deinit(allocator);
    ext.html_fragments.deinit(allocator);
    ext.xml_tag_stack.deinit(allocator);

    return .{
        .headings = headings,
        .links = links,
        .code_spans = code_spans,
        .tasks = tasks,
        .embeds = embeds,
        .callouts = callouts,
        .block_refs = block_refs,
        .query_blocks = query_blocks,
        .link_definitions = link_definitions,
        .properties = properties,
        .xml_tags = xml_tags,
        .allocator = allocator,
    };
}

// ── Tests ────────────────────────────────────────────────────────────

test {
    _ = @import("extraction_renderer_tests_headings.zig");
    _ = @import("extraction_renderer_tests_regressions.zig");
    _ = @import("extraction_renderer_tests_code_spans.zig");
    _ = @import("extraction_renderer_tests_elements.zig");
    _ = @import("extraction_renderer_tests_xml_tags.zig");
}
