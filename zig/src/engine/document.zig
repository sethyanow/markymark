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

fn parseAll(
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
fn slugifyText(text: []const u8, out: *[512]u8) []const u8 {
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

fn freeHeadings(allocator: Allocator, headings: []StoredHeading) void {
    for (headings) |h| {
        allocator.free(h.text);
        allocator.free(h.slug);
    }
    if (headings.len > 0) allocator.free(headings);
}

fn freeLinks(allocator: Allocator, links: []StoredLink) void {
    for (links) |l| {
        allocator.free(l.text);
        allocator.free(l.target);
    }
    if (links.len > 0) allocator.free(links);
}

fn freeTags(allocator: Allocator, tags: []StoredTag) void {
    for (tags) |t| {
        allocator.free(t.name);
    }
    if (tags.len > 0) allocator.free(tags);
}

fn freeBlockIds(allocator: Allocator, block_ids: []StoredBlockId) void {
    for (block_ids) |b| {
        allocator.free(b.id);
    }
    if (block_ids.len > 0) allocator.free(block_ids);
}

fn freeStoredHeadingsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredHeading), free_texts: bool) void {
    for (list.items) |h| {
        // h.text was transferred from extraction; only free it when texts_transferred=true
        // (i.e., after extraction.headings/links slice containers were freed at line 289-290).
        if (free_texts) allocator.free(h.text);
        allocator.free(h.slug);
    }
    list.deinit(allocator);
}

