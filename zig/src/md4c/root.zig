// Vendored from https://github.com/oven-sh/bun (MIT License)
// Original: src/md/root.zig at commit 6a8f33e7b1 (bun-v1.3.9)
// Modifications: Stripped Bun-specific dependencies for standalone compilation.

// Re-export types needed by external renderers (e.g. JS callback renderer).
pub const Renderer = types.Renderer;
pub const BlockType = types.BlockType;
pub const SpanType = types.SpanType;
pub const TextType = types.TextType;
pub const SpanDetail = types.SpanDetail;
pub const Align = types.Align;
pub const BLOCK_FENCED_CODE = types.BLOCK_FENCED_CODE;

pub const RenderOptions = struct {
    tag_filter: bool = false,
    heading_ids: bool = false,
    autolink_headings: bool = false,
};

pub const Options = struct {
    tables: bool = true,
    strikethrough: bool = true,
    tasklists: bool = true,
    permissive_autolinks: bool = false,
    permissive_url_autolinks: bool = false,
    permissive_www_autolinks: bool = false,
    permissive_email_autolinks: bool = false,
    hard_soft_breaks: bool = false,
    wiki_links: bool = false,
    underline: bool = false,
    latex_math: bool = false,
    collapse_whitespace: bool = false,
    permissive_atx_headers: bool = false,
    no_indented_code_blocks: bool = false,
    no_html_blocks: bool = false,
    no_html_spans: bool = false,
    /// GFM tag filter: replaces `<` with `&lt;` for disallowed HTML tags
    /// (title, textarea, style, xmp, iframe, noembed, noframes, script, plaintext).
    tag_filter: bool = false,
    heading_ids: bool = false,
    autolink_headings: bool = false,

    pub const commonmark: Options = .{
        .tables = false,
        .strikethrough = false,
        .tasklists = false,
    };

    pub const github: Options = .{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .permissive_autolinks = true,
        .permissive_www_autolinks = true,
        .permissive_email_autolinks = true,
        .tag_filter = true,
    };

    pub fn toFlags(self: Options) Flags {
        return .{
            .tables = self.tables,
            .strikethrough = self.strikethrough,
            .tasklists = self.tasklists,
            .permissive_url_autolinks = self.permissive_url_autolinks or self.permissive_autolinks,
            .permissive_www_autolinks = self.permissive_www_autolinks or self.permissive_autolinks,
            .permissive_email_autolinks = self.permissive_email_autolinks or self.permissive_autolinks,
            .hard_soft_breaks = self.hard_soft_breaks,
            .wiki_links = self.wiki_links,
            .underline = self.underline,
            .latex_math = self.latex_math,
            .collapse_whitespace = self.collapse_whitespace,
            .permissive_atx_headers = self.permissive_atx_headers,
            .no_indented_code_blocks = self.no_indented_code_blocks,
            .no_html_blocks = self.no_html_blocks,
            .no_html_spans = self.no_html_spans,
        };
    }

    pub fn toRenderOptions(self: Options) RenderOptions {
        return .{
            .tag_filter = self.tag_filter,
            .heading_ids = self.heading_ids,
            .autolink_headings = self.autolink_headings,
        };
    }
};

pub fn renderToHtml(text: []const u8, allocator: std.mem.Allocator) parser.Parser.Error![]u8 {
    return renderToHtmlWithOptions(text, allocator, .{});
}

pub fn renderToHtmlWithOptions(text: []const u8, allocator: std.mem.Allocator, options: Options) parser.Parser.Error![]u8 {
    return parser.renderToHtml(text, allocator, options.toFlags(), options.toRenderOptions());
}

/// Parse and render using a custom renderer implementation.
pub fn renderWithRenderer(text: []const u8, allocator: std.mem.Allocator, options: Options, renderer: Renderer) parser.Parser.Error!void {
    return parser.renderWithRenderer(text, allocator, options.toFlags(), options.toRenderOptions(), renderer);
}

pub const types = @import("./types.zig");
const Flags = types.Flags;

pub const entity = @import("./entity.zig");
pub const extraction_renderer = @import("./extraction_renderer.zig");
pub const helpers = @import("./helpers.zig");

const parser = @import("./parser.zig");
const std = @import("std");

// ── Smoke tests ──────────────────────────────────────────────────────

test "md4c smoke: heading and paragraph" {
    const allocator = std.testing.allocator;
    const input = "# Hello\n\nworld\n";
    const html = try renderToHtml(input, allocator);
    defer allocator.free(html);
    try std.testing.expectEqualStrings("<h1>Hello</h1>\n<p>world</p>\n", html);
}

test "md4c smoke: empty input" {
    const allocator = std.testing.allocator;
    const html = try renderToHtml("", allocator);
    defer allocator.free(html);
    try std.testing.expectEqualStrings("", html);
}

test "md4c smoke: code fence" {
    const allocator = std.testing.allocator;
    const input = "```rust\nfn main() {}\n```\n";
    const html = try renderToHtml(input, allocator);
    defer allocator.free(html);
    try std.testing.expect(std.mem.indexOf(u8, html, "<code") != null);
    try std.testing.expect(std.mem.indexOf(u8, html, "fn main()") != null);
}

test "md4c smoke: wiki link passthrough" {
    const allocator = std.testing.allocator;
    // Without wiki_links extension, [[link]] should be treated as nested brackets
    const input = "[[link]]\n";
    const html = try renderToHtmlWithOptions(input, allocator, Options.commonmark);
    defer allocator.free(html);
    // Should not crash; content is rendered as text within a paragraph
    try std.testing.expect(html.len > 0);
}

// Regression test for T3-3: Parser.init casts text.len via @intCast(OFF) — silent
// truncation for >4GB input in ReleaseFast builds. Both public entry points must
// validate len before the cast and return InputTooLarge.
//
// The fake slice uses a valid stack address; the function must return before
// dereferencing any element (size check precedes skipUtf8Bom).
test "renderToHtml: rejects input > 4GB with InputTooLarge" {
    const huge_len: usize = @as(usize, std.math.maxInt(u32)) + 1;
    var sentinel: u8 = 0;
    // [*]const u8 has no tracked length — slicing to huge_len creates a valid
    // fat pointer. We must not access elements beyond &sentinel[0].
    const p: [*]const u8 = @ptrCast(&sentinel);
    const fake: []const u8 = p[0..huge_len];
    // Before fix: @intCast panics in Debug or silently truncates in ReleaseFast.
    // After fix: returns error.InputTooLarge before touching data.
    try std.testing.expectError(error.InputTooLarge, renderToHtml(fake, std.testing.allocator));
}

test "renderWithRenderer: rejects input > 4GB with InputTooLarge" {
    const huge_len: usize = @as(usize, std.math.maxInt(u32)) + 1;
    var sentinel: u8 = 0;
    const p: [*]const u8 = @ptrCast(&sentinel);
    const fake: []const u8 = p[0..huge_len];
    // Renderer is never called because the size check fires first.
    const noop_vtable = types.Renderer.VTable{
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
        .text = struct {
            fn f(_: *anyopaque, _: types.TextType, _: []const u8) types.CallbackError!void {}
        }.f,
    };
    var dummy: u8 = 0;
    const noop_rend = types.Renderer{ .ptr = &dummy, .vtable = &noop_vtable };
    try std.testing.expectError(
        error.InputTooLarge,
        renderWithRenderer(fake, std.testing.allocator, .{}, noop_rend),
    );
}
