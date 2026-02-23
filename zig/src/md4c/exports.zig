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

// ── C ABI Types (re-exported from ffi_types.zig) ────────────────────
const ffi_types = @import("ffi_types.zig");
pub const CMd4cHeading = ffi_types.CMd4cHeading;
pub const CMd4cLink = ffi_types.CMd4cLink;
pub const CMd4cCodeSpan = ffi_types.CMd4cCodeSpan;
pub const CMd4cTask = ffi_types.CMd4cTask;
pub const CMd4cEmbed = ffi_types.CMd4cEmbed;
pub const CMd4cCallout = ffi_types.CMd4cCallout;
pub const CMd4cBlockRef = ffi_types.CMd4cBlockRef;
pub const CMd4cQueryBlock = ffi_types.CMd4cQueryBlock;
pub const CMd4cLinkDefinition = ffi_types.CMd4cLinkDefinition;
pub const CMd4cProperty = ffi_types.CMd4cProperty;
pub const CMd4cResult = ffi_types.CMd4cResult;

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
    const code_span_count = result.code_spans.len;
    const task_count = result.tasks.len;
    const embed_count = result.embeds.len;
    const callout_count = result.callouts.len;
    const block_ref_count = result.block_refs.len;
    const query_block_count = result.query_blocks.len;
    const link_definition_count = result.link_definitions.len;
    const property_count = result.properties.len;

    // Calculate text blob size
    var blob_size: usize = 0;
    for (result.headings) |h| {
        blob_size += h.text.len;
    }
    for (result.links) |l| {
        blob_size += l.text.len;
        blob_size += l.target.len;
    }
    for (result.code_spans) |cs| {
        blob_size += cs.text.len;
    }
    for (result.tasks) |tk| {
        blob_size += tk.text.len;
    }
    for (result.embeds) |e| {
        blob_size += e.target.len;
    }
    for (result.callouts) |cl| {
        blob_size += cl.callout_type.len;
        if (cl.title) |ttl| blob_size += ttl.len;
    }
    for (result.block_refs) |br| {
        blob_size += br.uuid.len;
    }
    for (result.query_blocks) |qb| {
        blob_size += qb.query.len;
    }
    for (result.link_definitions) |ld| {
        blob_size += ld.label.len;
        blob_size += ld.url.len;
        if (ld.title) |ttl| blob_size += ttl.len;
    }
    for (result.properties) |p| {
        blob_size += p.key.len;
        blob_size += p.value.len;
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

    // Allocate code span array
    var c_code_spans: ?[]CMd4cCodeSpan = null;
    if (code_span_count > 0) {
        c_code_spans = ffi_allocator.alloc(CMd4cCodeSpan, code_span_count) catch {
            if (c_links) |l| ffi_allocator.free(l);
            if (c_headings) |h| ffi_allocator.free(h);
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Allocate task array
    var c_tasks: ?[]CMd4cTask = null;
    if (task_count > 0) {
        c_tasks = ffi_allocator.alloc(CMd4cTask, task_count) catch {
            if (c_code_spans) |cs| ffi_allocator.free(cs);
            if (c_links) |l| ffi_allocator.free(l);
            if (c_headings) |h| ffi_allocator.free(h);
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Allocate embed array
    var c_embeds: ?[]CMd4cEmbed = null;
    if (embed_count > 0) {
        c_embeds = ffi_allocator.alloc(CMd4cEmbed, embed_count) catch {
            if (c_tasks) |tk| ffi_allocator.free(tk);
            if (c_code_spans) |cs| ffi_allocator.free(cs);
            if (c_links) |l| ffi_allocator.free(l);
            if (c_headings) |h| ffi_allocator.free(h);
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Allocate callout array
    var c_callouts: ?[]CMd4cCallout = null;
    if (callout_count > 0) {
        c_callouts = ffi_allocator.alloc(CMd4cCallout, callout_count) catch {
            if (c_embeds) |em| ffi_allocator.free(em);
            if (c_tasks) |tk| ffi_allocator.free(tk);
            if (c_code_spans) |cs| ffi_allocator.free(cs);
            if (c_links) |l| ffi_allocator.free(l);
            if (c_headings) |h| ffi_allocator.free(h);
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Allocate block ref array
    var c_block_refs: ?[]CMd4cBlockRef = null;
    if (block_ref_count > 0) {
        c_block_refs = ffi_allocator.alloc(CMd4cBlockRef, block_ref_count) catch {
            if (c_callouts) |cl| ffi_allocator.free(cl);
            if (c_embeds) |em| ffi_allocator.free(em);
            if (c_tasks) |tk| ffi_allocator.free(tk);
            if (c_code_spans) |cs| ffi_allocator.free(cs);
            if (c_links) |l| ffi_allocator.free(l);
            if (c_headings) |h| ffi_allocator.free(h);
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Allocate query block array
    var c_query_blocks: ?[]CMd4cQueryBlock = null;
    if (query_block_count > 0) {
        c_query_blocks = ffi_allocator.alloc(CMd4cQueryBlock, query_block_count) catch {
            if (c_block_refs) |br| ffi_allocator.free(br);
            if (c_callouts) |cl| ffi_allocator.free(cl);
            if (c_embeds) |em| ffi_allocator.free(em);
            if (c_tasks) |tk| ffi_allocator.free(tk);
            if (c_code_spans) |cs| ffi_allocator.free(cs);
            if (c_links) |l| ffi_allocator.free(l);
            if (c_headings) |h| ffi_allocator.free(h);
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Allocate link definition array
    var c_link_definitions: ?[]CMd4cLinkDefinition = null;
    if (link_definition_count > 0) {
        c_link_definitions = ffi_allocator.alloc(CMd4cLinkDefinition, link_definition_count) catch {
            if (c_query_blocks) |qb| ffi_allocator.free(qb);
            if (c_block_refs) |br| ffi_allocator.free(br);
            if (c_callouts) |cl| ffi_allocator.free(cl);
            if (c_embeds) |em| ffi_allocator.free(em);
            if (c_tasks) |tk| ffi_allocator.free(tk);
            if (c_code_spans) |cs| ffi_allocator.free(cs);
            if (c_links) |l| ffi_allocator.free(l);
            if (c_headings) |h| ffi_allocator.free(h);
            if (blob) |b| ffi_allocator.free(b);
            result.deinit();
            return -4;
        };
    }

    // Allocate property array
    var c_properties: ?[]CMd4cProperty = null;
    if (property_count > 0) {
        c_properties = ffi_allocator.alloc(CMd4cProperty, property_count) catch {
            if (c_link_definitions) |ld| ffi_allocator.free(ld);
            if (c_query_blocks) |qb| ffi_allocator.free(qb);
            if (c_block_refs) |br| ffi_allocator.free(br);
            if (c_callouts) |cl| ffi_allocator.free(cl);
            if (c_embeds) |em| ffi_allocator.free(em);
            if (c_tasks) |tk| ffi_allocator.free(tk);
            if (c_code_spans) |cs| ffi_allocator.free(cs);
            if (c_links) |l| ffi_allocator.free(l);
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

    for (result.code_spans, 0..) |cs, i| {
        const text_len: u32 = @intCast(cs.text.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, text_len) <= b.len);
            @memcpy(b[blob_offset..][0..text_len], cs.text);
        }
        c_code_spans.?[i] = .{
            .source_offset = cs.offset,
            .end_offset = cs.end_offset,
            .text_offset = blob_offset,
            .text_length = text_len,
        };
        blob_offset += text_len;
    }

    for (result.tasks, 0..) |tk, i| {
        const text_len: u32 = @intCast(tk.text.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, text_len) <= b.len);
            @memcpy(b[blob_offset..][0..text_len], tk.text);
        }
        c_tasks.?[i] = .{
            .source_offset = tk.offset,
            .end_offset = tk.end_offset,
            .text_offset = blob_offset,
            .text_length = text_len,
            .state = tk.state,
        };
        blob_offset += text_len;
    }

    for (result.embeds, 0..) |e, i| {
        const target_len: u32 = @intCast(e.target.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, target_len) <= b.len);
            @memcpy(b[blob_offset..][0..target_len], e.target);
        }
        c_embeds.?[i] = .{
            .source_offset = e.offset,
            .end_offset = e.end_offset,
            .target_offset = blob_offset,
            .target_length = target_len,
        };
        blob_offset += target_len;
    }

    for (result.callouts, 0..) |cl, i| {
        const type_len: u32 = @intCast(cl.callout_type.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, type_len) <= b.len);
            @memcpy(b[blob_offset..][0..type_len], cl.callout_type);
        }
        const type_off = blob_offset;
        blob_offset += type_len;

        var title_off: u32 = 0;
        var title_len: u32 = 0;
        if (cl.title) |ttl| {
            title_len = @intCast(ttl.len);
            if (blob) |b| {
                std.debug.assert(@as(usize, blob_offset) + @as(usize, title_len) <= b.len);
                @memcpy(b[blob_offset..][0..title_len], ttl);
            }
            title_off = blob_offset;
            blob_offset += title_len;
        }

        c_callouts.?[i] = .{
            .source_offset = cl.offset,
            .end_offset = cl.end_offset,
            .type_offset = type_off,
            .type_length = type_len,
            .title_offset = title_off,
            .title_length = title_len,
        };
    }

    for (result.block_refs, 0..) |br, i| {
        const uuid_len: u32 = @intCast(br.uuid.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, uuid_len) <= b.len);
            @memcpy(b[blob_offset..][0..uuid_len], br.uuid);
        }
        c_block_refs.?[i] = .{
            .source_offset = br.offset,
            .uuid_offset = blob_offset,
            .uuid_length = uuid_len,
        };
        blob_offset += uuid_len;
    }

    for (result.query_blocks, 0..) |qb, i| {
        const query_len: u32 = @intCast(qb.query.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, query_len) <= b.len);
            @memcpy(b[blob_offset..][0..query_len], qb.query);
        }
        c_query_blocks.?[i] = .{
            .source_offset = qb.offset,
            .end_offset = qb.end_offset,
            .query_offset = blob_offset,
            .query_length = query_len,
        };
        blob_offset += query_len;
    }

    for (result.link_definitions, 0..) |ld, i| {
        const label_len: u32 = @intCast(ld.label.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, label_len) <= b.len);
            @memcpy(b[blob_offset..][0..label_len], ld.label);
        }
        const label_off = blob_offset;
        blob_offset += label_len;

        const url_len: u32 = @intCast(ld.url.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, url_len) <= b.len);
            @memcpy(b[blob_offset..][0..url_len], ld.url);
        }
        const url_off = blob_offset;
        blob_offset += url_len;

        var title_off: u32 = 0;
        var title_len: u32 = 0;
        if (ld.title) |ttl| {
            title_len = @intCast(ttl.len);
            if (blob) |b| {
                std.debug.assert(@as(usize, blob_offset) + @as(usize, title_len) <= b.len);
                @memcpy(b[blob_offset..][0..title_len], ttl);
            }
            title_off = blob_offset;
            blob_offset += title_len;
        }

        c_link_definitions.?[i] = .{
            .source_offset = ld.offset,
            .end_offset = ld.end_offset,
            .label_offset = label_off,
            .label_length = label_len,
            .url_offset = url_off,
            .url_length = url_len,
            .title_offset = title_off,
            .title_length = title_len,
        };
    }

    for (result.properties, 0..) |p, i| {
        const key_len: u32 = @intCast(p.key.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, key_len) <= b.len);
            @memcpy(b[blob_offset..][0..key_len], p.key);
        }
        const key_off = blob_offset;
        blob_offset += key_len;

        const value_len: u32 = @intCast(p.value.len);
        if (blob) |b| {
            std.debug.assert(@as(usize, blob_offset) + @as(usize, value_len) <= b.len);
            @memcpy(b[blob_offset..][0..value_len], p.value);
        }
        const value_off = blob_offset;
        blob_offset += value_len;

        c_properties.?[i] = .{
            .key_offset = key_off,
            .key_length = key_len,
            .value_offset = value_off,
            .value_length = value_len,
            .value_type = p.value_type,
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
        .code_spans = if (c_code_spans) |cs| cs.ptr else null,
        .code_spans_count = @intCast(code_span_count),
        .tasks = if (c_tasks) |tk| tk.ptr else null,
        .tasks_count = @intCast(task_count),
        .embeds = if (c_embeds) |e| e.ptr else null,
        .embeds_count = @intCast(embed_count),
        .callouts = if (c_callouts) |cl| cl.ptr else null,
        .callouts_count = @intCast(callout_count),
        .block_refs = if (c_block_refs) |br| br.ptr else null,
        .block_refs_count = @intCast(block_ref_count),
        .query_blocks = if (c_query_blocks) |qb| qb.ptr else null,
        .query_blocks_count = @intCast(query_block_count),
        .link_definitions = if (c_link_definitions) |ld| ld.ptr else null,
        .link_definitions_count = @intCast(link_definition_count),
        .properties = if (c_properties) |pr| pr.ptr else null,
        .properties_count = @intCast(property_count),
        .text_blob = blob_ptr,
        .text_blob_len = blob_offset,
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
    if (r.code_spans) |code_spans_ptr| {
        if (r.code_spans_count > 0) {
            ffi_allocator.free(code_spans_ptr[0..r.code_spans_count]);
        }
    }
    if (r.tasks) |tasks_ptr| {
        if (r.tasks_count > 0) {
            ffi_allocator.free(tasks_ptr[0..r.tasks_count]);
        }
    }
    if (r.embeds) |embeds_ptr| {
        if (r.embeds_count > 0) {
            ffi_allocator.free(embeds_ptr[0..r.embeds_count]);
        }
    }
    if (r.callouts) |callouts_ptr| {
        if (r.callouts_count > 0) {
            ffi_allocator.free(callouts_ptr[0..r.callouts_count]);
        }
    }
    if (r.block_refs) |block_refs_ptr| {
        if (r.block_refs_count > 0) {
            ffi_allocator.free(block_refs_ptr[0..r.block_refs_count]);
        }
    }
    if (r.query_blocks) |query_blocks_ptr| {
        if (r.query_blocks_count > 0) {
            ffi_allocator.free(query_blocks_ptr[0..r.query_blocks_count]);
        }
    }
    if (r.link_definitions) |link_definitions_ptr| {
        if (r.link_definitions_count > 0) {
            ffi_allocator.free(link_definitions_ptr[0..r.link_definitions_count]);
        }
    }
    if (r.properties) |properties_ptr| {
        if (r.properties_count > 0) {
            ffi_allocator.free(properties_ptr[0..r.properties_count]);
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

// --- Code span FFI tests (marky-pdyo) ---

test "md4c_extract: code span text via blob" {
    const input = "here is `hello` world\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.code_spans_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const cs = result.code_spans.?[0];
    try testing.expectEqualStrings("hello", blob[cs.text_offset..cs.text_offset + cs.text_length]);
    try testing.expectEqual(@as(u32, 8), cs.source_offset);
    try testing.expectEqual(@as(u32, 15), cs.end_offset);
}

test "md4c_extract: mixed document with code spans" {
    const input = "# Title `code` [link](url)\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.headings_count);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    try testing.expectEqual(@as(u32, 1), result.code_spans_count);
    // All blob offsets should be valid
    const blob = result.text_blob.?[0..result.text_blob_len];
    const cs = result.code_spans.?[0];
    try testing.expectEqualStrings("code", blob[cs.text_offset..cs.text_offset + cs.text_length]);
}

test "md4c_extract: no code spans" {
    const input = "Just plain text with no backticks.\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.code_spans_count);
}

// --- Task/Embed FFI tests (marky-rd7r) ---

test "md4c_extract: task via blob" {
    const input = "- [x] Done\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.tasks_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const tk = result.tasks.?[0];
    try testing.expectEqualStrings("Done", blob[tk.text_offset..tk.text_offset + tk.text_length]);
    try testing.expectEqual(@as(u8, 'x'), tk.state);
}

test "md4c_extract: embed via blob" {
    const input = "![[target]]\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.embeds_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const e = result.embeds.?[0];
    try testing.expectEqualStrings("target", blob[e.target_offset..e.target_offset + e.target_length]);
    // Also has a wikilink
    try testing.expectEqual(@as(u32, 1), result.links_count);
}

test "md4c_extract: no tasks or embeds" {
    const input = "Just plain text.\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.tasks_count);
    try testing.expectEqual(@as(u32, 0), result.embeds_count);
}

// --- Callout/BlockRef FFI tests (marky-1r0t) ---

test "md4c_extract: callout via blob" {
    const input = "> [!note]\n> Some content\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.callouts_count);
    const blob_slice = result.text_blob.?[0..result.text_blob_len];
    const cl = result.callouts.?[0];
    try testing.expectEqualStrings("note", blob_slice[cl.type_offset..cl.type_offset + cl.type_length]);
    try testing.expectEqual(@as(u32, 0), cl.title_length); // no title
}

test "md4c_extract: callout with title via blob" {
    const input = "> [!tip] My Title\n> Content\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.callouts_count);
    const blob_slice = result.text_blob.?[0..result.text_blob_len];
    const cl = result.callouts.?[0];
    try testing.expectEqualStrings("tip", blob_slice[cl.type_offset..cl.type_offset + cl.type_length]);
    try testing.expectEqualStrings("My Title", blob_slice[cl.title_offset..cl.title_offset + cl.title_length]);
}

test "md4c_extract: block ref via blob" {
    const input = "Text ((a1b2c3d4-e5f6-7890-abcd-ef1234567890)) more\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.block_refs_count);
    const blob_slice = result.text_blob.?[0..result.text_blob_len];
    const br = result.block_refs.?[0];
    try testing.expectEqualStrings("a1b2c3d4-e5f6-7890-abcd-ef1234567890", blob_slice[br.uuid_offset..br.uuid_offset + br.uuid_length]);
}

test "md4c_extract: no callouts or block refs" {
    const input = "Just plain text.\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.callouts_count);
    try testing.expectEqual(@as(u32, 0), result.block_refs_count);
}
