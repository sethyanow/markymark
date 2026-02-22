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

pub const ExtractedCodeSpan = struct {
    text: []const u8, // owned decoded code span text
    offset: u32, // byte offset of opening backtick in source
    end_offset: u32, // byte offset past closing backtick in source
};

pub const ExtractedTask = struct {
    state: u8, // task mark char: ' ', 'x', 'X'
    text: []const u8, // owned by allocator
    offset: u32, // byte offset of '[' in [x]
    end_offset: u32, // byte offset past task text
};

pub const ExtractedEmbed = struct {
    target: []const u8, // owned by allocator
    offset: u32, // byte offset of '!' before '![['
    end_offset: u32, // byte offset past ']]'
};

pub const ExtractionResult = struct {
    headings: []ExtractedHeading,
    links: []ExtractedLink,
    code_spans: []ExtractedCodeSpan,
    tasks: []ExtractedTask,
    embeds: []ExtractedEmbed,
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
        for (self.code_spans) |cs| {
            self.allocator.free(cs.text);
        }
        self.allocator.free(self.code_spans);
        for (self.tasks) |t| {
            self.allocator.free(t.text);
        }
        self.allocator.free(self.tasks);
        for (self.embeds) |e| {
            self.allocator.free(e.target);
        }
        self.allocator.free(self.embeds);
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

    // Free accumulation buffers only (results transferred)
    ext.heading_text_buf.deinit(allocator);
    ext.link_text_buf.deinit(allocator);
    ext.link_href_buf.deinit(allocator);
    ext.code_text_buf.deinit(allocator);
    ext.task_text_buf.deinit(allocator);
    // Deinit the now-empty ArrayLists (items transferred to owned slices)
    ext.headings.deinit(allocator);
    ext.links.deinit(allocator);
    ext.code_spans.deinit(allocator);
    ext.tasks.deinit(allocator);
    ext.embeds.deinit(allocator);

    return .{
        .headings = headings,
        .links = links,
        .code_spans = code_spans,
        .tasks = tasks,
        .embeds = embeds,
        .allocator = allocator,
    };
}

// ── Tests ────────────────────────────────────────────────────────────

test {
    _ = @import("extraction_renderer_tests.zig");
}
