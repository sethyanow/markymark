// Blob serialization for DocumentEngine.
//
// Serializes engine state into a flat binary blob for zero-copy FFI transfer.
// Extracted from document.zig (marky-7kmo) for module size management.

const std = @import("std");
const blob = @import("blob.zig");
const document = @import("document.zig");

const DocumentEngine = document.DocumentEngine;

pub fn serializeState(engine: *const DocumentEngine) ![]u8 {
    // Defense-in-depth: guard against @intCast trap on >u32::MAX element counts.
    // Physically impossible (would require hundreds of GB of RAM), but prevents
    // an uncatchable panic if invariants are ever violated. Must be checked before
    // the text_pool_size loop which iterates these slices.
    const max_u32 = std.math.maxInt(u32);
    if (engine.headings.len > max_u32 or
        engine.links.len > max_u32 or
        engine.code_spans.len > max_u32 or
        engine.tags.len > max_u32 or
        engine.block_ids.len > max_u32 or
        engine.tasks.len > max_u32 or
        engine.embeds.len > max_u32 or
        engine.callouts.len > max_u32 or
        engine.block_refs.len > max_u32 or
        engine.line_starts.len > max_u32) return error.OutOfMemory;

    // Compute text pool size in u64 to avoid u32 wrap-before-check (C6).
    var text_pool_size: u64 = 0;
    for (engine.headings) |h| {
        text_pool_size += h.text.len;
        text_pool_size += h.slug.len;
    }
    for (engine.links) |l| {
        text_pool_size += l.text.len;
        text_pool_size += l.target.len;
    }
    for (engine.tags) |t| {
        text_pool_size += t.name.len;
    }
    for (engine.code_spans) |cs| {
        text_pool_size += cs.text.len;
    }
    for (engine.block_ids) |b| {
        text_pool_size += b.id.len;
    }
    for (engine.tasks) |t| {
        text_pool_size += t.text.len;
    }
    for (engine.embeds) |e| {
        text_pool_size += e.target.len;
    }
    for (engine.callouts) |c| {
        text_pool_size += c.callout_type.len;
        if (c.title) |t| text_pool_size += t.len;
    }
    for (engine.block_refs) |br| {
        text_pool_size += br.uuid.len;
    }
    if (text_pool_size > std.math.maxInt(u32)) return error.OutOfMemory;
    const text_pool_u32: u32 = @intCast(text_pool_size);

    const total_size = blob.computeBlobSize(
        @intCast(engine.headings.len),
        @intCast(engine.links.len),
        @intCast(engine.tags.len),
        @intCast(engine.block_ids.len),
        @intCast(engine.code_spans.len),
        @intCast(engine.tasks.len),
        @intCast(engine.embeds.len),
        @intCast(engine.callouts.len),
        @intCast(engine.block_refs.len),
        @intCast(engine.line_starts.len),
        text_pool_u32,
    ) orelse return error.OutOfMemory;

    // Allocate blob
    const buf = try engine.allocator.alloc(u8, total_size);
    errdefer engine.allocator.free(buf);

    // Zero the buffer for deterministic output
    @memset(buf, 0);

    // Write header
    const header = blob.ScanBlobHeader{
        .content_hash = engine.content_hash,
        .heading_count = @intCast(engine.headings.len),
        .link_count = @intCast(engine.links.len),
        .tag_count = @intCast(engine.tags.len),
        .block_id_count = @intCast(engine.block_ids.len),
        .code_span_count = @intCast(engine.code_spans.len),
        .task_count = @intCast(engine.tasks.len),
        .embed_count = @intCast(engine.embeds.len),
        .callout_count = @intCast(engine.callouts.len),
        .block_ref_count = @intCast(engine.block_refs.len),
        .line_count = @intCast(engine.line_starts.len),
        .text_pool_size = text_pool_u32,
        .token_estimate = engine.token_estimate,
        .total_blob_size = total_size,
    };
    blob.writeHeader(buf, header);

    const offsets = blob.computeSectionOffsets(header) orelse return error.OutOfMemory;

    // Write headings and build text pool
    var pool_off: u32 = 0;
    for (engine.headings, 0..) |h, i| {
        const bh = blob.BlobHeading{
            .text_off = pool_off,
            .text_len = @intCast(h.text.len),
            .slug_off = pool_off + @as(u32, @intCast(h.text.len)),
            .slug_len = @intCast(h.slug.len),
            .source_offset = h.source_offset,
            .start_line = h.start.line,
            .start_col = h.start.col,
            .end_line = h.end.line,
            .end_col = h.end.col,
            .level = h.level,
        };
        try blob.writeStruct(blob.BlobHeading, buf, offsets.headings + i * @sizeOf(blob.BlobHeading), bh);

        // Write text to text pool
        @memcpy(buf[offsets.text_pool + pool_off ..][0..h.text.len], h.text);
        pool_off += @intCast(h.text.len);
        @memcpy(buf[offsets.text_pool + pool_off ..][0..h.slug.len], h.slug);
        pool_off += @intCast(h.slug.len);
    }

    // Write links
    for (engine.links, 0..) |l, i| {
        const bl = blob.BlobLink{
            .text_off = pool_off,
            .text_len = @intCast(l.text.len),
            .target_off = pool_off + @as(u32, @intCast(l.text.len)),
            .target_len = @intCast(l.target.len),
            .source_offset = l.source_offset,
            .start_line = l.start.line,
            .start_col = l.start.col,
            .end_line = l.end.line,
            .end_col = l.end.col,
            .is_wiki = if (l.is_wiki) 1 else 0,
        };
        try blob.writeStruct(blob.BlobLink, buf, offsets.links + i * @sizeOf(blob.BlobLink), bl);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..l.text.len], l.text);
        pool_off += @intCast(l.text.len);
        @memcpy(buf[offsets.text_pool + pool_off ..][0..l.target.len], l.target);
        pool_off += @intCast(l.target.len);
    }

    // Write tags
    for (engine.tags, 0..) |t, i| {
        const bt = blob.BlobTag{
            .name_off = pool_off,
            .name_len = @intCast(t.name.len),
            .source_offset = t.source_offset,
            .start_line = t.start.line,
            .start_col = t.start.col,
        };
        try blob.writeStruct(blob.BlobTag, buf, offsets.tags + i * @sizeOf(blob.BlobTag), bt);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..t.name.len], t.name);
        pool_off += @intCast(t.name.len);
    }

    // Write block IDs
    for (engine.block_ids, 0..) |b, i| {
        const bb = blob.BlobBlockId{
            .id_off = pool_off,
            .id_len = @intCast(b.id.len),
            .source_offset = b.source_offset,
            .start_line = b.start.line,
            .start_col = b.start.col,
            .end_line = b.end.line,
            .end_col = b.end.col,
        };
        try blob.writeStruct(blob.BlobBlockId, buf, offsets.block_ids + i * @sizeOf(blob.BlobBlockId), bb);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..b.id.len], b.id);
        pool_off += @intCast(b.id.len);
    }

    // Write code spans
    for (engine.code_spans, 0..) |cs, i| {
        const bcs = blob.BlobCodeSpan{
            .text_off = pool_off,
            .text_len = @intCast(cs.text.len),
            .source_offset = cs.source_offset,
            .end_offset = cs.end_offset,
            .start_line = cs.start.line,
            .start_col = cs.start.col,
            .end_line = cs.end.line,
            .end_col = cs.end.col,
        };
        try blob.writeStruct(blob.BlobCodeSpan, buf, offsets.code_spans + i * @sizeOf(blob.BlobCodeSpan), bcs);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..cs.text.len], cs.text);
        pool_off += @intCast(cs.text.len);
    }

    // Write tasks
    for (engine.tasks, 0..) |t, i| {
        const bt = blob.BlobTask{
            .text_off = pool_off,
            .text_len = @intCast(t.text.len),
            .source_offset = t.source_offset,
            .end_offset = t.end_offset,
            .start_line = t.start.line,
            .start_col = t.start.col,
            .end_line = t.end.line,
            .end_col = t.end.col,
            .state = t.state,
        };
        try blob.writeStruct(blob.BlobTask, buf, offsets.tasks + i * @sizeOf(blob.BlobTask), bt);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..t.text.len], t.text);
        pool_off += @intCast(t.text.len);
    }

    // Write embeds
    for (engine.embeds, 0..) |e, i| {
        const be = blob.BlobEmbed{
            .target_off = pool_off,
            .target_len = @intCast(e.target.len),
            .source_offset = e.source_offset,
            .end_offset = e.end_offset,
            .start_line = e.start.line,
            .start_col = e.start.col,
            .end_line = e.end.line,
            .end_col = e.end.col,
        };
        try blob.writeStruct(blob.BlobEmbed, buf, offsets.embeds + i * @sizeOf(blob.BlobEmbed), be);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..e.target.len], e.target);
        pool_off += @intCast(e.target.len);
    }

    // Write callouts
    for (engine.callouts, 0..) |c, i| {
        const title_off: u32 = if (c.title != null) pool_off + @as(u32, @intCast(c.callout_type.len)) else 0;
        const title_len: u32 = if (c.title) |t| @intCast(t.len) else 0;
        const bc = blob.BlobCallout{
            .type_off = pool_off,
            .type_len = @intCast(c.callout_type.len),
            .title_off = title_off,
            .title_len = title_len,
            .source_offset = c.source_offset,
            .end_offset = c.end_offset,
            .start_line = c.start.line,
            .start_col = c.start.col,
            .end_line = c.end.line,
            .end_col = c.end.col,
        };
        try blob.writeStruct(blob.BlobCallout, buf, offsets.callouts + i * @sizeOf(blob.BlobCallout), bc);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..c.callout_type.len], c.callout_type);
        pool_off += @intCast(c.callout_type.len);
        if (c.title) |t| {
            @memcpy(buf[offsets.text_pool + pool_off ..][0..t.len], t);
            pool_off += @intCast(t.len);
        }
    }

    // Write block refs
    for (engine.block_refs, 0..) |br, i| {
        const bbr = blob.BlobBlockRef{
            .uuid_off = pool_off,
            .uuid_len = @intCast(br.uuid.len),
            .source_offset = br.source_offset,
            .start_line = br.start.line,
            .start_col = br.start.col,
            .end_line = br.end.line,
            .end_col = br.end.col,
        };
        try blob.writeStruct(blob.BlobBlockRef, buf, offsets.block_refs + i * @sizeOf(blob.BlobBlockRef), bbr);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..br.uuid.len], br.uuid);
        pool_off += @intCast(br.uuid.len);
    }

    // Write line_starts
    for (engine.line_starts, 0..) |ls, i| {
        const offset = offsets.line_starts + @as(u32, @intCast(i)) * @sizeOf(u32);
        std.mem.writeInt(u32, buf[offset..][0..4], ls, .little);
    }

    return buf;
}
