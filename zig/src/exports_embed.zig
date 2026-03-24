const std = @import("std");
const embeddings = @import("shared/embeddings.zig");
const EmbeddingIndex = embeddings.EmbeddingIndex;
const SearchResult = embeddings.SearchResult;

/// Allocator used for embedding index heap allocations.
/// page_allocator is the simplest choice for long-lived, FFI-owned memory.
const index_allocator = std.heap.page_allocator;

/// Create a new embedding index for vectors of the given dimensionality.
///
/// Returns an opaque handle on success, or null if dims == 0.
/// The caller (Rust) owns the handle and MUST call `zig_embedding_index_destroy`
/// to free all memory.
export fn zig_embedding_index_create(dims: u32) ?*anyopaque {
    const idx = index_allocator.create(EmbeddingIndex) catch return null;
    idx.* = EmbeddingIndex.init(index_allocator, dims) orelse {
        index_allocator.destroy(idx);
        return null;
    };
    return @ptrCast(idx);
}

/// Destroy an embedding index, freeing all entries and internal storage.
///
/// After this call the handle is invalid. Passing null is a no-op.
export fn zig_embedding_index_destroy(handle: ?*anyopaque) void {
    const idx = castHandle(handle) orelse return;
    idx.deinit();
    index_allocator.destroy(idx);
}

/// Add an embedding to the index. Both the ID and vector are copied internally.
///
/// If an entry with the same ID already exists, its vector is replaced.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null handle, null pointers, wrong dims, empty id)
///  -3  — allocation failure
export fn zig_embedding_index_add(
    handle: ?*anyopaque,
    id: ?[*]const u8,
    id_len: u32,
    embedding: ?[*]const f32,
    dims: u32,
) i32 {
    const idx = castHandle(handle) orelse return -1;
    const id_ptr = id orelse return -1;
    const emb_ptr = embedding orelse return -1;
    if (id_len == 0) return -1;
    if (dims == 0) return -1;

    return idx.add(id_ptr[0..id_len], emb_ptr[0..dims]);
}

/// Remove an entry from the index by ID, freeing its allocations.
///
/// Returns:
///   0  — entry found and removed
///  -1  — invalid input (null handle, null id, empty id) or entry not found
export fn zig_embedding_index_remove(
    handle: ?*anyopaque,
    id: ?[*]const u8,
    id_len: u32,
) i32 {
    const idx = castHandle(handle) orelse return -1;
    const id_ptr = id orelse return -1;
    if (id_len == 0) return -1;

    return if (idx.remove(id_ptr[0..id_len])) 0 else -1;
}

/// Search the index for the top-K most similar embeddings to the query.
///
/// Results are written into caller-provided parallel arrays:
///   result_ids[i]      — pointer to the i-th result's ID string (borrowed from index)
///   result_id_lens[i]  — length of the i-th result's ID string
///   result_scores[i]   — cosine similarity score of the i-th result
///
/// Results are sorted by descending similarity score.
///
/// Returns:
///   0   — success (actual count is written to `written`)
///  -1   — invalid input (null handle/pointers, wrong dims)
///  -2   — k == 0
///  -3   — allocation failure (k > 256)
export fn zig_embedding_index_search(
    handle: ?*anyopaque,
    query: ?[*]const f32,
    dims: u32,
    result_ids: ?[*][*]const u8,
    result_id_lens: ?[*]u32,
    result_scores: ?[*]f32,
    k: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    if (k == 0) {
        w.* = 0;
        return -2;
    }
    const idx = castHandle(handle) orelse return -1;
    const q = query orelse return -1;
    const r_ids = result_ids orelse return -1;
    const r_lens = result_id_lens orelse return -1;
    const r_scores = result_scores orelse return -1;

    if (dims == 0) return -1;

    // Allocate temporary SearchResult buffer on the stack (up to 256),
    // or use page_allocator for larger k values.
    const max_stack_k: u32 = 256;
    if (k <= max_stack_k) {
        var buf: [max_stack_k]SearchResult = undefined;
        const result = idx.search(q[0..dims], buf[0..k]);
        if (result < 0) return result;

        const n: u32 = @intCast(result);
        w.* = n;
        for (0..n) |i| {
            r_ids[i] = buf[i].id_ptr;
            r_lens[i] = buf[i].id_len;
            r_scores[i] = buf[i].score;
        }
        return 0;
    } else {
        const buf = index_allocator.alloc(SearchResult, k) catch return -3;
        defer index_allocator.free(buf);

        const result = idx.search(q[0..dims], buf);
        if (result < 0) return result;

        const n: u32 = @intCast(result);
        w.* = n;
        for (0..n) |i| {
            r_ids[i] = buf[i].id_ptr;
            r_lens[i] = buf[i].id_len;
            r_scores[i] = buf[i].score;
        }
        return 0;
    }
}

