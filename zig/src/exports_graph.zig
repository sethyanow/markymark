const std = @import("std");
const link_graph = @import("kernels/link_graph.zig");
const LinkGraph = link_graph.LinkGraph;

/// Allocator used for graph heap allocations.
/// page_allocator is the simplest choice for long-lived, FFI-owned memory.
const graph_allocator = std.heap.page_allocator;

/// Helper to cast opaque handle to LinkGraph pointer.
fn castHandle(handle: ?*anyopaque) ?*LinkGraph {
    const ptr = handle orelse return null;
    return @ptrCast(@alignCast(ptr));
}

// ============================================================================
// Lifecycle
// ============================================================================

/// Create a new link graph.
///
/// Returns an opaque handle on success, or null on allocation failure.
/// The caller (Rust) owns the handle and MUST call `marky_graph_destroy`.
export fn marky_graph_create() ?*anyopaque {
    const g = graph_allocator.create(LinkGraph) catch return null;
    g.* = LinkGraph.init(graph_allocator);
    return @ptrCast(g);
}

/// Destroy a link graph, freeing all nodes and internal storage.
///
/// After this call the handle is invalid. Passing null is a no-op.
export fn marky_graph_destroy(handle: ?*anyopaque) void {
    const g = castHandle(handle) orelse return;
    g.deinit();
    graph_allocator.destroy(g);
}

// ============================================================================
// Mutation
// ============================================================================

/// Add a document with outbound links to the graph.
///
/// If the document already exists, its edges are replaced.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null handle or null targets with nonzero count)
///  -3  — allocation failure
export fn marky_graph_add_document(
    handle: ?*anyopaque,
    doc_id: u32,
    targets: ?[*]const u32,
    target_count: u32,
) i32 {
    const g = castHandle(handle) orelse return -1;

    if (target_count == 0) {
        return g.addDocument(doc_id, &.{});
    }

    const t = targets orelse return -1;
    return g.addDocument(doc_id, t[0..target_count]);
}

/// Remove a document from the graph.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null handle or doc not found)
export fn marky_graph_remove_document(handle: ?*anyopaque, doc_id: u32) i32 {
    const g = castHandle(handle) orelse return -1;
    return g.removeDocument(doc_id);
}

// ============================================================================
// Queries
// ============================================================================

/// Get the number of documents in the graph.
export fn marky_graph_count(handle: ?*anyopaque) u32 {
    const g = castHandle(handle) orelse return 0;
    return g.count();
}

/// Find documents with zero inbound links (orphans).
///
/// Writes orphan doc IDs into `out[0..cap]`.
///
/// Returns:
///   >=0 — number of orphans written
///   -1  — invalid input (null handle or null output)
///   -2  — buffer too small (partial results written)
export fn marky_graph_find_orphans(
    handle: ?*anyopaque,
    out: ?[*]u32,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    w.* = 0;

    const g = castHandle(handle) orelse return -1;
    const o = out orelse {
        if (cap == 0) return 0;
        return -1;
    };

    const rc = g.findOrphans(o, cap);
    if (rc >= 0) {
        w.* = @intCast(rc);
    } else if (rc == -2) {
        w.* = cap; // partial
    }
    return rc;
}

/// Find all documents that transitively link to a target.
///
/// Returns:
///   >=0 — number written
///   -1  — invalid input
///   -2  — buffer too small
///   -3  — allocation failure
export fn marky_graph_find_broken_chains(
    handle: ?*anyopaque,
    target: u32,
    out: ?[*]u32,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    w.* = 0;

    const g = castHandle(handle) orelse return -1;
    const o = out orelse {
        if (cap == 0) return 0;
        return -1;
    };

    // Use page_allocator for scratch since we can't pass testing_allocator through C ABI
    const rc = g.findBrokenChains(target, o, cap, graph_allocator);
    if (rc >= 0) {
        w.* = @intCast(rc);
    } else if (rc == -2) {
        w.* = cap;
    }
    return rc;
}

/// Compute PageRank scores for all documents.
///
/// Results are written into parallel arrays: ids[i] and scores[i].
///
/// Returns:
///   >=0 — number of nodes scored
///   -1  — invalid input (null handle, zero iterations)
///   -2  — buffer too small
///   -3  — allocation failure
export fn marky_graph_compute_pagerank(
    handle: ?*anyopaque,
    iterations: u32,
    damping: f32,
    ids: ?[*]u32,
    scores: ?[*]f32,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    w.* = 0;

    const g = castHandle(handle) orelse return -1;
    const id_buf = ids orelse return -1;
    const score_buf = scores orelse return -1;

    const rc = g.computePagerank(iterations, damping, id_buf, score_buf, cap, graph_allocator);
    if (rc >= 0) {
        w.* = @intCast(rc);
    }
    return rc;
}

