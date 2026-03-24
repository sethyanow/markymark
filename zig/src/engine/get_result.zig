// Build and free CEngineResult FFI payloads from DocumentEngine state.

const std = @import("std");
const DocumentEngine = @import("document.zig").DocumentEngine;
const ffi = @import("ffi_types.zig");

const ffi_allocator = std.heap.page_allocator;

pub const Error = error{
    OutOfMemory,
    Overflow,
};

pub fn getResult(engine: *const DocumentEngine, out: *ffi.CEngineResult) Error!void {
    out.* = std.mem.zeroes(ffi.CEngineResult);

    const headings_count: u32 = try toU32(engine.headings.len);
    const links_count: u32 = try toU32(engine.links.len);
    const code_spans_count: u32 = try toU32(engine.code_spans.len);
    const tags_count: u32 = try toU32(engine.tags.len);
    const block_ids_count: u32 = try toU32(engine.block_ids.len);
    const tasks_count: u32 = try toU32(engine.tasks.len);
    const embeds_count: u32 = try toU32(engine.embeds.len);
    const callouts_count: u32 = try toU32(engine.callouts.len);
    const block_refs_count: u32 = try toU32(engine.block_refs.len);
    const query_blocks_count: u32 = try toU32(engine.query_blocks.len);
    const link_definitions_count: u32 = try toU32(engine.link_definitions.len);
    const properties_count: u32 = try toU32(engine.properties.len);
    const xml_tags_count: u32 = try toU32(engine.xml_tags.len);
    const line_starts_count: u32 = try toU32(engine.line_starts.len);

    const blob_size = try computeTextBlobSize(engine);

    var text_blob: ?[]u8 = null;
    if (blob_size > 0) {
        text_blob = ffi_allocator.alloc(u8, blob_size) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(text_blob.?);
    }

    var c_headings: ?[]ffi.CEngineHeading = null;
    if (engine.headings.len > 0) {
        c_headings = ffi_allocator.alloc(ffi.CEngineHeading, engine.headings.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_headings.?);
    }

    var c_links: ?[]ffi.CEngineLink = null;
    if (engine.links.len > 0) {
        c_links = ffi_allocator.alloc(ffi.CEngineLink, engine.links.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_links.?);
    }

    var c_code_spans: ?[]ffi.CEngineCodeSpan = null;
    if (engine.code_spans.len > 0) {
        c_code_spans = ffi_allocator.alloc(ffi.CEngineCodeSpan, engine.code_spans.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_code_spans.?);
    }

    var c_tags: ?[]ffi.CEngineTag = null;
    if (engine.tags.len > 0) {
        c_tags = ffi_allocator.alloc(ffi.CEngineTag, engine.tags.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_tags.?);
    }

    var c_block_ids: ?[]ffi.CEngineBlockId = null;
    if (engine.block_ids.len > 0) {
        c_block_ids = ffi_allocator.alloc(ffi.CEngineBlockId, engine.block_ids.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_block_ids.?);
    }

    var c_tasks: ?[]ffi.CEngineTask = null;
    if (engine.tasks.len > 0) {
        c_tasks = ffi_allocator.alloc(ffi.CEngineTask, engine.tasks.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_tasks.?);
    }

    var c_embeds: ?[]ffi.CEngineEmbed = null;
    if (engine.embeds.len > 0) {
        c_embeds = ffi_allocator.alloc(ffi.CEngineEmbed, engine.embeds.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_embeds.?);
    }

    var c_callouts: ?[]ffi.CEngineCallout = null;
    if (engine.callouts.len > 0) {
        c_callouts = ffi_allocator.alloc(ffi.CEngineCallout, engine.callouts.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_callouts.?);
    }

    var c_block_refs: ?[]ffi.CEngineBlockRef = null;
    if (engine.block_refs.len > 0) {
        c_block_refs = ffi_allocator.alloc(ffi.CEngineBlockRef, engine.block_refs.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_block_refs.?);
    }

    var c_query_blocks: ?[]ffi.CEngineQueryBlock = null;
    if (engine.query_blocks.len > 0) {
        c_query_blocks = ffi_allocator.alloc(ffi.CEngineQueryBlock, engine.query_blocks.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_query_blocks.?);
    }

    var c_link_definitions: ?[]ffi.CEngineLinkDefinition = null;
    if (engine.link_definitions.len > 0) {
        c_link_definitions = ffi_allocator.alloc(ffi.CEngineLinkDefinition, engine.link_definitions.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_link_definitions.?);
    }

    var c_properties: ?[]ffi.CEngineProperty = null;
    if (engine.properties.len > 0) {
        c_properties = ffi_allocator.alloc(ffi.CEngineProperty, engine.properties.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_properties.?);
    }

    var c_xml_tags: ?[]ffi.CEngineXmlTag = null;
    if (engine.xml_tags.len > 0) {
        c_xml_tags = ffi_allocator.alloc(ffi.CEngineXmlTag, engine.xml_tags.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_xml_tags.?);
    }

    var c_line_starts: ?[]u32 = null;
    if (engine.line_starts.len > 0) {
        c_line_starts = ffi_allocator.alloc(u32, engine.line_starts.len) catch return error.OutOfMemory;
        errdefer ffi_allocator.free(c_line_starts.?);
        @memcpy(c_line_starts.?, engine.line_starts);
    }

    var blob_offset: u32 = 0;

    if (c_headings) |arr| {
        for (engine.headings, 0..) |h, i| {
            const text_offset = try packText(text_blob, &blob_offset, h.text);
            const slug_offset = try packText(text_blob, &blob_offset, h.slug);
            arr[i] = .{
                .text_offset = text_offset,
                .text_length = try toU32(h.text.len),
                .slug_offset = slug_offset,
                .slug_length = try toU32(h.slug.len),
                .source_offset = h.source_offset,
                .start_line = h.start.line,
                .start_col = h.start.col,
                .end_line = h.end.line,
                .end_col = h.end.col,
                .level = h.level,
                ._pad = .{ 0, 0, 0 },
            };
        }
    }

    if (c_links) |arr| {
        for (engine.links, 0..) |l, i| {
            const text_offset = try packText(text_blob, &blob_offset, l.text);
            const target_offset = try packText(text_blob, &blob_offset, l.target);
            arr[i] = .{
                .text_offset = text_offset,
                .text_length = try toU32(l.text.len),
                .target_offset = target_offset,
                .target_length = try toU32(l.target.len),
                .source_offset = l.source_offset,
                .start_line = l.start.line,
                .start_col = l.start.col,
                .end_line = l.end.line,
                .end_col = l.end.col,
                .is_wiki = if (l.is_wiki) 1 else 0,
                ._pad = .{ 0, 0, 0 },
            };
        }
    }

    if (c_code_spans) |arr| {
        for (engine.code_spans, 0..) |cs, i| {
            const text_offset = try packText(text_blob, &blob_offset, cs.text);
            arr[i] = .{
                .text_offset = text_offset,
                .text_length = try toU32(cs.text.len),
                .source_offset = cs.source_offset,
                .end_offset = cs.end_offset,
                .start_line = cs.start.line,
                .start_col = cs.start.col,
                .end_line = cs.end.line,
                .end_col = cs.end.col,
            };
        }
    }

    if (c_tags) |arr| {
        for (engine.tags, 0..) |tag, i| {
            const name_offset = try packText(text_blob, &blob_offset, tag.name);
            arr[i] = .{
                .name_offset = name_offset,
                .name_length = try toU32(tag.name.len),
                .source_offset = tag.source_offset,
                .start_line = tag.start.line,
                .start_col = tag.start.col,
            };
        }
    }

    if (c_block_ids) |arr| {
        for (engine.block_ids, 0..) |bid, i| {
            const id_offset = try packText(text_blob, &blob_offset, bid.id);
            arr[i] = .{
                .id_offset = id_offset,
                .id_length = try toU32(bid.id.len),
                .source_offset = bid.source_offset,
                .start_line = bid.start.line,
                .start_col = bid.start.col,
                .end_line = bid.end.line,
                .end_col = bid.end.col,
            };
        }
    }

    if (c_tasks) |arr| {
        for (engine.tasks, 0..) |task, i| {
            const text_offset = try packText(text_blob, &blob_offset, task.text);
            arr[i] = .{
                .text_offset = text_offset,
                .text_length = try toU32(task.text.len),
                .source_offset = task.source_offset,
                .end_offset = task.end_offset,
                .start_line = task.start.line,
                .start_col = task.start.col,
                .end_line = task.end.line,
                .end_col = task.end.col,
                .state = task.state,
                ._pad = .{ 0, 0, 0 },
            };
        }
    }

    if (c_embeds) |arr| {
        for (engine.embeds, 0..) |embed, i| {
            const target_offset = try packText(text_blob, &blob_offset, embed.target);
            arr[i] = .{
                .target_offset = target_offset,
                .target_length = try toU32(embed.target.len),
                .source_offset = embed.source_offset,
                .end_offset = embed.end_offset,
                .start_line = embed.start.line,
                .start_col = embed.start.col,
                .end_line = embed.end.line,
                .end_col = embed.end.col,
            };
        }
    }

    if (c_callouts) |arr| {
        for (engine.callouts, 0..) |callout, i| {
            const type_offset = try packText(text_blob, &blob_offset, callout.callout_type);
            const title_offset = if (callout.title) |title| try packText(text_blob, &blob_offset, title) else 0;
            const title_length = if (callout.title) |title| try toU32(title.len) else 0;
            arr[i] = .{
                .type_offset = type_offset,
                .type_length = try toU32(callout.callout_type.len),
                .title_offset = title_offset,
                .title_length = title_length,
                .source_offset = callout.source_offset,
                .end_offset = callout.end_offset,
                .start_line = callout.start.line,
                .start_col = callout.start.col,
                .end_line = callout.end.line,
                .end_col = callout.end.col,
            };
        }
    }

    if (c_block_refs) |arr| {
        for (engine.block_refs, 0..) |block_ref, i| {
            const uuid_offset = try packText(text_blob, &blob_offset, block_ref.uuid);
            arr[i] = .{
                .uuid_offset = uuid_offset,
                .uuid_length = try toU32(block_ref.uuid.len),
                .source_offset = block_ref.source_offset,
                .start_line = block_ref.start.line,
                .start_col = block_ref.start.col,
                .end_line = block_ref.end.line,
                .end_col = block_ref.end.col,
            };
        }
    }

    if (c_query_blocks) |arr| {
        for (engine.query_blocks, 0..) |query_block, i| {
            const query_offset = try packText(text_blob, &blob_offset, query_block.query);
            arr[i] = .{
                .query_offset = query_offset,
                .query_length = try toU32(query_block.query.len),
                .source_offset = query_block.source_offset,
                .end_offset = query_block.end_offset,
                .start_line = query_block.start.line,
                .start_col = query_block.start.col,
                .end_line = query_block.end.line,
                .end_col = query_block.end.col,
            };
        }
    }

    if (c_link_definitions) |arr| {
        for (engine.link_definitions, 0..) |link_def, i| {
            const label_offset = try packText(text_blob, &blob_offset, link_def.label);
            const url_offset = try packText(text_blob, &blob_offset, link_def.url);
            const title_offset = if (link_def.title) |title| try packText(text_blob, &blob_offset, title) else 0;
            const title_length = if (link_def.title) |title| try toU32(title.len) else 0;
            arr[i] = .{
                .label_offset = label_offset,
                .label_length = try toU32(link_def.label.len),
                .url_offset = url_offset,
                .url_length = try toU32(link_def.url.len),
                .title_offset = title_offset,
                .title_length = title_length,
                .source_offset = link_def.source_offset,
                .end_offset = link_def.end_offset,
                .start_line = link_def.start.line,
                .start_col = link_def.start.col,
                .end_line = link_def.end.line,
                .end_col = link_def.end.col,
            };
        }
    }

    if (c_properties) |arr| {
        for (engine.properties, 0..) |property, i| {
            const key_offset = try packText(text_blob, &blob_offset, property.key);
            const value_offset = try packText(text_blob, &blob_offset, property.value);
            arr[i] = .{
                .key_offset = key_offset,
                .key_length = try toU32(property.key.len),
                .value_offset = value_offset,
                .value_length = try toU32(property.value.len),
                .value_type = property.value_type,
                ._pad = .{ 0, 0, 0 },
            };
        }
    }

    if (c_xml_tags) |arr| {
        for (engine.xml_tags, 0..) |xml_tag, i| {
            const tag_name_offset = try packText(text_blob, &blob_offset, xml_tag.tag_name);
            const raw_html_offset = try packText(text_blob, &blob_offset, xml_tag.raw_html);
            arr[i] = .{
                .tag_name_offset = tag_name_offset,
                .tag_name_length = try toU32(xml_tag.tag_name.len),
                .raw_html_offset = raw_html_offset,
                .raw_html_length = try toU32(xml_tag.raw_html.len),
                .source_offset = xml_tag.source_offset,
                .end_offset = xml_tag.end_offset,
                .start_line = xml_tag.start.line,
                .start_col = xml_tag.start.col,
                .end_line = xml_tag.end.line,
                .end_col = xml_tag.end.col,
                .is_self_closing = if (xml_tag.is_self_closing) 1 else 0,
                .is_unclosed = if (xml_tag.is_unclosed) 1 else 0,
                .is_inline = if (xml_tag.is_inline) 1 else 0,
                ._pad = .{0},
            };
        }
    }

    out.* = .{
        .headings = if (c_headings) |arr| arr.ptr else null,
        .links = if (c_links) |arr| arr.ptr else null,
        .code_spans = if (c_code_spans) |arr| arr.ptr else null,
        .tags = if (c_tags) |arr| arr.ptr else null,
        .block_ids = if (c_block_ids) |arr| arr.ptr else null,
        .tasks = if (c_tasks) |arr| arr.ptr else null,
        .embeds = if (c_embeds) |arr| arr.ptr else null,
        .callouts = if (c_callouts) |arr| arr.ptr else null,
        .block_refs = if (c_block_refs) |arr| arr.ptr else null,
        .query_blocks = if (c_query_blocks) |arr| arr.ptr else null,
        .link_definitions = if (c_link_definitions) |arr| arr.ptr else null,
        .properties = if (c_properties) |arr| arr.ptr else null,
        .xml_tags = if (c_xml_tags) |arr| arr.ptr else null,
        .line_starts = if (c_line_starts) |arr| arr.ptr else null,
        .text_blob = if (text_blob) |b| b.ptr else null,

        .content_hash = engine.content_hash,
        .generation = engine.getGeneration(),

        .headings_count = headings_count,
        .links_count = links_count,
        .code_spans_count = code_spans_count,
        .tags_count = tags_count,
        .block_ids_count = block_ids_count,
        .tasks_count = tasks_count,
        .embeds_count = embeds_count,
        .callouts_count = callouts_count,
        .block_refs_count = block_refs_count,
        .query_blocks_count = query_blocks_count,
        .link_definitions_count = link_definitions_count,
        .properties_count = properties_count,
        .xml_tags_count = xml_tags_count,
        .line_starts_count = line_starts_count,
        .text_blob_len = blob_offset,
        .token_estimate = engine.token_estimate,

        ._reserved = [_]u8{0} ** 32,
    };
}

