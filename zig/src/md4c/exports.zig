// C ABI exports for md4c ExtractionRenderer.
// Enables Rust FFI bindings in markymark-kernels to call the single-pass
// md4c extraction pipeline. Created for marky-6zl8.

const std = @import("std");
const extraction_renderer = @import("extraction_renderer.zig");
const extractFromMarkdown = extraction_renderer.extractFromMarkdown;
const ExtractionResult = extraction_renderer.ExtractionResult;

// ── Allocator ────────────────────────────────────────────────────────
// page_allocator is thread-safe and matches the existing FFI pattern
// (see exports_embed.zig). No global state — each call is independent.
const ffi_allocator = std.heap.page_allocator;

// ── C ABI Types ──────────────────────────────────────────────────────
// Fields ordered by alignment to avoid implicit padding holes.
// Both Zig extern struct and Rust #[repr(C)] MUST use identical field order.

pub const CMd4cHeading = extern struct {
    source_offset: u32, // byte offset of '#' (ATX) or text start (setext) in source
    text_offset: u32, // offset into text_blob for decoded heading text
    text_length: u32, // length in text_blob
    level: u8, // 1-6
    _padding: [3]u8, // explicit padding to 16-byte struct size
};
comptime {
    std.debug.assert(@sizeOf(CMd4cHeading) == 16);
}

pub const CMd4cLink = extern struct {
    source_offset: u32, // byte offset of '[' or '[[' in source
    text_offset: u32, // offset into text_blob for display text
    target_offset: u32, // offset into text_blob for href/target
    text_length: u32, // length in text_blob
    target_length: u32, // length in text_blob
    is_wiki: u8, // 1 for [[wiki]] links, 0 otherwise
    _padding: [3]u8, // explicit padding to 24-byte struct size
};
comptime {
    std.debug.assert(@sizeOf(CMd4cLink) == 24);
}

// Pointers grouped first, then u32 counts — avoids internal padding on 64-bit.
pub const CMd4cResult = extern struct {
    headings: ?[*]CMd4cHeading, // Zig-allocated array, freed by marky_md4c_free
    links: ?[*]CMd4cLink, // Zig-allocated array, freed by marky_md4c_free
    text_blob: ?[*]const u8, // concatenated decoded texts, freed by marky_md4c_free
    headings_count: u32,
    links_count: u32,
    text_blob_len: u32,
    _padding: u32, // explicit padding to 40 bytes (8-byte alignment)
};
comptime {
    std.debug.assert(@sizeOf(CMd4cResult) == 40);
}

// ── C ABI Functions ──────────────────────────────────────────────────

