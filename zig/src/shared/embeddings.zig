const std = @import("std");
const math = std.math;
const Allocator = std.mem.Allocator;
const similarity = @import("similarity.zig");

/// A single entry in the embedding index: an ID string paired with its embedding vector.
pub const EmbeddingEntry = struct {
    /// Heap-allocated copy of the ID string (owned by this entry).
    id: []u8,
    /// Heap-allocated f32 vector (owned by this entry).
    vector: []f32,
};

/// Result of a top-K cosine similarity search.
pub const SearchResult = struct {
    /// Pointer to the entry's ID string (borrowed from index, valid until next mutation).
    id_ptr: [*]const u8,
    id_len: u32,
    score: f32,
};

/// In-memory embedding index with brute-force cosine similarity search.
///
/// Stores (id, vector) pairs and supports top-K retrieval by cosine distance.
/// Uses the provided allocator for all heap allocations. Caller must call
/// `deinit()` to free all memory.
///
/// Thread safety: NOT thread-safe. Caller must synchronize access (Rust uses
/// tokio::sync::Mutex per the BRZA spec).
pub const EmbeddingIndex = struct {
    dims: u32,
    entries: std.ArrayListUnmanaged(EmbeddingEntry),
    allocator: Allocator,

    /// Create a new embedding index for vectors of the given dimensionality.
    /// Returns null if dims == 0.
    pub fn init(allocator: Allocator, dims: u32) ?EmbeddingIndex {
        if (dims == 0) return null;
        return EmbeddingIndex{
            .dims = dims,
            .entries = .{},
            .allocator = allocator,
        };
    }

    /// Free all entries and internal storage.
    pub fn deinit(self: *EmbeddingIndex) void {
        for (self.entries.items) |entry| {
            self.allocator.free(entry.id);
            self.allocator.free(entry.vector);
        }
        self.entries.deinit(self.allocator);
    }

    /// Add an embedding to the index. Copies both the ID string and vector.
    ///
    /// If an entry with the same ID already exists, it is replaced.
    ///
    /// Returns:
    ///   0  — success
    ///  -1  — invalid input (null-like conditions handled by caller)
    ///  -3  — allocation failure
    pub fn add(self: *EmbeddingIndex, id: []const u8, vector: []const f32) i32 {
        if (vector.len != self.dims) return -1;
        if (id.len == 0) return -1;

        // Check for existing entry with same ID — replace if found
        for (self.entries.items, 0..) |*entry, i| {
            if (std.mem.eql(u8, entry.id, id)) {
                // Replace the vector
                @memcpy(self.entries.items[i].vector, vector);
                return 0;
            }
        }

        // Allocate copies
        const id_copy = self.allocator.dupe(u8, id) catch return -3;
        errdefer self.allocator.free(id_copy);

        const vec_copy = self.allocator.dupe(f32, vector) catch {
            self.allocator.free(id_copy);
            return -3;
        };

        self.entries.append(self.allocator, .{
            .id = id_copy,
            .vector = vec_copy,
        }) catch {
            self.allocator.free(id_copy);
            self.allocator.free(vec_copy);
            return -3;
        };

        return 0;
    }

    /// Search for the top-K most similar embeddings to the query.
    ///
    /// Writes results into `results[0..k]`, sorted by descending similarity.
    /// Returns the number of results written (may be less than k if index
    /// has fewer entries).
    ///
    /// Returns -1 for invalid input, -2 if k == 0.
    pub fn search(
        self: *const EmbeddingIndex,
        query: []const f32,
        results: []SearchResult,
    ) i32 {
        if (query.len != self.dims) return -1;
        if (results.len == 0) return -2;

        const k = results.len;
        const n = self.entries.items.len;
        if (n == 0) return 0;

        // Brute-force: compute cosine similarity against all entries
        // Use a simple insertion-sort approach for top-K
        var result_count: usize = 0;

        for (self.entries.items) |entry| {
            const score = similarity.cosine_similarity(
                query.ptr,
                entry.vector.ptr,
                self.dims,
            );

            // Skip error results from cosine (e.g., zero-magnitude)
            if (score <= -1.5) continue;

            if (result_count < k) {
                // Still filling up results
                results[result_count] = .{
                    .id_ptr = entry.id.ptr,
                    .id_len = @intCast(entry.id.len),
                    .score = score,
                };
                result_count += 1;

                // Insertion sort to maintain descending order
                var j = result_count - 1;
                while (j > 0 and results[j].score > results[j - 1].score) {
                    const tmp = results[j];
                    results[j] = results[j - 1];
                    results[j - 1] = tmp;
                    j -= 1;
                }
            } else if (score > results[k - 1].score) {
                // Replace the lowest-scoring result
                results[k - 1] = .{
                    .id_ptr = entry.id.ptr,
                    .id_len = @intCast(entry.id.len),
                    .score = score,
                };

                // Bubble up to maintain descending order
                var j = k - 1;
                while (j > 0 and results[j].score > results[j - 1].score) {
                    const tmp = results[j];
                    results[j] = results[j - 1];
                    results[j - 1] = tmp;
                    j -= 1;
                }
            }
        }

        return @intCast(result_count);
    }

    /// Return the number of entries in the index.
    pub fn count(self: *const EmbeddingIndex) u32 {
        return @intCast(self.entries.items.len);
    }

    /// Return the dimensionality of vectors in this index.
    pub fn dimensions(self: *const EmbeddingIndex) u32 {
        return self.dims;
    }
};

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "create_empty_index" {
    var idx = EmbeddingIndex.init(testing.allocator, 384) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    try testing.expectEqual(@as(u32, 0), idx.count());
    try testing.expectEqual(@as(u32, 384), idx.dimensions());
}

