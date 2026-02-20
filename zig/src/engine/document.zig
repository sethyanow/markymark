// DocumentEngine: stateful document indexer that owns persistent parse state.
//
// Combines md4c extraction (headings, links), SIMD scans (tags, block_ids),
// and derived data (slugs, line_starts, positions) into a single engine.
// Serializes state to a flat binary blob for zero-copy FFI transfer.

const std = @import("std");
const Allocator = std.mem.Allocator;

const blob = @import("blob.zig");
const extraction_renderer = @import("../md4c/extraction_renderer.zig");
const root = @import("../md4c/root.zig");
const slug_kernel = @import("../kernels/slug.zig");
const tag_scan_mod = @import("../kernels/tag_scan.zig");
const block_scan_mod = @import("../kernels/block_scan.zig");
const fence_map_mod = @import("../kernels/fence_map.zig");
const token_estimate_mod = @import("../kernels/token_estimate.zig");
const content_hash_mod = @import("../kernels/content_hash.zig");
const helpers = @import("../md4c/helpers.zig");

// ── Stored types (engine-internal) ──────────────────────────────────

pub const Position = struct {
    line: u32,
    col: u32,
};

pub const StoredHeading = struct {
    text: []const u8, // owned
    slug: []const u8, // owned
    source_offset: u32,
    start: Position,
    end: Position,
    level: u8,
};

pub const StoredLink = struct {
    text: []const u8, // owned
    target: []const u8, // owned
    source_offset: u32,
    start: Position,
    end: Position,
    is_wiki: bool,
};

pub const StoredTag = struct {
    name: []const u8, // owned
    source_offset: u32,
    start: Position,
};

pub const StoredBlockId = struct {
    id: []const u8, // owned
    source_offset: u32,
    start: Position,
    end: Position,
};

// ── DocumentEngine ──────────────────────────────────────────────────

pub const DocumentEngine = struct {
    allocator: Allocator,

    headings: []StoredHeading = &.{},
    links: []StoredLink = &.{},
    tags: []StoredTag = &.{},
    block_ids: []StoredBlockId = &.{},
    line_starts: []u32 = &.{},

    token_estimate: u32 = 0,
    content_hash: u64 = 0,

    cached_blob: ?[]u8 = null,

    pub const Error = error{
        OutOfMemory,
        ParseFailed,
    };

    /// Create a new engine from markdown text.
    pub fn create(text: []const u8, allocator: Allocator) Error!*DocumentEngine {
        const self = allocator.create(DocumentEngine) catch return error.OutOfMemory;
        self.* = .{ .allocator = allocator };
        self.parseAndStore(text) catch |e| {
            allocator.destroy(self);
            return e;
        };
        return self;
    }

    /// Update engine state with new markdown text.
    /// On success, old state is freed. On failure, old state is preserved.
    pub fn update(self: *DocumentEngine, text: []const u8) Error!void {
        // Parse new text FIRST, before freeing old state.
        // This ensures old state is preserved on parse failure.
        var new_headings: []StoredHeading = &.{};
        var new_links: []StoredLink = &.{};
        var new_tags: []StoredTag = &.{};
        var new_block_ids: []StoredBlockId = &.{};
        var new_line_starts: []u32 = &.{};
        var new_token_estimate: u32 = 0;
        var new_content_hash: u64 = 0;

        parseAll(
            self.allocator,
            text,
            &new_headings,
            &new_links,
            &new_tags,
            &new_block_ids,
            &new_line_starts,
            &new_token_estimate,
            &new_content_hash,
        ) catch |e| return e;

        // Parse succeeded — free old state, install new state
        self.freeState();
        self.headings = new_headings;
        self.links = new_links;
        self.tags = new_tags;
        self.block_ids = new_block_ids;
        self.line_starts = new_line_starts;
        self.token_estimate = new_token_estimate;
        self.content_hash = new_content_hash;
        self.cached_blob = null; // Invalidate cached blob
    }

    /// Get the serialized blob. Lazy: built on first call, cached until update.
    pub fn getBlob(self: *DocumentEngine) Error![]const u8 {
        if (self.cached_blob) |b| return b;

        const b = serializeState(self) catch return error.OutOfMemory;
        self.cached_blob = b;
        return b;
    }

    /// Destroy the engine, freeing all owned memory.
    pub fn destroy(self: *DocumentEngine) void {
        self.freeState();
        self.allocator.destroy(self);
    }

    // ── Private helpers ─────────────────────────────────────────────

    fn parseAndStore(self: *DocumentEngine, text: []const u8) Error!void {
        parseAll(
            self.allocator,
            text,
            &self.headings,
            &self.links,
            &self.tags,
            &self.block_ids,
            &self.line_starts,
            &self.token_estimate,
            &self.content_hash,
        ) catch |e| return e;
    }

    fn freeState(self: *DocumentEngine) void {
        freeHeadings(self.allocator, self.headings);
        self.headings = &.{};
        freeLinks(self.allocator, self.links);
        self.links = &.{};
        freeTags(self.allocator, self.tags);
        self.tags = &.{};
        freeBlockIds(self.allocator, self.block_ids);
        self.block_ids = &.{};
        if (self.line_starts.len > 0) {
            self.allocator.free(self.line_starts);
            self.line_starts = &.{};
        }
        if (self.cached_blob) |b| {
            self.allocator.free(b);
            self.cached_blob = null;
        }
    }
};

