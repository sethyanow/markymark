// Document helpers: standalone pure functions used by document.zig.
//
// No dependency on DocumentEngine state. Used for slug generation,
// line/position mapping, and fence range filtering.

const std = @import("std");
const Allocator = std.mem.Allocator;

const extraction_renderer = @import("../md4c/extraction_renderer.zig");
const slug_kernel = @import("../kernels/slug.zig");
const fence_map_mod = @import("../kernels/fence_map.zig");
const stored_types = @import("stored_types.zig");

const Position = stored_types.Position;

// ── Slug helpers ────────────────────────────────────────────────────

/// Slugify heading text and deduplicate against previously processed headings.
/// Uses O(n^2) scan for simplicity (n = heading count, typically < 100).
pub fn makeSlug(
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

pub fn inFenceRange(ranges: []const fence_map_mod.FenceRange, pos: u32) bool {
    for (ranges) |r| {
        if (pos >= r.start and pos < r.end) return true;
    }
    return false;
}