test "create_zero_dims_returns_null" {
    const idx = EmbeddingIndex.init(testing.allocator, 0);
    try testing.expect(idx == null);
}

test "add_and_count" {
    var idx = EmbeddingIndex.init(testing.allocator, 4) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    const v1 = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    const v2 = [_]f32{ 0.0, 1.0, 0.0, 0.0 };

    try testing.expectEqual(@as(i32, 0), idx.add("doc1", &v1));
    try testing.expectEqual(@as(u32, 1), idx.count());

    try testing.expectEqual(@as(i32, 0), idx.add("doc2", &v2));
    try testing.expectEqual(@as(u32, 2), idx.count());
}

test "add_wrong_dims_returns_error" {
    var idx = EmbeddingIndex.init(testing.allocator, 4) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    const wrong = [_]f32{ 1.0, 2.0 }; // dims=2, index expects 4
    try testing.expectEqual(@as(i32, -1), idx.add("doc1", &wrong));
    try testing.expectEqual(@as(u32, 0), idx.count());
}

test "add_empty_id_returns_error" {
    var idx = EmbeddingIndex.init(testing.allocator, 4) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    const v = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    try testing.expectEqual(@as(i32, -1), idx.add("", &v));
}

test "add_duplicate_replaces" {
    var idx = EmbeddingIndex.init(testing.allocator, 4) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    const v1 = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    const v2 = [_]f32{ 0.0, 1.0, 0.0, 0.0 };

    try testing.expectEqual(@as(i32, 0), idx.add("doc1", &v1));
    try testing.expectEqual(@as(u32, 1), idx.count());

    // Replace with different vector
    try testing.expectEqual(@as(i32, 0), idx.add("doc1", &v2));
    try testing.expectEqual(@as(u32, 1), idx.count()); // still 1, not 2

    // Search should find the new vector
    var results: [1]SearchResult = undefined;
    const query = [_]f32{ 0.0, 1.0, 0.0, 0.0 };
    const n = idx.search(&query, &results);
    try testing.expectEqual(@as(i32, 1), n);
    try testing.expectApproxEqAbs(@as(f32, 1.0), results[0].score, 1e-5);
}