// ── Core parse/extract pipeline ─────────────────────────────────────

pub fn parseAll(
    allocator: Allocator,
    text: []const u8,
    out_headings: *[]StoredHeading,
    out_links: *[]StoredLink,
    out_tags: *[]StoredTag,
    out_block_ids: *[]StoredBlockId,
    out_line_starts: *[]u32,
    out_token_estimate: *u32,
    out_content_hash: *u64,
) DocumentEngine.Error!void {
    // 1. md4c extraction (headings + links)
    var extraction = extraction_renderer.extractFromMarkdown(text, allocator) catch |e| return switch (e) {
        error.OutOfMemory => error.OutOfMemory,
        error.StackOverflow, error.InputTooLarge => error.ParseFailed,
    };

    // We'll build stored versions and then free the extraction result.
    // On error, free both extraction and any partial stored results.
    var stored_headings_list = std.ArrayListUnmanaged(StoredHeading){};
    var stored_links_list = std.ArrayListUnmanaged(StoredLink){};
    var stored_tags_list = std.ArrayListUnmanaged(StoredTag){};
    var stored_block_ids_list = std.ArrayListUnmanaged(StoredBlockId){};

    // Tracks whether h.text/l.text/l.target have been transferred from extraction into
    // the stored lists (i.e., after extraction.headings/links slice containers are freed
    // at line 289-290). Before transfer, extraction.deinit() in each catch block frees
    // the strings. After transfer, the errdefer must free them via the stored lists.
    var texts_transferred: bool = false;

    // On error: free everything we've built. Pass texts_transferred so the free helpers
    // know whether they own the string data (post-transfer) or extraction owns it (pre-transfer).
    errdefer {
        freeStoredHeadingsList(allocator, &stored_headings_list, texts_transferred);
        freeStoredLinksList(allocator, &stored_links_list, texts_transferred);
        freeStoredTagsList(allocator, &stored_tags_list);
        freeStoredBlockIdsList(allocator, &stored_block_ids_list);
    }

    // 2. Compute line_starts
    const line_starts = computeLineStarts(allocator, text) catch {
        extraction.deinit();
        return error.OutOfMemory;
    };
    errdefer if (line_starts.len > 0) allocator.free(line_starts);

    // 3. Build fence map for tag/block filtering
    var fence_buf: [256]fence_map_mod.FenceRange = undefined;
    var fence_count: u32 = 0;
    if (text.len > 0) {
        fence_count = fence_map_mod.build_fence_map(text.ptr, @intCast(text.len), &fence_buf, 256);
        if (fence_count > 256) fence_count = 256;
    }
    const fence_ranges = fence_buf[0..fence_count];

    // 4. Process headings: slugify + dedup + positions
    for (extraction.headings, 0..) |h, i| {
        const slug = makeSlug(allocator, h.text, extraction.headings[0..i]) catch {
            extraction.deinit();
            return error.OutOfMemory;
        };
        errdefer allocator.free(slug);

        const start_pos = byteOffsetToPosition(line_starts, h.offset);
        // End position: past "## text" = offset + level + space + text_len
        const end_offset = h.offset +| @as(u32, h.level) +| 1 +| @as(u32, @intCast(h.text.len));
        const end_pos = byteOffsetToPosition(line_starts, end_offset);

        // Transfer text ownership from extraction to stored heading
        stored_headings_list.append(allocator, .{
            .text = h.text,
            .slug = slug,
            .source_offset = h.offset,
            .start = start_pos,
            .end = end_pos,
            .level = h.level,
        }) catch {
            // slug freed by errdefer allocator.free(slug) above
            extraction.deinit();
            return error.OutOfMemory;
        };
    }

    // 5. Process links: positions
    for (extraction.links) |l| {
        const start_pos = byteOffsetToPosition(line_starts, l.offset);
        // Use the accurate end_offset from the extraction renderer's scan cursor,
        // which was advanced to the position past the link's closing character
        // (past ']]' for wiki, past '>' for autolinks, past ')' or ']' for others).
        const end_pos = byteOffsetToPosition(line_starts, l.end_offset);
        stored_links_list.append(allocator, .{
            .text = l.text,
            .target = l.target,
            .source_offset = l.offset,
            .start = start_pos,
            .end = end_pos,
            .is_wiki = l.is_wiki,
        }) catch {
            extraction.deinit();
            return error.OutOfMemory;
        };
    }

    // OWNERSHIP: The string data (h.text, l.text, l.target) from extraction_renderer's
    // ExtractedHeading/ExtractedLink arrays has been moved into stored_headings_list and
    // stored_links_list by the loops above (steps 4-5). Only the slice containers
    // (extraction.headings, extraction.links) are freed here — NOT the string contents.
    // Do not add allocator.free(h.text) or similar; the strings are now owned by stored lists.
    allocator.free(extraction.headings);
    allocator.free(extraction.links);
    // From this point, the errdefer must free string data from the stored lists directly.
    texts_transferred = true;

    // 6. Scan tags (with fence filtering)
    if (text.len > 0) {
        var tag_buf: [1024]tag_scan_mod.TagScan = undefined;
        const raw_tag_count = tag_scan_mod.scan_tags(text.ptr, @intCast(text.len), &tag_buf, 1024);
        const tag_count = @min(raw_tag_count, 1024);

        for (tag_buf[0..tag_count]) |t| {
            if (inFenceRange(fence_ranges, t.offset)) continue;
            const name_start = t.offset + 1; // skip '#'
            const name_end = name_start + t.length;
            if (name_end > text.len) continue;
            const name = allocator.dupe(u8, text[name_start..name_end]) catch
                return error.OutOfMemory;
            errdefer allocator.free(name);
            const pos = byteOffsetToPosition(line_starts, t.offset);
            stored_tags_list.append(allocator, .{
                .name = name,
                .source_offset = t.offset,
                .start = pos,
            }) catch {
                // name freed by errdefer allocator.free(name) above
                return error.OutOfMemory;
            };
        }
    }

    // 7. Scan block IDs (with fence filtering)
    if (text.len > 0) {
        var block_buf: [1024]block_scan_mod.BlockIdScan = undefined;
        const raw_block_count = block_scan_mod.scan_block_ids(text.ptr, @intCast(text.len), &block_buf, 1024);
        const block_count = @min(raw_block_count, 1024);

        for (block_buf[0..block_count]) |b| {
            if (inFenceRange(fence_ranges, b.offset)) continue;
            const id_start = b.offset + 1; // skip '^'
            const id_end = id_start + b.length;
            if (id_end > text.len) continue;
            const id = allocator.dupe(u8, text[id_start..id_end]) catch
                return error.OutOfMemory;
            errdefer allocator.free(id);
            const pos = byteOffsetToPosition(line_starts, b.offset);
            // End position: past "^block-id" = offset + 1 (^) + id_length
            const block_end_offset = b.offset +| 1 +| @as(u32, b.length);
            const block_end_pos = byteOffsetToPosition(line_starts, block_end_offset);
            stored_block_ids_list.append(allocator, .{
                .id = id,
                .source_offset = b.offset,
                .start = pos,
                .end = block_end_pos,
            }) catch {
                // id freed by errdefer allocator.free(id) above
                return error.OutOfMemory;
            };
        }
    }

    // 8. Token estimate and content hash
    var token_est: u32 = 0;
    var c_hash: u64 = 0;
    if (text.len > 0) {
        token_est = token_estimate_mod.estimate_tokens(text.ptr, @intCast(text.len));
        c_hash = content_hash_mod.content_hash(text.ptr, @intCast(text.len));
    }

    // 9. Transfer ownership to output parameters.
    // Scoped errdefers protect each transferred slice: after toOwnedSlice
    // empties the stored list, the top-level errdefer (lines 213-218) runs on
    // an empty list (no-op). The scoped errdefer handles the actual cleanup if
    // a later toOwnedSlice fails.
    out_headings.* = stored_headings_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeHeadings(allocator, out_headings.*);
    out_links.* = stored_links_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeLinks(allocator, out_links.*);
    out_tags.* = stored_tags_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeTags(allocator, out_tags.*);
    // No errdefer for block_ids: nothing allocates after this point.
    out_block_ids.* = stored_block_ids_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    out_line_starts.* = line_starts;
    out_token_estimate.* = token_est;
    out_content_hash.* = c_hash;
}