/// Return the number of entries in the index, or -1 for null handle.
export fn zig_embedding_index_count(handle: ?*anyopaque) i32 {
    const idx = castHandleConst(handle) orelse return -1;
    return @intCast(idx.count());
}

/// Return the dimensionality of vectors in this index, or -1 for null handle.
export fn zig_embedding_index_dimensions(handle: ?*anyopaque) i32 {
    const idx = castHandleConst(handle) orelse return -1;
    return @intCast(idx.dimensions());
}

// ============================================================================
// Helpers
// ============================================================================

fn castHandle(handle: ?*anyopaque) ?*EmbeddingIndex {
    const ptr = handle orelse return null;
    return @ptrCast(@alignCast(ptr));
}

fn castHandleConst(handle: ?*anyopaque) ?*const EmbeddingIndex {
    const ptr = handle orelse return null;
    return @ptrCast(@alignCast(ptr));
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "zig_embedding_index_create_and_destroy" {
    const handle = zig_embedding_index_create(384);
    try testing.expect(handle != null);

    try testing.expectEqual(@as(i32, 384), zig_embedding_index_dimensions(handle));
    try testing.expectEqual(@as(i32, 0), zig_embedding_index_count(handle));

    zig_embedding_index_destroy(handle);
}

test "zig_embedding_index_create_zero_dims" {
    const handle = zig_embedding_index_create(0);
    try testing.expect(handle == null);
}

test "zig_embedding_index_destroy_null" {
    // Should be a no-op, not crash
    zig_embedding_index_destroy(null);
}

test "zig_embedding_index_add_basic" {
    const handle = zig_embedding_index_create(4);
    try testing.expect(handle != null);
    defer zig_embedding_index_destroy(handle);

    const v = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    const rc = zig_embedding_index_add(handle, "doc1", 4, &v, 4);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(i32, 1), zig_embedding_index_count(handle));
}

test "zig_embedding_index_add_null_handle" {
    const v = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    const rc = zig_embedding_index_add(null, "doc1", 4, &v, 4);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "zig_embedding_index_add_null_id" {
    const handle = zig_embedding_index_create(4);
    defer zig_embedding_index_destroy(handle);

    const v = [_]f32{ 1.0, 0.0 };
    const rc = zig_embedding_index_add(handle, null, 0, &v, 4);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "zig_embedding_index_add_null_embedding" {
    const handle = zig_embedding_index_create(4);
    defer zig_embedding_index_destroy(handle);

    const rc = zig_embedding_index_add(handle, "doc1", 4, null, 4);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "zig_embedding_index_add_dims_mismatch" {
    const handle = zig_embedding_index_create(4);
    defer zig_embedding_index_destroy(handle);

    const v = [_]f32{ 1.0, 0.0 }; // 2 dims, index expects 4
    const rc = zig_embedding_index_add(handle, "doc1", 4, &v, 2);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "zig_embedding_index_search_basic" {
    const handle = zig_embedding_index_create(4);
    try testing.expect(handle != null);
    defer zig_embedding_index_destroy(handle);

    // Add two entries
    const v1 = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    const v2 = [_]f32{ 0.0, 1.0, 0.0, 0.0 };
    _ = zig_embedding_index_add(handle, "doc1", 4, &v1, 4);
    _ = zig_embedding_index_add(handle, "doc2", 4, &v2, 4);

    // Search for v1
    var r_ids: [2][*]const u8 = undefined;
    var r_lens: [2]u32 = undefined;
    var r_scores: [2]f32 = undefined;
    var written: u32 = undefined;

    const rc = zig_embedding_index_search(
        handle,
        &v1,
        4,
        &r_ids,
        &r_lens,
        &r_scores,
        2,
        &written,
    );

    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 2), written);

    // First result should be doc1 with score ≈ 1.0
    const top_id = r_ids[0][0..r_lens[0]];
    try testing.expectEqualStrings("doc1", top_id);
    try testing.expectApproxEqAbs(@as(f32, 1.0), r_scores[0], 1e-5);
}

test "zig_embedding_index_search_empty_index" {
    const handle = zig_embedding_index_create(4);
    defer zig_embedding_index_destroy(handle);

    const query = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    var r_ids: [2][*]const u8 = undefined;
    var r_lens: [2]u32 = undefined;
    var r_scores: [2]f32 = undefined;
    var written: u32 = undefined;

    const rc = zig_embedding_index_search(
        handle,
        &query,
        4,
        &r_ids,
        &r_lens,
        &r_scores,
        2,
        &written,
    );

    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), written);
}