pub fn freeResult(result: *ffi.CEngineResult) void {
    freeArray(ffi.CEngineHeading, result.headings, result.headings_count);
    freeArray(ffi.CEngineLink, result.links, result.links_count);
    freeArray(ffi.CEngineCodeSpan, result.code_spans, result.code_spans_count);
    freeArray(ffi.CEngineTag, result.tags, result.tags_count);
    freeArray(ffi.CEngineBlockId, result.block_ids, result.block_ids_count);
    freeArray(ffi.CEngineTask, result.tasks, result.tasks_count);
    freeArray(ffi.CEngineEmbed, result.embeds, result.embeds_count);
    freeArray(ffi.CEngineCallout, result.callouts, result.callouts_count);
    freeArray(ffi.CEngineBlockRef, result.block_refs, result.block_refs_count);
    freeArray(ffi.CEngineQueryBlock, result.query_blocks, result.query_blocks_count);
    freeArray(
        ffi.CEngineLinkDefinition,
        result.link_definitions,
        result.link_definitions_count,
    );
    freeArray(ffi.CEngineProperty, result.properties, result.properties_count);
    freeArray(ffi.CEngineXmlTag, result.xml_tags, result.xml_tags_count);
    freeArray(u32, result.line_starts, result.line_starts_count);

    if (result.text_blob) |ptr| {
        if (result.text_blob_len > 0) {
            ffi_allocator.free(@constCast(ptr[0..result.text_blob_len]));
        }
    }

    result.* = std.mem.zeroes(ffi.CEngineResult);
}