// ── Slug helpers ────────────────────────────────────────────────────

/// Slugify heading text and deduplicate against previously processed headings.
/// Uses O(n^2) scan for simplicity (n = heading count, typically < 100).
fn makeSlug(
    allocator: Allocator,
    heading_text: []const u8,
    previous_headings: []const extraction_renderer.ExtractedHeading,
) ![]const u8 {
    var slug_buf: [512]u8 = undefined;
    const base_slug = slugifyText(heading_text, &slug_buf);

    // Count how many previous headings have the same base slug
    var dup_count: u32 = 0;
    for (previous_headings) |prev| {
        var prev_buf: [512]u8 = undefined;
        const prev_slug = slugifyText(prev.text, &prev_buf);
        if (std.mem.eql(u8, base_slug, prev_slug)) {
            dup_count += 1;
        }
    }

    if (dup_count == 0) {
        return try allocator.dupe(u8, base_slug);
    } else {
        return try std.fmt.allocPrint(allocator, "{s}-{d}", .{ base_slug, dup_count });
    }
}

/// Slugify text into a stack buffer. Returns the slug slice.
pub fn slugifyText(text: []const u8, out: *[512]u8) []const u8 {
    if (text.len == 0) return "";
    const rc = slug_kernel.slugify(text.ptr, @intCast(text.len), out, 512);
    if (rc >= 0) {
        return out[0..@intCast(rc)];
    }
    // Truncated (-2): buffer contains 512 valid slug bytes — return them.
    if (rc == -2) return out[0..];
    // True error (-1): return empty.
    return "";
}

