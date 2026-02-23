// XML tag processing for ExtractionRenderer.
//
// Handles both block-level HTML fragments (pointers into source text) and
// inline HTML fragments (md4c internal buffer, requiring source offset recovery).
// Extracted from extraction_renderer.zig (marky-s64n) for module size management.

const std = @import("std");
const Allocator = std.mem.Allocator;
const ext_mod = @import("extraction_renderer.zig");
const ExtractedXmlTag = ext_mod.ExtractedXmlTag;

// ── Types ────────────────────────────────────────────────────────────

pub const HtmlFragment = struct {
    content: []const u8, // slice into src_text
    offset: u32, // byte offset in source
};

pub const OpenXmlTag = struct {
    tag_name: []const u8, // slice into fragment/tag text (not owned)
    raw_html: []const u8, // slice into fragment/tag text (not owned)
    offset: u32, // start byte in source
};

/// Inline HTML tag text captured from md4c callbacks.
/// Content is owned (duped from md4c's internal buffer which is freed after parsing).
pub const InlineHtmlTag = struct {
    text: []const u8, // owned copy of tag markup (e.g. "<agent>" or "</agent>")
};

// HTML5 void elements — self-closing even without />
pub const void_elements = [_][]const u8{
    "br", "hr", "img", "input", "meta", "link",
    "source", "track", "wbr", "area", "base",
    "col", "embed", "param",
};

// ── Helpers ──────────────────────────────────────────────────────────