/// Extract headings and links from markdown text in a single pass.
///
/// Returns: 0=success, -1=null pointer, -3=parse error, -4=out of memory,
///          -5=overflow (total extracted text exceeds u32 limit).
/// On success, `out` is populated with Zig-allocated arrays that MUST be
/// freed by calling `marky_md4c_free`.
export fn marky_md4c_extract(text: ?[*]const u8, len: u32, out: ?*CMd4cResult) i32 {
    const out_ptr = out orelse return -1;

    // Zero out result immediately (safe default on any error path)
    out_ptr.* = std.mem.zeroes(CMd4cResult);

    if (len == 0) {
        // Empty input is valid — zero results, no allocations needed
        if (text == null) return -1;
        return 0;
    }

    const t = text orelse return -1;
    const input = t[0..len];

    // Run the extraction
    var result = extractFromMarkdown(input, ffi_allocator) catch |err| {
        return switch (err) {
            error.OutOfMemory => @as(i32, -4),
            error.InputTooLarge => @as(i32, -5),
            else => @as(i32, -3),
        };
    };

    const heading_count = result.headings.len;
    const link_count = result.links.len;

    // Calculate text blob size
    var blob_size: usize = 0;
    for (result.headings) |h| {
        blob_size += h.text.len;
    }
    for (result.links) |l| {
        blob_size += l.text.len;
        blob_size += l.target.len;
    }

    // T1-3: blob_offset is u32 — guard against wrapping for documents whose total
    // extracted text exceeds 4 GiB. blob_size is usize (full-width), so this check
    // is safe on all targets.
    if (blob_size > std.math.maxInt(u32)) {
        result.deinit();
        return -5;
    }

    // Allocate text blob (skip if nothing to pack)
    var blob: ?[]u8 = null;
    if (blob_size > 0) {
        blob = ffi_allocator.alloc(u8, blob_size) catch {
            result.deinit();
            return -4;
        };
    }

    // Allocate heading array
    var c_headings: ?[]CMd4cHeading = null;
    if (heading_count > 0) {
        c_headings = ffi_allocator.alloc(CMd4cHeading, heading_count) catch {
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Allocate link array
    var c_links: ?[]CMd4cLink = null;
    if (link_count > 0) {
        c_links = ffi_allocator.alloc(CMd4cLink, link_count) catch {
            if (c_headings) |h| ffi_allocator.free(h);
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Pack data into blob and fill C structs
    var blob_offset: u32 = 0;
    const blob_ptr = if (blob) |b| b.ptr else null;

    for (result.headings, 0..) |h, i| {
        const text_len: u32 = @intCast(h.text.len);
        if (blob) |b| {
            // T3-2: Defensive bounds invariant — blob_size guard ensures this holds.
            std.debug.assert(@as(usize, blob_offset) + @as(usize, text_len) <= b.len);
            @memcpy(b[blob_offset..][0..text_len], h.text);
        }
        c_headings.?[i] = .{
            .source_offset = h.offset,
            .text_offset = blob_offset,
            .text_length = text_len,
            .level = h.level,
            ._padding = .{ 0, 0, 0 },
        };
        blob_offset += text_len;
    }

    for (result.links, 0..) |l, i| {
        const text_len: u32 = @intCast(l.text.len);
        const target_len: u32 = @intCast(l.target.len);
        if (blob) |b| {
            // T3-2: Defensive bounds invariant — blob_size guard ensures this holds.
            std.debug.assert(@as(usize, blob_offset) + @as(usize, text_len) <= b.len);
            @memcpy(b[blob_offset..][0..text_len], l.text);
        }
        const text_off = blob_offset;
        blob_offset += text_len;

        if (blob) |b| {
            // T3-2: Defensive bounds invariant — blob_size guard ensures this holds.
            std.debug.assert(@as(usize, blob_offset) + @as(usize, target_len) <= b.len);
            @memcpy(b[blob_offset..][0..target_len], l.target);
        }
        const target_off = blob_offset;
        blob_offset += target_len;

        c_links.?[i] = .{
            .source_offset = l.offset,
            .text_offset = text_off,
            .target_offset = target_off,
            .text_length = text_len,
            .target_length = target_len,
            .is_wiki = if (l.is_wiki) 1 else 0,
            ._padding = .{ 0, 0, 0 },
        };
    }

    // Free ExtractionResult (owned strings — already copied to blob)
    result.deinit();

    // Write output
    out_ptr.* = .{
        .headings = if (c_headings) |h| h.ptr else null,
        .headings_count = @intCast(heading_count),
        .links = if (c_links) |l| l.ptr else null,
        .links_count = @intCast(link_count),
        .text_blob = blob_ptr,
        .text_blob_len = blob_offset,
        ._padding = 0,
    };

    return 0;
}

/// Free all Zig-allocated memory in a CMd4cResult.
///
/// After this call the result is zeroed (double-free is a no-op).
/// Passing null is a no-op.
export fn marky_md4c_free(result: ?*CMd4cResult) void {
    const r = result orelse return;

    if (r.headings) |headings_ptr| {
        if (r.headings_count > 0) {
            ffi_allocator.free(headings_ptr[0..r.headings_count]);
        }
    }
    if (r.links) |links_ptr| {
        if (r.links_count > 0) {
            ffi_allocator.free(links_ptr[0..r.links_count]);
        }
    }
    if (r.text_blob) |blob_ptr| {
        if (r.text_blob_len > 0) {
            ffi_allocator.free(@constCast(blob_ptr[0..r.text_blob_len]));
        }
    }

    // Zero out to prevent double-free
    r.* = std.mem.zeroes(CMd4cResult);
}

// ── Tests ────────────────────────────────────────────────────────────

const testing = std.testing;

test "md4c_extract: simple heading" {
    const input = "# Hello\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.headings_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const h = result.headings.?[0];
    try testing.expectEqualStrings("Hello", blob[h.text_offset..h.text_offset + h.text_length]);
    try testing.expectEqual(@as(u8, 1), h.level);
}

test "md4c_extract: inline link with text and target" {
    const input = "[click](https://example.com)\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const l = result.links.?[0];
    try testing.expectEqualStrings("click", blob[l.text_offset..l.text_offset + l.text_length]);
    try testing.expectEqualStrings("https://example.com", blob[l.target_offset..l.target_offset + l.target_length]);
    try testing.expectEqual(@as(u8, 0), l.is_wiki);
}

test "md4c_extract: null text pointer returns -1" {
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(null, 10, &result);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "md4c_extract: null out pointer returns -1" {
    const input = "# Hello\n";
    const rc = marky_md4c_extract(input.ptr, input.len, null);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "md4c_extract: empty input returns zero results" {
    const input = "";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, 0, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.headings_count);
    try testing.expectEqual(@as(u32, 0), result.links_count);
}

test "md4c_extract: wiki link" {
    const input = "[[Target]]\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    try testing.expectEqual(@as(u8, 1), result.links.?[0].is_wiki);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const l = result.links.?[0];
    try testing.expectEqualStrings("Target", blob[l.target_offset..l.target_offset + l.target_length]);
}

test "md4c_extract: double free is no-op" {
    const input = "# Test\n";
    var result: CMd4cResult = undefined;
    _ = marky_md4c_extract(input.ptr, input.len, &result);
    marky_md4c_free(&result);
    // Second free should be no-op (result zeroed by first free)
    marky_md4c_free(&result);
}

test "md4c_extract: entity text decoded in heading" {
    // Entity references are decoded to UTF-8 by ExtractionRenderer (marky-yfh7)
    const input = "# Hello &amp; World\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const h = result.headings.?[0];
    try testing.expectEqualStrings("Hello & World", blob[h.text_offset..h.text_offset + h.text_length]);
}

test "md4c_extract: mixed document headings and links" {
    const input = "# Title\n\nSome [link](url) text.\n\n## Section\n\nSee [[Wiki]] for details.\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 2), result.headings_count);
    try testing.expectEqual(@as(u32, 2), result.links_count);

    const blob = result.text_blob.?[0..result.text_blob_len];
    const h0 = result.headings.?[0];
    const h1 = result.headings.?[1];
    try testing.expectEqualStrings("Title", blob[h0.text_offset..h0.text_offset + h0.text_length]);
    try testing.expectEqualStrings("Section", blob[h1.text_offset..h1.text_offset + h1.text_length]);

    // Second link should be wiki
    try testing.expectEqual(@as(u8, 1), result.links.?[1].is_wiki);
}

test "md4c_extract: null text with zero len returns -1" {
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(null, 0, &result);
    try testing.expectEqual(@as(i32, -1), rc);
}
