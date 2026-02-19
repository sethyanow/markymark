const std = @import("std");
const similarity = @import("../shared/similarity.zig");

pub const RankedMatch = struct {
    score: i32,
    index: u32,
};

fn less_than(_: void, a: RankedMatch, b: RankedMatch) bool {
    if (a.score != b.score) return a.score > b.score;
    return a.index < b.index;
}

/// Scalar reference ranking for fuzzy matches.
///
/// Returns candidates with score > 0, sorted deterministically:
/// - score descending
/// - candidate index ascending on ties
pub fn rank_candidates(
    allocator: std.mem.Allocator,
    query: []const u8,
    candidates: []const []const u8,
    top_k: usize,
) ![]RankedMatch {
    if (query.len == 0 or candidates.len == 0 or top_k == 0) {
        return allocator.alloc(RankedMatch, 0);
    }

    var all = std.ArrayListUnmanaged(RankedMatch){};
    defer all.deinit(allocator);

    for (candidates, 0..) |candidate, i| {
        const score = similarity.fuzzy_match_score(
            query.ptr,
            @intCast(query.len),
            candidate.ptr,
            @intCast(candidate.len),
        );
        if (score <= 0) continue;
        try all.append(allocator, .{
            .score = score,
            .index = @intCast(i),
        });
    }

    std.mem.sort(RankedMatch, all.items, {}, less_than);
    const n = @min(top_k, all.items.len);
    const out = try allocator.alloc(RankedMatch, n);
    @memcpy(out, all.items[0..n]);
    return out;
}

const testing = std.testing;

test "rank_candidates deterministic tie ordering" {
    const candidates = [_][]const u8{
        "acb",
        "adb",
        "aeb",
    };

    const ranked = try rank_candidates(testing.allocator, "ab", &candidates, 2);
    defer testing.allocator.free(ranked);

    try testing.expectEqual(@as(usize, 2), ranked.len);
    try testing.expectEqual(@as(u32, 0), ranked[0].index);
    try testing.expectEqual(@as(u32, 1), ranked[1].index);
}