pub fn parseTagName(html: []const u8) ?struct { name: []const u8, is_closing: bool } {
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

pub fn isVoidElement(name: []const u8) bool {
    for (&void_elements) |v| {
        if (std.ascii.eqlIgnoreCase(name, v)) return true;
    }
    return false;
}

// ── Block-level HTML fragment processing ─────────────────────────────

/// Process collected block-level HTML fragments into structured XML tag entries.
/// Fragments contain slices into source text with known byte offsets.
/// Returns true if OOM occurred.
pub fn processHtmlFragments(
    fragments: []const HtmlFragment,
    allocator: Allocator,
    xml_tags: *std.ArrayListUnmanaged(ExtractedXmlTag),
) bool {
    var xml_tag_stack: std.ArrayListUnmanaged(OpenXmlTag) = .{};
    defer xml_tag_stack.deinit(allocator);

    for (fragments) |frag| {
        // Scan through the fragment for all '<...>' tag occurrences
        var pos: usize = 0;
        while (pos < frag.content.len) {
            // Find next '<'
            const lt_pos = std.mem.indexOfScalarPos(u8, frag.content, pos, '<') orelse break;
            // Find matching '>'
            const gt_pos = std.mem.indexOfScalarPos(u8, frag.content, lt_pos + 1, '>') orelse break;

            const tag_slice = frag.content[lt_pos .. gt_pos + 1];
            const tag_offset = frag.offset +| @as(u32, @intCast(lt_pos));

            pos = gt_pos + 1;

            const parsed = parseTagName(tag_slice) orelse continue;

            if (parsed.is_closing) {
                if (emitClosingTag(allocator, xml_tags, &xml_tag_stack, parsed.name, tag_offset, tag_slice, false))
                    return true;
            } else {
                if (emitOpenOrSelfClosingTag(allocator, xml_tags, &xml_tag_stack, parsed.name, tag_slice, tag_offset, false))
                    return true;
            }
        }
    }

    // Finalize: remaining stack entries are unclosed tags
    return finalizeUnclosedTags(allocator, xml_tags, &xml_tag_stack, false);
}

// ── Inline HTML processing with source offset recovery ───────────────

/// Process collected inline HTML tag fragments into structured XML tag entries.
/// Inline HTML pointers are into md4c's internal buffer (not source text), so
/// this function scans the original source to recover byte offsets.
/// Skips fenced code blocks during source scan to avoid false matches.
/// Returns true if OOM occurred.
pub fn processInlineHtmlFragments(
    inline_tags: []const InlineHtmlTag,
    src_text: []const u8,
    allocator: Allocator,
    xml_tags: *std.ArrayListUnmanaged(ExtractedXmlTag),
) bool {
    if (inline_tags.len == 0) return false;

    var xml_tag_stack: std.ArrayListUnmanaged(OpenXmlTag) = .{};
    defer xml_tag_stack.deinit(allocator);

    var tag_idx: usize = 0;
    var pos: usize = 0;
    var in_fence: bool = false;

    while (pos < src_text.len and tag_idx < inline_tags.len) {
        // Track fenced code blocks at line starts
        const at_line_start = (pos == 0 or src_text[pos - 1] == '\n');
        if (at_line_start) {
            var fp = pos;
            // Allow up to 3 leading spaces
            while (fp < src_text.len and src_text[fp] == ' ' and fp - pos < 3) : (fp += 1) {}
            if (fp < src_text.len and (src_text[fp] == '`' or src_text[fp] == '~')) {
                const fence_char = src_text[fp];
                var fence_len: usize = 0;
                while (fp + fence_len < src_text.len and src_text[fp + fence_len] == fence_char) : (fence_len += 1) {}
                if (fence_len >= 3) {
                    in_fence = !in_fence;
                    // Skip to end of line
                    while (pos < src_text.len and src_text[pos] != '\n') : (pos += 1) {}
                    if (pos < src_text.len) pos += 1;
                    continue;
                }
            }
        }

        if (in_fence) {
            // Skip to next line
            while (pos < src_text.len and src_text[pos] != '\n') : (pos += 1) {}
            if (pos < src_text.len) pos += 1;
            continue;
        }

        // Try to match current inline tag text in source
        const tag_text = inline_tags[tag_idx].text;
        if (pos + tag_text.len <= src_text.len and
            std.mem.eql(u8, src_text[pos .. pos + tag_text.len], tag_text))
        {
            const source_offset: u32 = @intCast(pos);
            pos += tag_text.len;

            const parsed = parseTagName(tag_text) orelse {
                tag_idx += 1;
                continue;
            };

            if (parsed.is_closing) {
                if (emitClosingTag(allocator, xml_tags, &xml_tag_stack, parsed.name, source_offset, tag_text, true))
                    return true;
            } else {
                if (emitOpenOrSelfClosingTag(allocator, xml_tags, &xml_tag_stack, parsed.name, tag_text, source_offset, true))
                    return true;
            }

            tag_idx += 1;
        } else {
            pos += 1;
        }
    }

    // Finalize unclosed inline tags
    return finalizeUnclosedTags(allocator, xml_tags, &xml_tag_stack, true);
}

// ── Shared tag emission helpers ──────────────────────────────────────

/// Handle a closing tag by matching it with an open tag on the stack.
/// Returns true on OOM.
fn emitClosingTag(
    allocator: Allocator,
    xml_tags: *std.ArrayListUnmanaged(ExtractedXmlTag),
    xml_tag_stack: *std.ArrayListUnmanaged(OpenXmlTag),
    name: []const u8,
    tag_offset: u32,
    tag_slice: []const u8,
    is_inline: bool,
) bool {
    // Pop matching open tag from stack (innermost first, same-name)
    var match_idx: ?usize = null;
    var j = xml_tag_stack.items.len;
    while (j > 0) {
        j -= 1;
        if (std.ascii.eqlIgnoreCase(xml_tag_stack.items[j].tag_name, name)) {
            match_idx = j;
            break;
        }
    }
    if (match_idx) |idx| {
        const open = xml_tag_stack.orderedRemove(idx);
        const end_offset = tag_offset +| @as(u32, @intCast(tag_slice.len));

        const owned_name = allocator.dupe(u8, open.tag_name) catch return true;
        errdefer allocator.free(owned_name);
        const owned_html = allocator.dupe(u8, open.raw_html) catch return true;

        xml_tags.append(allocator, .{
            .tag_name = owned_name,
            .raw_html = owned_html,
            .offset = open.offset,
            .end_offset = end_offset,
            .is_self_closing = false,
            .is_unclosed = false,
            .is_inline = is_inline,
        }) catch {
            allocator.free(owned_name);
            allocator.free(owned_html);
            return true;
        };
    }
    // Unmatched close tags silently ignored
    return false;
}

/// Handle an open tag: emit self-closing or push to stack.
/// Returns true on OOM.
fn emitOpenOrSelfClosingTag(
    allocator: Allocator,
    xml_tags: *std.ArrayListUnmanaged(ExtractedXmlTag),
    xml_tag_stack: *std.ArrayListUnmanaged(OpenXmlTag),
    name: []const u8,
    tag_slice: []const u8,
    tag_offset: u32,
    is_inline: bool,
) bool {
    const is_self_closing = (tag_slice.len >= 2 and
        tag_slice[tag_slice.len - 2] == '/' and
        tag_slice[tag_slice.len - 1] == '>') or
        isVoidElement(name);

    if (is_self_closing) {
        const end_offset = tag_offset +| @as(u32, @intCast(tag_slice.len));

        const owned_name = allocator.dupe(u8, name) catch return true;
        errdefer allocator.free(owned_name);
        const owned_html = allocator.dupe(u8, tag_slice) catch return true;

        xml_tags.append(allocator, .{
            .tag_name = owned_name,
            .raw_html = owned_html,
            .offset = tag_offset,
            .end_offset = end_offset,
            .is_self_closing = true,
            .is_unclosed = false,
            .is_inline = is_inline,
        }) catch {
            allocator.free(owned_name);
            allocator.free(owned_html);
            return true;
        };
    } else {
        // Push to stack for matching
        xml_tag_stack.append(allocator, .{
            .tag_name = name,
            .raw_html = tag_slice,
            .offset = tag_offset,
        }) catch return true;
    }
    return false;
}

/// Emit remaining unclosed tags from the stack.
/// Returns true on OOM.
fn finalizeUnclosedTags(
    allocator: Allocator,
    xml_tags: *std.ArrayListUnmanaged(ExtractedXmlTag),
    xml_tag_stack: *std.ArrayListUnmanaged(OpenXmlTag),
    is_inline: bool,
) bool {
    for (xml_tag_stack.items) |open| {
        const end_offset = open.offset +| @as(u32, @intCast(open.raw_html.len));

        const owned_name = allocator.dupe(u8, open.tag_name) catch return true;
        errdefer allocator.free(owned_name);
        const owned_html = allocator.dupe(u8, open.raw_html) catch return true;

        xml_tags.append(allocator, .{
            .tag_name = owned_name,
            .raw_html = owned_html,
            .offset = open.offset,
            .end_offset = end_offset,
            .is_self_closing = false,
            .is_unclosed = true,
            .is_inline = is_inline,
        }) catch {
            allocator.free(owned_name);
            allocator.free(owned_html);
            return true;
        };
    }
    return false;
}