// ── Line starts and position helpers ────────────────────────────────

/// Compute line start byte offsets. First entry is always 0.
/// Returns empty slice for empty input.
pub fn computeLineStarts(allocator: Allocator, text: []const u8) ![]u32 {
    if (text.len == 0) return &.{};

    var starts = std.ArrayListUnmanaged(u32){};
    errdefer starts.deinit(allocator);

    try starts.append(allocator, 0); // Line 0 starts at byte 0

    for (text, 0..) |ch, i| {
        if (ch == '\n') {
            try starts.append(allocator, @intCast(i + 1));
        }
    }

    return starts.toOwnedSlice(allocator);
}

/// Convert a byte offset to a line/column position using precomputed line_starts.
pub fn byteOffsetToPosition(line_starts: []const u32, offset: u32) Position {
    if (line_starts.len == 0) return .{ .line = 0, .col = 0 };

    // Binary search for the line containing this offset
    var lo: usize = 0;
    var hi: usize = line_starts.len;
    while (lo < hi) {
        const mid = lo + (hi - lo) / 2;
        if (line_starts[mid] <= offset) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    // lo is now the first line_start > offset, so the line is lo - 1
    const line: u32 = if (lo > 0) @intCast(lo - 1) else 0;
    const col: u32 = offset - line_starts[line];
    return .{ .line = line, .col = col };
}

// ── Fence range filtering ───────────────────────────────────────────

fn inFenceRange(ranges: []const fence_map_mod.FenceRange, pos: u32) bool {
    for (ranges) |r| {
        if (pos >= r.start and pos < r.end) return true;
    }
    return false;
}

// ── Blob serialization ──────────────────────────────────────────────

fn serializeState(engine: *const DocumentEngine) ![]u8 {
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
    for (engine.block_ids) |b| {
        text_pool_size += b.id.len;
    }
    if (text_pool_size > std.math.maxInt(u32)) return error.OutOfMemory;
    const text_pool_u32: u32 = @intCast(text_pool_size);

    const total_size = blob.computeBlobSize(
        @intCast(engine.headings.len),
        @intCast(engine.links.len),
        @intCast(engine.tags.len),
        @intCast(engine.block_ids.len),
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
        blob.writeStruct(blob.BlobHeading, buf, offsets.headings + i * @sizeOf(blob.BlobHeading), bh);

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
        blob.writeStruct(blob.BlobLink, buf, offsets.links + i * @sizeOf(blob.BlobLink), bl);

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
        blob.writeStruct(blob.BlobTag, buf, offsets.tags + i * @sizeOf(blob.BlobTag), bt);

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
        blob.writeStruct(blob.BlobBlockId, buf, offsets.block_ids + i * @sizeOf(blob.BlobBlockId), bb);

        @memcpy(buf[offsets.text_pool + pool_off ..][0..b.id.len], b.id);
        pool_off += @intCast(b.id.len);
    }

    // Write line_starts
    for (engine.line_starts, 0..) |ls, i| {
        const offset = offsets.line_starts + @as(u32, @intCast(i)) * @sizeOf(u32);
        std.mem.writeInt(u32, buf[offset..][0..4], ls, .little);
    }

    return buf;
}

// ── Free helpers ────────────────────────────────────────────────────

pub fn freeHeadings(allocator: Allocator, headings: []StoredHeading) void {
    for (headings) |h| {
        allocator.free(h.text);
        allocator.free(h.slug);
    }
    if (headings.len > 0) allocator.free(headings);
}

pub fn freeLinks(allocator: Allocator, links: []StoredLink) void {
    for (links) |l| {
        allocator.free(l.text);
        allocator.free(l.target);
    }
    if (links.len > 0) allocator.free(links);
}

pub fn freeTags(allocator: Allocator, tags: []StoredTag) void {
    for (tags) |t| {
        allocator.free(t.name);
    }
    if (tags.len > 0) allocator.free(tags);
}

pub fn freeBlockIds(allocator: Allocator, block_ids: []StoredBlockId) void {
    for (block_ids) |b| {
        allocator.free(b.id);
    }
    if (block_ids.len > 0) allocator.free(block_ids);
}

pub fn freeStoredHeadingsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredHeading), free_texts: bool) void {
    for (list.items) |h| {
        // h.text was transferred from extraction; only free it when texts_transferred=true
        // (i.e., after extraction.headings/links slice containers were freed at line 289-290).
        if (free_texts) allocator.free(h.text);
        allocator.free(h.slug);
    }
    list.deinit(allocator);
}

pub fn freeStoredLinksList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredLink), free_texts: bool) void {
    // l.text and l.target were transferred from extraction; free them only when
    // texts_transferred=true (after extraction slice containers freed at line 289-290).
    if (free_texts) {
        for (list.items) |l| {
            allocator.free(l.text);
            allocator.free(l.target);
        }
    }
    list.deinit(allocator);
}

fn freeStoredTagsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredTag)) void {
    for (list.items) |t| {
        allocator.free(t.name);
    }
    list.deinit(allocator);
}

fn freeStoredBlockIdsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredBlockId)) void {
    for (list.items) |b| {
        allocator.free(b.id);
    }
    list.deinit(allocator);
}

// ── Tests ───────────────────────────────────────────────────────────

test {
    _ = @import("document_test.zig");
}