test "search_top_k_ordering" {
    var idx = EmbeddingIndex.init(testing.allocator, 4) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    // Add vectors with known cosine distances to query [1,0,0,0]
    const query = [_]f32{ 1.0, 0.0, 0.0, 0.0 };

    // Identical to query — cosine = 1.0
    try testing.expectEqual(@as(i32, 0), idx.add("exact", &[_]f32{ 1.0, 0.0, 0.0, 0.0 }));
    // Orthogonal — cosine = 0.0
    try testing.expectEqual(@as(i32, 0), idx.add("ortho", &[_]f32{ 0.0, 1.0, 0.0, 0.0 }));
    // Similar — cosine ≈ 0.707
    try testing.expectEqual(@as(i32, 0), idx.add("similar", &[_]f32{ 1.0, 1.0, 0.0, 0.0 }));
    // Opposite — cosine = -1.0
    try testing.expectEqual(@as(i32, 0), idx.add("opposite", &[_]f32{ -1.0, 0.0, 0.0, 0.0 }));

    var results: [3]SearchResult = undefined;
    const n = idx.search(&query, &results);
    try testing.expectEqual(@as(i32, 3), n);

    // Results should be sorted by descending score
    try testing.expect(results[0].score >= results[1].score);
    try testing.expect(results[1].score >= results[2].score);

    // Top result should be "exact" (cosine = 1.0)
    const top_id = results[0].id_ptr[0..results[0].id_len];
    try testing.expectEqualStrings("exact", top_id);

    // Second should be "similar" (cosine ≈ 0.707)
    const second_id = results[1].id_ptr[0..results[1].id_len];
    try testing.expectEqualStrings("similar", second_id);
}

test "search_empty_index_returns_zero" {
    var idx = EmbeddingIndex.init(testing.allocator, 4) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    const query = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    var results: [5]SearchResult = undefined;
    const n = idx.search(&query, &results);
    try testing.expectEqual(@as(i32, 0), n);
}

test "search_wrong_dims_returns_error" {
    var idx = EmbeddingIndex.init(testing.allocator, 4) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    try testing.expectEqual(@as(i32, 0), idx.add("doc1", &[_]f32{ 1.0, 0.0, 0.0, 0.0 }));

    const wrong_query = [_]f32{ 1.0, 0.0 }; // dims=2, index is 4
    var results: [5]SearchResult = undefined;
    const n = idx.search(&wrong_query, &results);
    try testing.expectEqual(@as(i32, -1), n);
}

test "create_destroy_no_leak" {
    // testing.allocator (GeneralPurposeAllocator) detects leaks
    var idx = EmbeddingIndex.init(testing.allocator, 128) orelse
        return error.TestUnexpectedResult;

    // Add several entries
    var vec: [128]f32 = undefined;
    for (0..128) |i| {
        vec[i] = @floatFromInt(i);
    }

    var id_buf: [8]u8 = undefined;
    for (0..20) |i| {
        const id_len = std.fmt.bufPrint(&id_buf, "doc-{d}", .{i}) catch unreachable;
        _ = idx.add(id_len, &vec);
    }

    try testing.expectEqual(@as(u32, 20), idx.count());

    // deinit should free everything — testing.allocator will report leaks
    idx.deinit();
}

test "search_100_embeddings" {
    var idx = EmbeddingIndex.init(testing.allocator, 8) orelse
        return error.TestUnexpectedResult;
    defer idx.deinit();

    // Add 100 embeddings with different patterns
    var vec: [8]f32 = undefined;
    var id_buf: [8]u8 = undefined;
    for (0..100) |i| {
        for (0..8) |d| {
            // Each entry has a different pattern
            vec[d] = @as(f32, @floatFromInt((i + d) % 17)) / 17.0;
        }
        const id_len = std.fmt.bufPrint(&id_buf, "e-{d:0>3}", .{i}) catch unreachable;
        try testing.expectEqual(@as(i32, 0), idx.add(id_len, &vec));
    }

    try testing.expectEqual(@as(u32, 100), idx.count());

    // Search with a known query
    const query = [_]f32{ 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0 };
    var results: [5]SearchResult = undefined;
    const n = idx.search(&query, &results);
    try testing.expect(n > 0);
    try testing.expect(n <= 5);

    // Verify descending order
    const result_count: usize = @intCast(n);
    for (1..result_count) |j| {
        try testing.expect(results[j - 1].score >= results[j].score);
    }
}

test "dimensions_common_sizes" {
    // Verify index works with common embedding dimensions
    const dim_sizes = [_]u32{ 128, 384, 768, 1536 };
    for (dim_sizes) |dims| {
        var idx = EmbeddingIndex.init(testing.allocator, dims) orelse
            return error.TestUnexpectedResult;
        defer idx.deinit();

        try testing.expectEqual(dims, idx.dimensions());

        // Add one entry to verify it works at this dimension
        const vec = try testing.allocator.alloc(f32, dims);
        defer testing.allocator.free(vec);
        for (vec) |*v| v.* = 1.0;

        try testing.expectEqual(@as(i32, 0), idx.add("test", vec));
        try testing.expectEqual(@as(u32, 1), idx.count());
    }
}