fn freeArray(comptime T: type, ptr_opt: ?[*]T, count: u32) void {
    if (ptr_opt) |ptr| {
        if (count > 0) {
            ffi_allocator.free(ptr[0..count]);
        }
    }
}

fn packText(blob_opt: ?[]u8, offset: *u32, text: []const u8) Error!u32 {
    const len_u32 = try toU32(text.len);
    const start = offset.*;
    const end_u64 = @as(u64, start) + @as(u64, len_u32);
    if (end_u64 > std.math.maxInt(u32)) return error.Overflow;
    const end: u32 = @intCast(end_u64);

    if (blob_opt) |blob| {
        const start_usize: usize = @intCast(start);
        const end_usize: usize = @intCast(end);
        std.debug.assert(end_usize <= blob.len);
        @memcpy(blob[start_usize..end_usize], text);
    }

    offset.* = end;
    return start;
}

fn toU32(value: usize) Error!u32 {
    if (value > std.math.maxInt(u32)) return error.Overflow;
    return @intCast(value);
}

fn addBlobTextSize(total: *u64, text: []const u8) Error!void {
    const text_len_u64: u64 = @intCast(text.len);
    const next = total.* + text_len_u64;
    if (next > std.math.maxInt(u32)) return error.Overflow;
    total.* = next;
}