fn freeStoredLinksList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredLink), free_texts: bool) void {
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

const testing = std.testing;

// Import blob tests
test {
    _ = @import("blob.zig");
}

// --- Extraction correctness ---

test "test_create_simple_markdown" {
    const input = "# Hello\n\nSome [link](url.md) text with #tag and ^blockid\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    try testing.expectEqual(@as(usize, 1), engine.links.len);
    try testing.expectEqual(@as(usize, 1), engine.tags.len);
    try testing.expectEqual(@as(usize, 1), engine.block_ids.len);

    try testing.expectEqualStrings("Hello", engine.headings[0].text);
    try testing.expectEqual(@as(u8, 1), engine.headings[0].level);
    try testing.expectEqualStrings("hello", engine.headings[0].slug);
}

test "test_create_multiple_headings" {
    const input = "# H1\n\n## H2\n\n### H3\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 3), engine.headings.len);
    try testing.expectEqual(@as(u8, 1), engine.headings[0].level);
    try testing.expectEqual(@as(u8, 2), engine.headings[1].level);
    try testing.expectEqual(@as(u8, 3), engine.headings[2].level);
}

test "test_entity_decoding" {
    const input = "# Hello &amp; World\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    try testing.expectEqualStrings("Hello & World", engine.headings[0].text);
}

test "test_wiki_links" {
    const input = "See [[Other Page]] and [normal](link.md)\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 2), engine.links.len);
    // Order depends on md4c parse order (wiki link and normal link)
    var wiki_count: usize = 0;
    var normal_count: usize = 0;
    for (engine.links) |l| {
        if (l.is_wiki) wiki_count += 1 else normal_count += 1;
    }
    try testing.expectEqual(@as(usize, 1), wiki_count);
    try testing.expectEqual(@as(usize, 1), normal_count);
}

// --- Slug dedup ---

test "test_slug_dedup" {
    const input = "# Title\n\n# Title\n\n# Title\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 3), engine.headings.len);
    try testing.expectEqualStrings("title", engine.headings[0].slug);
    try testing.expectEqualStrings("title-1", engine.headings[1].slug);
    try testing.expectEqualStrings("title-2", engine.headings[2].slug);
}

// --- Line starts and positions ---

test "test_line_starts" {
    const input = "first\nsecond\nthird\n";
    const starts = try computeLineStarts(testing.allocator, input);
    defer testing.allocator.free(starts);

    // "first\n" = 6 bytes, "second\n" = 7 bytes, "third\n" = 6 bytes
    try testing.expectEqual(@as(usize, 4), starts.len);
    try testing.expectEqual(@as(u32, 0), starts[0]);
    try testing.expectEqual(@as(u32, 6), starts[1]);
    try testing.expectEqual(@as(u32, 13), starts[2]);
    try testing.expectEqual(@as(u32, 19), starts[3]);
}

test "test_byte_offset_to_position" {
    const starts = &[_]u32{ 0, 6, 13, 19 };
    // Offset 0 → line 0, col 0
    const p0 = byteOffsetToPosition(starts, 0);
    try testing.expectEqual(@as(u32, 0), p0.line);
    try testing.expectEqual(@as(u32, 0), p0.col);

    // Offset 6 → line 1, col 0
    const p1 = byteOffsetToPosition(starts, 6);
    try testing.expectEqual(@as(u32, 1), p1.line);
    try testing.expectEqual(@as(u32, 0), p1.col);

    // Offset 8 → line 1, col 2
    const p2 = byteOffsetToPosition(starts, 8);
    try testing.expectEqual(@as(u32, 1), p2.line);
    try testing.expectEqual(@as(u32, 2), p2.col);
}

// --- Blob serialization ---

test "test_blob_header" {
    const input = "# Hello\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = blob.readHeader(blob_data);

    try testing.expectEqual(blob.BLOB_MAGIC, header.magic);
    try testing.expectEqual(blob.BLOB_VERSION, header.version);
    try testing.expectEqual(@as(u32, 1), header.heading_count);
    try testing.expectEqual(@as(u32, 0), header.link_count);
}

test "test_blob_text_pool" {
    const input = "# Hello\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = blob.readHeader(blob_data);
    const offsets = blob.computeSectionOffsets(header).?;

    // Read the heading from the blob
    const bh = blob.readStruct(blob.BlobHeading, blob_data, offsets.headings);
    try testing.expectEqual(@as(u8, 1), bh.level);

    // Verify text pool contains "Hello"
    const text_start = offsets.text_pool + bh.text_off;
    const text_end = text_start + bh.text_len;
    try testing.expectEqualStrings("Hello", blob_data[text_start..text_end]);

    // Verify text pool contains slug "hello"
    const slug_start = offsets.text_pool + bh.slug_off;
    const slug_end = slug_start + bh.slug_len;
    try testing.expectEqualStrings("hello", blob_data[slug_start..slug_end]);
}

test "test_blob_empty_document" {
    const input = "";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    // Empty document blob is header only (64 bytes)
    try testing.expectEqual(@as(usize, 64), blob_data.len);

    const header = blob.readHeader(blob_data);
    try testing.expectEqual(@as(u32, 0), header.heading_count);
    try testing.expectEqual(@as(u32, 0), header.link_count);
    try testing.expectEqual(@as(u32, 0), header.tag_count);
    try testing.expectEqual(@as(u32, 0), header.block_id_count);
}

test "test_blob_validate_rejects_bad_magic" {
    var buf: [64]u8 = .{0} ** 64;
    std.mem.writeInt(u32, buf[0..4], 0xDEADBEEF, .little);
    try testing.expectError(error.InvalidMagic, blob.validateBlob(&buf));
}

test "test_blob_validates_after_serialize" {
    const input = "# Title\n\n[link](url.md) #tag ^block\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = try blob.validateBlob(blob_data);
    try testing.expectEqual(blob.BLOB_MAGIC, header.magic);
    try testing.expectEqual(blob.BLOB_VERSION, header.version);
}

// --- Update ---

test "test_update_replaces_state" {
    var engine = try DocumentEngine.create("# A\n", testing.allocator);
    defer engine.destroy();

    try testing.expectEqualStrings("A", engine.headings[0].text);
    try engine.update("# B\n");
    try testing.expectEqualStrings("B", engine.headings[0].text);
}

test "test_update_invalidates_blob" {
    var engine = try DocumentEngine.create("# A\n", testing.allocator);
    defer engine.destroy();

    const blob1 = try engine.getBlob();
    const blob1_len = blob1.len;

    try engine.update("# B\n## C\n");
    // Blob should be invalidated
    try testing.expectEqual(@as(?[]u8, null), engine.cached_blob);

    const blob2 = try engine.getBlob();
    // New blob should be different (more headings = larger)
    try testing.expect(blob2.len != blob1_len or blob2.ptr != blob1.ptr);
}

test "test_update_changes_counts" {
    var engine = try DocumentEngine.create("# One\n", testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);

    try engine.update("# One\n## Two\n### Three\n");
    try testing.expectEqual(@as(usize, 3), engine.headings.len);
}

// --- Memory safety ---

test "test_create_destroy_no_leaks" {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer {
        const check = gpa.deinit();
        if (check == .leak) @panic("Memory leak detected in create/destroy");
    }
    const allocator = gpa.allocator();

    var engine = try DocumentEngine.create("# Hello\n\n[link](url) #tag ^id\n", allocator);
    engine.destroy();
}

test "test_update_100_times_no_leaks" {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer {
        const check = gpa.deinit();
        if (check == .leak) @panic("Memory leak detected in update×100");
    }
    const allocator = gpa.allocator();

    var engine = try DocumentEngine.create("# Initial\n", allocator);
    defer engine.destroy();

    var i: u32 = 0;
    while (i < 100) : (i += 1) {
        var buf: [128]u8 = undefined;
        const text = std.fmt.bufPrint(&buf, "# Heading {d}\n\nSome [link](url{d}.md) text #tag{d}\n", .{ i, i, i }) catch continue;
        try engine.update(text);
    }
}

// --- Additional edge cases ---

test "empty heading produces empty slug" {
    const input = "# \n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    // Empty heading text → empty slug
    try testing.expectEqualStrings("", engine.headings[0].slug);
}

test "line_starts for empty input" {
    const starts = try computeLineStarts(testing.allocator, "");
    // Empty text has no line starts
    try testing.expectEqual(@as(usize, 0), starts.len);
}

test "token_estimate is nonzero for nonempty input" {
    var engine = try DocumentEngine.create("hello world foo bar\n", testing.allocator);
    defer engine.destroy();

    try testing.expect(engine.token_estimate > 0);
}

test "content_hash is deterministic" {
    var engine1 = try DocumentEngine.create("# Same\n", testing.allocator);
    defer engine1.destroy();

    var engine2 = try DocumentEngine.create("# Same\n", testing.allocator);
    defer engine2.destroy();

    try testing.expectEqual(engine1.content_hash, engine2.content_hash);
}

test "content_hash differs for different input" {
    var engine1 = try DocumentEngine.create("# A\n", testing.allocator);
    defer engine1.destroy();

    var engine2 = try DocumentEngine.create("# B\n", testing.allocator);
    defer engine2.destroy();

    try testing.expect(engine1.content_hash != engine2.content_hash);
}

test "tags inside code blocks are filtered" {
    const input = "text #visible\n```\n#hidden\n```\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.tags.len);
    try testing.expectEqualStrings("visible", engine.tags[0].name);
}

test "getBlob caches result" {
    var engine = try DocumentEngine.create("# Test\n", testing.allocator);
    defer engine.destroy();

    const blob1 = try engine.getBlob();
    const blob2 = try engine.getBlob();
    // Same pointer — cached
    try testing.expectEqual(blob1.ptr, blob2.ptr);
}

test "blob line_starts roundtrip" {
    const input = "# Line1\nLine2\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = blob.readHeader(blob_data);
    const offsets = blob.computeSectionOffsets(header).?;

    // Read line_starts from blob
    for (0..header.line_count) |i| {
        const offset = offsets.line_starts + @as(u32, @intCast(i)) * @sizeOf(u32);
        const ls = std.mem.readInt(u32, blob_data[offset..][0..4], .little);
        try testing.expectEqual(engine.line_starts[i], ls);
    }
}

test "slugifyText truncated slug returns content not empty string" {
    // When heading text produces >512 slug bytes, slugify() returns -2 (truncated).
    // The output buffer holds 512 valid bytes. Fix: return out[0..512], not "".
    // This test verifies the fix: a 513-char heading gets a 512-byte slug, not "".
    var out: [512]u8 = undefined;
    const long_text = "a" ** 513; // 513 'a' chars → slugify returns -2, buffer has 512 'a' chars
    const slug = slugifyText(long_text, &out);
    try testing.expectEqual(@as(usize, 512), slug.len);
    try testing.expectEqualStrings("a" ** 512, slug);
}

test "freeStoredHeadingsList with free_texts=true frees text strings" {
    // Verify that freeStoredHeadingsList with free_texts=true frees both text and slug.
    // This simulates the errdefer cleanup path after texts_transferred (Bug 1 fix):
    // before the fix, errdefer only freed slugs, leaking h.text owned by stored lists.
    const alloc = testing.allocator;
    var list = std.ArrayListUnmanaged(StoredHeading){};
    const text = try alloc.dupe(u8, "Hello World");
    const slug = try alloc.dupe(u8, "hello-world");
    try list.append(alloc, .{
        .text = text,
        .slug = slug,
        .source_offset = 0,
        .start = .{ .line = 0, .col = 0 },
        .end = .{ .line = 0, .col = 10 },
        .level = 1,
    });
    freeStoredHeadingsList(alloc, &list, true);
    // testing.allocator (GPA) detects leaks: if text or slug aren't freed, test fails
}

test "freeStoredLinksList with free_texts=true frees text and target strings" {
    // Verify that freeStoredLinksList with free_texts=true frees text and target.
    // Before the fix, freeStoredLinksList freed nothing (link texts were always owned
    // by extraction until line 289-290, but errdefer fires after that with no way to
    // distinguish which allocations to free).
    const alloc = testing.allocator;
    var list = std.ArrayListUnmanaged(StoredLink){};
    const text = try alloc.dupe(u8, "Click here");
    const target = try alloc.dupe(u8, "https://example.com");
    try list.append(alloc, .{
        .text = text,
        .target = target,
        .source_offset = 0,
        .start = .{ .line = 0, .col = 0 },
        .end = .{ .line = 0, .col = 10 },
        .is_wiki = false,
    });
    freeStoredLinksList(alloc, &list, true);
    // testing.allocator (GPA) detects leaks: if text or target aren't freed, test fails
}

test "slugifyText truncated heading via DocumentEngine is non-empty" {
    // Integration: DocumentEngine.create with a >512-char heading text should
    // produce a non-empty slug (not "" from silently discarding truncated output).
    const prefix = "# ";
    const heading_text = "b" ** 513;
    const input = prefix ++ heading_text ++ "\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    // Slug should be 512 'b' chars, not empty
    try testing.expectEqual(@as(usize, 512), engine.headings[0].slug.len);
    try testing.expectEqualStrings("b" ** 512, engine.headings[0].slug);
}

// --- marky-8nzt: toOwnedSlice cascade leak regression ---

test "marky-8nzt: parseAll toOwnedSlice cascade OOM — no leak" {
    // Exercises every OOM failure point in parseAll by iterating fail_index
    // from 0..N. At each index, exactly one allocation fails.
    // GPA detects leaks (.leak status) — verifies that scoped errdefers after
    // each toOwnedSlice call correctly free transferred data when a later
    // toOwnedSlice fails.
    //
    // Input has headings, links, tags, and block IDs so all four toOwnedSlice
    // paths (lines 359-362) are exercised. The critical path: headings
    // toOwnedSlice succeeds → links toOwnedSlice fails → errdefer must free
    // out_headings (top-level errdefer runs on empty stored_headings_list,
    // which is a no-op after toOwnedSlice consumed it).
    const input = "# Heading One\n\n[Link Text](https://example.com)\n\n#tag1\n\nA paragraph ^block-one\n";

    var fail_index: usize = 0;
    var consecutive_successes: usize = 0;
    while (consecutive_successes < 5) : (fail_index += 1) {
        // Safety valve: prevent infinite loop if something is very wrong
        if (fail_index > 300) break;

        var gpa = std.heap.GeneralPurposeAllocator(.{}){};
        var failing = std.testing.FailingAllocator.init(gpa.allocator(), .{ .fail_index = fail_index });

        var out_headings: []StoredHeading = &.{};
        var out_links: []StoredLink = &.{};
        var out_tags: []StoredTag = &.{};
        var out_block_ids: []StoredBlockId = &.{};
        var out_line_starts: []u32 = &.{};
        var out_token_estimate: u32 = 0;
        var out_content_hash: u64 = 0;

        const result = parseAll(
            failing.allocator(),
            input,
            &out_headings,
            &out_links,
            &out_tags,
            &out_block_ids,
            &out_line_starts,
            &out_token_estimate,
            &out_content_hash,
        );

        if (result) |_| {
            // Success: free output slices manually (simulates caller cleanup)
            freeHeadings(failing.allocator(), out_headings);
            freeLinks(failing.allocator(), out_links);
            freeTags(failing.allocator(), out_tags);
            freeBlockIds(failing.allocator(), out_block_ids);
            if (out_line_starts.len > 0) failing.allocator().free(out_line_starts);
            consecutive_successes += 1;
        } else |_| {
            consecutive_successes = 0;
        }

        const check = gpa.deinit();
        try testing.expect(check == .ok);
    }

    // Verify we actually tested multiple failure points (not just index 0)
    try testing.expect(fail_index > 5);
}
