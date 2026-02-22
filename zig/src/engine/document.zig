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
const serialize_mod = @import("serialize.zig");

/// Maximum number of fenced code block ranges tracked on the stack.
/// Limits stack allocation to ~2 KB (256 × 8 bytes). Documents with more
/// than 256 fenced blocks will have tags/block-ids inside excess fences
/// silently included — a benign false positive on extreme inputs.
pub const FENCE_MAP_MAX: u32 = 256;

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

pub const StoredCodeSpan = struct {
    text: []const u8, // owned decoded text
    source_offset: u32, // byte offset of opening backtick
    end_offset: u32, // byte offset past closing backtick
    start: Position, // line:col of opening backtick
    end: Position, // line:col past closing backtick
};

pub const StoredBlockId = struct {
    id: []const u8, // owned
    source_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredTask = struct {
    state: u8,
    text: []const u8, // owned
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredEmbed = struct {
    target: []const u8, // owned
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredCallout = struct {
    callout_type: []const u8, // owned, lowercase alpha
    title: ?[]const u8, // owned, null if no title
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredBlockRef = struct {
    uuid: []const u8, // owned, 36-char UUID
    source_offset: u32,
    start: Position,
    end: Position,
};

// ── DocumentEngine ──────────────────────────────────────────────────

pub const DocumentEngine = struct {
    allocator: Allocator,

    headings: []StoredHeading = &.{},
    links: []StoredLink = &.{},
    code_spans: []StoredCodeSpan = &.{},
    tags: []StoredTag = &.{},
    block_ids: []StoredBlockId = &.{},
    tasks: []StoredTask = &.{},
    embeds: []StoredEmbed = &.{},
    callouts: []StoredCallout = &.{},
    block_refs: []StoredBlockRef = &.{},
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
        var new_code_spans: []StoredCodeSpan = &.{};
        var new_tags: []StoredTag = &.{};
        var new_block_ids: []StoredBlockId = &.{};
        var new_tasks: []StoredTask = &.{};
        var new_embeds: []StoredEmbed = &.{};
        var new_callouts: []StoredCallout = &.{};
        var new_block_refs: []StoredBlockRef = &.{};
        var new_line_starts: []u32 = &.{};
        var new_token_estimate: u32 = 0;
        var new_content_hash: u64 = 0;

        parseAll(
            self.allocator,
            text,
            &new_headings,
            &new_links,
            &new_code_spans,
            &new_tags,
            &new_block_ids,
            &new_tasks,
            &new_embeds,
            &new_callouts,
            &new_block_refs,
            &new_line_starts,
            &new_token_estimate,
            &new_content_hash,
        ) catch |e| return e;

        // Parse succeeded — free old state, install new state
        self.freeState();
        self.headings = new_headings;
        self.links = new_links;
        self.code_spans = new_code_spans;
        self.tags = new_tags;
        self.block_ids = new_block_ids;
        self.tasks = new_tasks;
        self.embeds = new_embeds;
        self.callouts = new_callouts;
        self.block_refs = new_block_refs;
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
            &self.code_spans,
            &self.tags,
            &self.block_ids,
            &self.tasks,
            &self.embeds,
            &self.callouts,
            &self.block_refs,
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
        freeCodeSpans(self.allocator, self.code_spans);
        self.code_spans = &.{};
        freeTags(self.allocator, self.tags);
        self.tags = &.{};
        freeBlockIds(self.allocator, self.block_ids);
        self.block_ids = &.{};
        freeTasks(self.allocator, self.tasks);
        self.tasks = &.{};
        freeEmbeds(self.allocator, self.embeds);
        self.embeds = &.{};
        freeCallouts(self.allocator, self.callouts);
        self.callouts = &.{};
        freeBlockRefs(self.allocator, self.block_refs);
        self.block_refs = &.{};
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
    out_code_spans: *[]StoredCodeSpan,
    out_tags: *[]StoredTag,
    out_block_ids: *[]StoredBlockId,
    out_tasks: *[]StoredTask,
    out_embeds: *[]StoredEmbed,
    out_callouts: *[]StoredCallout,
    out_block_refs: *[]StoredBlockRef,
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
    var stored_code_spans_list = std.ArrayListUnmanaged(StoredCodeSpan){};
    var stored_tags_list = std.ArrayListUnmanaged(StoredTag){};
    var stored_block_ids_list = std.ArrayListUnmanaged(StoredBlockId){};
    var stored_tasks_list = std.ArrayListUnmanaged(StoredTask){};
    var stored_embeds_list = std.ArrayListUnmanaged(StoredEmbed){};
    var stored_callouts_list = std.ArrayListUnmanaged(StoredCallout){};
    var stored_block_refs_list = std.ArrayListUnmanaged(StoredBlockRef){};

    // Tracks whether h.text/l.text/l.target/cs.text have been transferred from extraction
    // into the stored lists (i.e., after extraction.headings/links/code_spans slice containers
    // are freed). Before transfer, extraction.deinit() in each catch block frees the strings.
    // After transfer, the errdefer must free them via the stored lists.
    var texts_transferred: bool = false;

    // On error: free everything we've built. Pass texts_transferred so the free helpers
    // know whether they own the string data (post-transfer) or extraction owns it (pre-transfer).
    errdefer {
        freeStoredHeadingsList(allocator, &stored_headings_list, texts_transferred);
        freeStoredLinksList(allocator, &stored_links_list, texts_transferred);
        freeStoredCodeSpansList(allocator, &stored_code_spans_list, texts_transferred);
        freeStoredTagsList(allocator, &stored_tags_list);
        freeStoredBlockIdsList(allocator, &stored_block_ids_list);
        freeStoredTasksList(allocator, &stored_tasks_list, texts_transferred);
        freeStoredEmbedsList(allocator, &stored_embeds_list, texts_transferred);
        freeStoredCalloutsList(allocator, &stored_callouts_list, texts_transferred);
        freeStoredBlockRefsList(allocator, &stored_block_refs_list, texts_transferred);
    }

    // 2. Compute line_starts
    const line_starts = computeLineStarts(allocator, text) catch {
        extraction.deinit();
        return error.OutOfMemory;
    };
    errdefer if (line_starts.len > 0) allocator.free(line_starts);

    // 3. Build fence map for tag/block filtering
    var fence_buf: [FENCE_MAP_MAX]fence_map_mod.FenceRange = undefined;
    var fence_count: u32 = 0;
    if (text.len > 0) {
        fence_count = fence_map_mod.build_fence_map(text.ptr, @intCast(text.len), &fence_buf, FENCE_MAP_MAX);
        if (fence_count > FENCE_MAP_MAX) fence_count = FENCE_MAP_MAX;
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

    // 5b. Process code spans: positions
    for (extraction.code_spans) |cs| {
        const start_pos = byteOffsetToPosition(line_starts, cs.offset);
        const end_pos = byteOffsetToPosition(line_starts, cs.end_offset);
        stored_code_spans_list.append(allocator, .{
            .text = cs.text,
            .source_offset = cs.offset,
            .end_offset = cs.end_offset,
            .start = start_pos,
            .end = end_pos,
        }) catch {
            extraction.deinit();
            return error.OutOfMemory;
        };
    }

    // 5c. Process tasks: positions
    for (extraction.tasks) |t| {
        const start_pos = byteOffsetToPosition(line_starts, t.offset);
        const end_pos = byteOffsetToPosition(line_starts, t.end_offset);
        stored_tasks_list.append(allocator, .{
            .state = t.state,
            .text = t.text,
            .source_offset = t.offset,
            .end_offset = t.end_offset,
            .start = start_pos,
            .end = end_pos,
        }) catch {
            extraction.deinit();
            return error.OutOfMemory;
        };
    }

    // 5d. Process embeds: positions
    for (extraction.embeds) |e| {
        const start_pos = byteOffsetToPosition(line_starts, e.offset);
        const end_pos = byteOffsetToPosition(line_starts, e.end_offset);
        stored_embeds_list.append(allocator, .{
            .target = e.target,
            .source_offset = e.offset,
            .end_offset = e.end_offset,
            .start = start_pos,
            .end = end_pos,
        }) catch {
            extraction.deinit();
            return error.OutOfMemory;
        };
    }

    // 5e. Process callouts: positions
    for (extraction.callouts) |c| {
        const start_pos = byteOffsetToPosition(line_starts, c.offset);
        const end_pos = byteOffsetToPosition(line_starts, c.end_offset);
        stored_callouts_list.append(allocator, .{
            .callout_type = c.callout_type,
            .title = c.title,
            .source_offset = c.offset,
            .end_offset = c.end_offset,
            .start = start_pos,
            .end = end_pos,
        }) catch {
            extraction.deinit();
            return error.OutOfMemory;
        };
    }

    // 5f. Process block refs: positions
    for (extraction.block_refs) |br| {
        const start_pos = byteOffsetToPosition(line_starts, br.offset);
        // End position: past "((" + 36-char UUID + "))" = offset + 40
        const end_offset = br.offset +| 40;
        const end_pos = byteOffsetToPosition(line_starts, end_offset);
        stored_block_refs_list.append(allocator, .{
            .uuid = br.uuid,
            .source_offset = br.offset,
            .start = start_pos,
            .end = end_pos,
        }) catch {
            extraction.deinit();
            return error.OutOfMemory;
        };
    }

    // OWNERSHIP: The string data (h.text, l.text, l.target, cs.text, t.text, e.target,
    // c.callout_type, c.title, br.uuid) from extraction_renderer's arrays has been moved
    // into the stored lists by the loops above (steps 4-5f). Only the slice containers
    // are freed here — NOT the string contents. The strings are now owned by stored lists.
    allocator.free(extraction.headings);
    allocator.free(extraction.links);
    allocator.free(extraction.code_spans);
    allocator.free(extraction.tasks);
    allocator.free(extraction.embeds);
    allocator.free(extraction.callouts);
    allocator.free(extraction.block_refs);
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
    out_code_spans.* = stored_code_spans_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeCodeSpans(allocator, out_code_spans.*);
    out_tags.* = stored_tags_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeTags(allocator, out_tags.*);
    out_block_ids.* = stored_block_ids_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeBlockIds(allocator, out_block_ids.*);
    out_tasks.* = stored_tasks_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeTasks(allocator, out_tasks.*);
    out_embeds.* = stored_embeds_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeEmbeds(allocator, out_embeds.*);
    out_callouts.* = stored_callouts_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
    errdefer freeCallouts(allocator, out_callouts.*);
    // No errdefer for block_refs: nothing allocates after this point.
    out_block_refs.* = stored_block_refs_list.toOwnedSlice(allocator) catch return error.OutOfMemory;
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

const serializeState = serialize_mod.serializeState;

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

pub fn freeCodeSpans(allocator: Allocator, code_spans: []StoredCodeSpan) void {
    for (code_spans) |cs| {
        allocator.free(cs.text);
    }
    if (code_spans.len > 0) allocator.free(code_spans);
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

pub fn freeTasks(allocator: Allocator, tasks: []StoredTask) void {
    for (tasks) |t| {
        allocator.free(t.text);
    }
    if (tasks.len > 0) allocator.free(tasks);
}

pub fn freeEmbeds(allocator: Allocator, embeds: []StoredEmbed) void {
    for (embeds) |e| {
        allocator.free(e.target);
    }
    if (embeds.len > 0) allocator.free(embeds);
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

fn freeStoredCodeSpansList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredCodeSpan), free_texts: bool) void {
    // cs.text was transferred from extraction; free only when texts_transferred=true.
    if (free_texts) {
        for (list.items) |cs| {
            allocator.free(cs.text);
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

fn freeStoredTasksList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredTask), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |t| {
            allocator.free(t.text);
        }
    }
    list.deinit(allocator);
}

fn freeStoredEmbedsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredEmbed), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |e| {
            allocator.free(e.target);
        }
    }
    list.deinit(allocator);
}

pub fn freeCallouts(allocator: Allocator, callouts: []StoredCallout) void {
    for (callouts) |c| {
        allocator.free(c.callout_type);
        if (c.title) |t| allocator.free(t);
    }
    if (callouts.len > 0) allocator.free(callouts);
}

pub fn freeBlockRefs(allocator: Allocator, block_refs: []StoredBlockRef) void {
    for (block_refs) |br| {
        allocator.free(br.uuid);
    }
    if (block_refs.len > 0) allocator.free(block_refs);
}

fn freeStoredCalloutsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredCallout), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |c| {
            allocator.free(c.callout_type);
            if (c.title) |t| allocator.free(t);
        }
    }
    list.deinit(allocator);
}

fn freeStoredBlockRefsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredBlockRef), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |br| {
            allocator.free(br.uuid);
        }
    }
    list.deinit(allocator);
}

// ── Tests ───────────────────────────────────────────────────────────

test {
    _ = @import("document_test.zig");
}