test "zig_embedding_index_search_null_handle" {
    const query = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    var r_ids: [2][*]const u8 = undefined;
    var r_lens: [2]u32 = undefined;
    var r_scores: [2]f32 = undefined;
    var written: u32 = undefined;

    const rc = zig_embedding_index_search(
        null,
        &query,
        4,
        &r_ids,
        &r_lens,
        &r_scores,
        2,
        &written,
    );

    try testing.expectEqual(@as(i32, -1), rc);
}

test "zig_embedding_index_search_k_zero" {
    const handle = zig_embedding_index_create(4);
    defer zig_embedding_index_destroy(handle);

    const query = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    var written: u32 = undefined;

    const rc = zig_embedding_index_search(
        handle,
        &query,
        4,
        null,
        null,
        null,
        0,
        &written,
    );

    try testing.expectEqual(@as(i32, -2), rc);
}

test "zig_embedding_index_count_null_handle" {
    try testing.expectEqual(@as(i32, -1), zig_embedding_index_count(null));
}

test "zig_embedding_index_dimensions_null_handle" {
    try testing.expectEqual(@as(i32, -1), zig_embedding_index_dimensions(null));
}

test "zig_embedding_index_remove_basic" {
    const handle = zig_embedding_index_create(4);
    try testing.expect(handle != null);
    defer zig_embedding_index_destroy(handle);

    const v1 = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    const v2 = [_]f32{ 0.0, 1.0, 0.0, 0.0 };
    _ = zig_embedding_index_add(handle, "doc1", 4, &v1, 4);
    _ = zig_embedding_index_add(handle, "doc2", 4, &v2, 4);
    try testing.expectEqual(@as(i32, 2), zig_embedding_index_count(handle));

    try testing.expectEqual(@as(i32, 0), zig_embedding_index_remove(handle, "doc1", 4));
    try testing.expectEqual(@as(i32, 1), zig_embedding_index_count(handle));
}

test "zig_embedding_index_remove_nonexistent" {
    const handle = zig_embedding_index_create(4);
    defer zig_embedding_index_destroy(handle);

    try testing.expectEqual(@as(i32, -1), zig_embedding_index_remove(handle, "nope", 4));
}

test "zig_embedding_index_remove_null_handle" {
    try testing.expectEqual(@as(i32, -1), zig_embedding_index_remove(null, "doc1", 4));
}

test "zig_embedding_index_remove_null_id" {
    const handle = zig_embedding_index_create(4);
    defer zig_embedding_index_destroy(handle);

    try testing.expectEqual(@as(i32, -1), zig_embedding_index_remove(handle, null, 0));
}

test "zig_embedding_index_lifecycle_100_entries" {
    const handle = zig_embedding_index_create(8);
    try testing.expect(handle != null);
    defer zig_embedding_index_destroy(handle);

    // Add 100 entries
    var vec: [8]f32 = undefined;
    var id_buf: [8]u8 = undefined;
    for (0..100) |i| {
        for (0..8) |d| {
            vec[d] = @as(f32, @floatFromInt((i + d) % 17)) / 17.0;
        }
        const id_slice = std.fmt.bufPrint(&id_buf, "e-{d:0>3}", .{i}) catch unreachable;
        const rc = zig_embedding_index_add(handle, id_slice.ptr, @intCast(id_slice.len), &vec, 8);
        try testing.expectEqual(@as(i32, 0), rc);
    }

    try testing.expectEqual(@as(i32, 100), zig_embedding_index_count(handle));
    try testing.expectEqual(@as(i32, 8), zig_embedding_index_dimensions(handle));

    // Search for top-5
    const query = [_]f32{ 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0 };
    var r_ids: [5][*]const u8 = undefined;
    var r_lens: [5]u32 = undefined;
    var r_scores: [5]f32 = undefined;
    var written: u32 = undefined;

    const rc = zig_embedding_index_search(handle, &query, 8, &r_ids, &r_lens, &r_scores, 5, &written);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expect(written > 0);
    try testing.expect(written <= 5);

    // Verify descending order
    for (1..written) |j| {
        try testing.expect(r_scores[j - 1] >= r_scores[j]);
    }
}