/// Compute connectivity statistics.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null handle or null out params)
///  -3  — allocation failure
export fn marky_graph_connectivity_stats(
    handle: ?*anyopaque,
    out_components: ?*u32,
    out_avg_degree_x100: ?*u32,
    out_max_degree: ?*u32,
) i32 {
    const g = castHandle(handle) orelse return -1;
    const comp = out_components orelse return -1;
    const avg = out_avg_degree_x100 orelse return -1;
    const max_deg = out_max_degree orelse return -1;

    return g.connectivityStats(comp, avg, max_deg, graph_allocator);
}

// ============================================================================
// C ABI integration tests
// ============================================================================

test "C ABI: create and destroy graph" {
    const h = marky_graph_create();
    try std.testing.expect(h != null);
    marky_graph_destroy(h);
}

test "C ABI: destroy null is no-op" {
    marky_graph_destroy(null);
}

test "C ABI: add document and count" {
    const h = marky_graph_create();
    defer marky_graph_destroy(h);

    const targets = [_]u32{ 2, 3 };
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 1, &targets, 2));
    try std.testing.expectEqual(@as(u32, 3), marky_graph_count(h));
}

test "C ABI: add document with no targets" {
    const h = marky_graph_create();
    defer marky_graph_destroy(h);

    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 1, null, 0));
    try std.testing.expectEqual(@as(u32, 1), marky_graph_count(h));
}

test "C ABI: add document null handle" {
    try std.testing.expectEqual(@as(i32, -1), marky_graph_add_document(null, 1, null, 0));
}

test "C ABI: remove document" {
    const h = marky_graph_create();
    defer marky_graph_destroy(h);

    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 1, null, 0));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_remove_document(h, 1));
    try std.testing.expectEqual(@as(u32, 0), marky_graph_count(h));
}

test "C ABI: remove non-existent returns -1" {
    const h = marky_graph_create();
    defer marky_graph_destroy(h);

    try std.testing.expectEqual(@as(i32, -1), marky_graph_remove_document(h, 999));
}

test "C ABI: find orphans" {
    const h = marky_graph_create();
    defer marky_graph_destroy(h);

    const targets = [_]u32{2};
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 1, &targets, 1));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 2, null, 0));

    var out: [8]u32 = undefined;
    var w: u32 = undefined;
    const rc = marky_graph_find_orphans(h, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 1), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 1), out[0]); // doc 1 is the only orphan
}

test "C ABI: find orphans null handle" {
    var w: u32 = undefined;
    var out: [4]u32 = undefined;
    try std.testing.expectEqual(@as(i32, -1), marky_graph_find_orphans(null, &out, 4, &w));
}

test "C ABI: find broken chains" {
    const h = marky_graph_create();
    defer marky_graph_destroy(h);

    // A(1) -> B(2) -> C(3)
    const t1 = [_]u32{2};
    const t2 = [_]u32{3};
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 1, &t1, 1));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 2, &t2, 1));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 3, null, 0));

    var out: [8]u32 = undefined;
    var w: u32 = undefined;
    const rc = marky_graph_find_broken_chains(h, 3, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 2), rc); // both 1 and 2 link to 3 transitively
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "C ABI: compute pagerank" {
    const h = marky_graph_create();
    defer marky_graph_destroy(h);

    // B,C,D -> A
    const t_a = [_]u32{1};
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 1, null, 0));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 2, &t_a, 1));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 3, &t_a, 1));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 4, &t_a, 1));

    var ids: [8]u32 = undefined;
    var scores: [8]f32 = undefined;
    var w: u32 = undefined;
    const rc = marky_graph_compute_pagerank(h, 20, 0.85, &ids, &scores, 8, &w);
    try std.testing.expectEqual(@as(i32, 4), rc);
    try std.testing.expectEqual(@as(u32, 4), w);
}

test "C ABI: connectivity stats" {
    const h = marky_graph_create();
    defer marky_graph_destroy(h);

    const t = [_]u32{2};
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 1, &t, 1));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 2, null, 0));
    try std.testing.expectEqual(@as(i32, 0), marky_graph_add_document(h, 3, null, 0));

    var components: u32 = undefined;
    var avg_degree: u32 = undefined;
    var max_degree: u32 = undefined;
    const rc = marky_graph_connectivity_stats(h, &components, &avg_degree, &max_degree);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), components);
}

test "C ABI: connectivity stats null handle" {
    var c: u32 = undefined;
    var a: u32 = undefined;
    var m: u32 = undefined;
    try std.testing.expectEqual(@as(i32, -1), marky_graph_connectivity_stats(null, &c, &a, &m));
}