fn computeTextBlobSize(engine: *const DocumentEngine) Error!usize {
    var total: u64 = 0;

    for (engine.headings) |h| {
        try addBlobTextSize(&total, h.text);
        try addBlobTextSize(&total, h.slug);
    }
    for (engine.links) |l| {
        try addBlobTextSize(&total, l.text);
        try addBlobTextSize(&total, l.target);
    }
    for (engine.code_spans) |cs| {
        try addBlobTextSize(&total, cs.text);
    }
    for (engine.tags) |tag| {
        try addBlobTextSize(&total, tag.name);
    }
    for (engine.block_ids) |bid| {
        try addBlobTextSize(&total, bid.id);
    }
    for (engine.tasks) |task| {
        try addBlobTextSize(&total, task.text);
    }
    for (engine.embeds) |embed| {
        try addBlobTextSize(&total, embed.target);
    }
    for (engine.callouts) |callout| {
        try addBlobTextSize(&total, callout.callout_type);
        if (callout.title) |title| {
            try addBlobTextSize(&total, title);
        }
    }
    for (engine.block_refs) |block_ref| {
        try addBlobTextSize(&total, block_ref.uuid);
    }
    for (engine.query_blocks) |query_block| {
        try addBlobTextSize(&total, query_block.query);
    }
    for (engine.link_definitions) |link_def| {
        try addBlobTextSize(&total, link_def.label);
        try addBlobTextSize(&total, link_def.url);
        if (link_def.title) |title| {
            try addBlobTextSize(&total, title);
        }
    }
    for (engine.properties) |property| {
        try addBlobTextSize(&total, property.key);
        try addBlobTextSize(&total, property.value);
    }
    for (engine.xml_tags) |xml_tag| {
        try addBlobTextSize(&total, xml_tag.tag_name);
        try addBlobTextSize(&total, xml_tag.raw_html);
    }

    return @intCast(total);
}
