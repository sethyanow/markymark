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

    /// Scan forward from code_scan_cursor to find the opening backtick(s) of a code span.
    /// Advances code_scan_cursor past the closing backtick(s).
    fn findCodeSpanOffset(self: *ExtractionRenderer) u32 {
        const src = self.src_text;
        var pos: u32 = self.code_scan_cursor;

        // Find the opening backtick run
        while (pos < src.len) {
            if (src[pos] == '`') {
                const open_start = pos;
                // Count opening backtick run length
                var open_len: u32 = 0;
                while (pos < src.len and src[pos] == '`') : (pos += 1) {
                    open_len += 1;
                }
                // Scan for closing backtick run of exactly the same length
                while (pos < src.len) {
                    if (src[pos] == '`') {
                        var close_len: u32 = 0;
                        while (pos < src.len and src[pos] == '`') : (pos += 1) {
                            close_len += 1;
                        }
                        if (close_len == open_len) {
                            // Found matching closing backticks
                            self.code_scan_cursor = pos;
                            return open_start;
                        }
                        // Not matching — continue scanning (pos already advanced past these backticks)
                    } else {
                        pos += 1;
                    }
                }
                // No matching close found — return opening position, advance cursor
                self.code_scan_cursor = pos;
                return open_start;
            }
            pos += 1;
        }

        // Fallback: no backtick found
        return self.code_scan_cursor;
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

    /// Scan forward from task_scan_cursor to find the '[' of a task checkbox [x].
    /// Advances task_scan_cursor past the checkbox and task text.
    fn findTaskOffset(self: *ExtractionRenderer) u32 {
        const src = self.src_text;
        var pos: u32 = self.task_scan_cursor;
        while (pos + 2 < src.len) {
            if (src[pos] == '[' and src[pos + 2] == ']') {
                // Advance cursor past the checkbox marker "[ ] " or "[x] "
                self.task_scan_cursor = @intCast(@min(@as(u64, pos) + 4, src.len));
                return pos;
            }
            pos += 1;
        }
        return self.task_scan_cursor;
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
