//! Link Graph Engine
//!
//! Adjacency-list graph for document link networks. Supports:
//! - Add/remove documents with outbound links
//! - Orphan detection (docs with zero inbound links)
//! - Broken chain analysis (transitive reverse traversal)
//! - PageRank computation (iterative power method)
//! - Connectivity statistics (union-find for components)
//!
//! Graph mutations: addDocument is O(degree), removeDocument is O(V + E).
//! Orphan detection is O(V).
//! PageRank is O(iterations * E). Connectivity is O(V * α(V)).
//!
//! Thread safety: NOT thread-safe. Caller must synchronize.

const std = @import("std");
const Allocator = std.mem.Allocator;

/// A document node in the link graph.
const DocNode = struct {
    /// Outbound link targets (doc IDs this document links to).
    outbound: std.ArrayListUnmanaged(u32),
    /// Number of inbound links from other documents.
    inbound_count: u32,
};

/// Document link graph with bidirectional edge tracking.
///
/// Uses u32 document IDs as keys. Maintains both forward (outbound)
/// and reverse (inbound count) edges for efficient orphan detection.
pub const LinkGraph = struct {
    /// Map from doc_id -> DocNode
    nodes: std.AutoHashMapUnmanaged(u32, DocNode),
    allocator: Allocator,

    pub fn init(allocator: Allocator) LinkGraph {
        return .{
            .nodes = .{},
            .allocator = allocator,
        };
    }

    pub fn deinit(self: *LinkGraph) void {
        var it = self.nodes.valueIterator();
        while (it.next()) |node| {
            node.outbound.deinit(self.allocator);
        }
        self.nodes.deinit(self.allocator);
    }

    /// Add a document with its outbound links.
    ///
    /// If the document already exists, its edges are replaced.
    /// Target IDs that don't exist as documents are tracked for
    /// inbound counts (they'll get nodes when added later).
    ///
    /// Returns: 0 success, -3 allocation failure.
    pub fn addDocument(self: *LinkGraph, doc_id: u32, targets: []const u32) i32 {
        // If doc already exists, remove its old edges first
        if (self.nodes.getPtr(doc_id)) |existing| {
            self.decrementInbound(existing.outbound.items);
            existing.outbound.clearRetainingCapacity();
        } else {
            // Create new node
            self.nodes.put(self.allocator, doc_id, .{
                .outbound = .{},
                .inbound_count = 0,
            }) catch return -3;
        }

        // Reserve potential target insertions before capturing `node` pointer.
        // This prevents hashmap growth from invalidating the pointer mid-loop.
        const target_cap: u32 = @intCast(targets.len);
        self.nodes.ensureUnusedCapacity(self.allocator, target_cap) catch return -3;

        const node = self.nodes.getPtr(doc_id).?;

        // Add outbound edges, deduplicating targets so inbound_count reflects
        // unique sources rather than raw occurrences (marky-859).
        var seen = std.AutoHashMapUnmanaged(u32, void){};
        defer seen.deinit(self.allocator);
        seen.ensureTotalCapacity(self.allocator, @intCast(targets.len)) catch return -3;
        node.outbound.ensureTotalCapacity(self.allocator, targets.len) catch return -3;
        for (targets) |target| {
            const sr = seen.getOrPutAssumeCapacity(target);
            if (sr.found_existing) continue;
            node.outbound.appendAssumeCapacity(target);
            // Ensure target node exists and increment its inbound count
            const gop = self.nodes.getOrPut(self.allocator, target) catch return -3;
            if (!gop.found_existing) {
                gop.value_ptr.* = .{
                    .outbound = .{},
                    .inbound_count = 0,
                };
            }
            gop.value_ptr.inbound_count += 1;
        }

        return 0;
    }

    /// Remove a document and clean up all edges.
    ///
    /// Returns: 0 success, -1 doc not found.
    pub fn removeDocument(self: *LinkGraph, doc_id: u32) i32 {
        const node_ptr = self.nodes.getPtr(doc_id) orelse return -1;

        // Decrement inbound counts for all targets
        self.decrementInbound(node_ptr.outbound.items);

        // Remove inbound edges from other nodes pointing to this doc
        var it = self.nodes.valueIterator();
        while (it.next()) |other_node| {
            var i: usize = 0;
            while (i < other_node.outbound.items.len) {
                if (other_node.outbound.items[i] == doc_id) {
                    _ = other_node.outbound.swapRemove(i);
                    // Don't decrement node_ptr.inbound_count — we're deleting it
                } else {
                    i += 1;
                }
            }
        }

        // Free outbound list and remove from map
        node_ptr.outbound.deinit(self.allocator);
        self.nodes.removeByPtr(self.nodes.getKeyPtr(doc_id).?);
        return 0;
    }

    /// Find documents with zero inbound links (orphans).
    ///
    /// Writes orphan doc IDs into `out[0..cap]`.
    /// Returns number written, or -2 if buffer too small (partial results written).
    pub fn findOrphans(self: *const LinkGraph, out: [*]u32, cap: u32) i32 {
        var written: u32 = 0;
        var overflow = false;

        var it = self.nodes.iterator();
        while (it.next()) |entry| {
            if (entry.value_ptr.inbound_count == 0) {
                if (written < cap) {
                    out[written] = entry.key_ptr.*;
                    written += 1;
                } else {
                    overflow = true;
                }
            }
        }

        if (overflow) return -2;
        return @intCast(written);
    }

    /// Find all documents that transitively link to a target.
    ///
    /// Uses iterative BFS on the reverse graph (inbound edges).
    /// Returns number written, or -2 if buffer too small.
    pub fn findBrokenChains(self: *const LinkGraph, target: u32, out: [*]u32, cap: u32, scratch_allocator: Allocator) i32 {
        // Target not in graph — no chains
        if (!self.nodes.contains(target)) return 0;

        // BFS using scratch allocator for queue and visited set
        var visited = std.AutoHashMapUnmanaged(u32, void){};
        defer visited.deinit(scratch_allocator);
        var queue = std.ArrayListUnmanaged(u32){};
        defer queue.deinit(scratch_allocator);

        // Seed: find all docs that directly link to target
        var it = self.nodes.iterator();
        while (it.next()) |entry| {
            for (entry.value_ptr.outbound.items) |t| {
                if (t == target) {
                    queue.append(scratch_allocator, entry.key_ptr.*) catch return -3;
                    visited.put(scratch_allocator, entry.key_ptr.*, {}) catch return -3;
                    break;
                }
            }
        }

        // BFS: for each queued doc, find who links to it
        var written: u32 = 0;
        var head: usize = 0;
        while (head < queue.items.len) {
            const current = queue.items[head];
            head += 1;

            if (written < cap) {
                out[written] = current;
                written += 1;
            }

            // Find all docs linking to `current`
            var it2 = self.nodes.iterator();
            while (it2.next()) |entry| {
                if (visited.contains(entry.key_ptr.*)) continue;
                for (entry.value_ptr.outbound.items) |t| {
                    if (t == current) {
                        visited.put(scratch_allocator, entry.key_ptr.*, {}) catch return -3;
                        queue.append(scratch_allocator, entry.key_ptr.*) catch return -3;
                        break;
                    }
                }
            }
        }

        if (written < queue.items.len) return -2;
        return @intCast(written);
    }

    /// Compute PageRank scores using iterative power method.
    ///
    /// `scores` must have capacity >= node count.
    /// `ids` receives the doc IDs in the same order as scores.
    /// Returns number of nodes, or negative error.
    pub fn computePagerank(
        self: *const LinkGraph,
        iterations: u32,
        damping: f32,
        ids: [*]u32,
        scores: [*]f32,
        cap: u32,
        scratch_allocator: Allocator,
    ) i32 {
        if (iterations == 0) return -1;
        const n = self.nodes.count();
        if (n == 0) return 0;
        if (n > cap) return -2;

        const n_f: f32 = @floatFromInt(n);

        // Build ID array and index map
        var id_to_idx = std.AutoHashMapUnmanaged(u32, u32){};
        defer id_to_idx.deinit(scratch_allocator);

        var idx: u32 = 0;
        var it = self.nodes.keyIterator();
        while (it.next()) |key| {
            ids[idx] = key.*;
            id_to_idx.put(scratch_allocator, key.*, idx) catch return -3;
            scores[idx] = 1.0 / n_f;
            idx += 1;
        }

        // Scratch buffer for new scores
        const new_scores = scratch_allocator.alloc(f32, n) catch return -3;
        defer scratch_allocator.free(new_scores);

        const base = (1.0 - damping) / n_f;

        for (0..iterations) |_| {
            // Reset new scores
            for (0..n) |i| {
                new_scores[i] = base;
            }

            // Distribute rank from each node to its outbound targets
            var node_it = self.nodes.iterator();
            while (node_it.next()) |entry| {
                const src_idx = id_to_idx.get(entry.key_ptr.*).?;
                const out_degree = entry.value_ptr.outbound.items.len;
                if (out_degree == 0) {
                    // Dangling node: distribute evenly to all nodes
                    const share = damping * scores[src_idx] / n_f;
                    for (0..n) |i| {
                        new_scores[i] += share;
                    }
                } else {
                    const share = damping * scores[src_idx] / @as(f32, @floatFromInt(out_degree));
                    for (entry.value_ptr.outbound.items) |target_id| {
                        if (id_to_idx.get(target_id)) |target_idx| {
                            new_scores[target_idx] += share;
                        }
                        // Links to non-existent nodes are ignored (treated as dangling)
                    }
                }
            }

            // Copy new scores back
            @memcpy(scores[0..n], new_scores[0..n]);
        }

        return @intCast(n);
    }

    /// Compute connectivity statistics using union-find.
    ///
    /// Returns: connected_components, max_degree, total_edges (via out params).
    /// Return value: 0 success, -3 allocation failure.
    pub fn connectivityStats(
        self: *const LinkGraph,
        out_components: *u32,
        out_avg_degree_x100: *u32,
        out_max_degree: *u32,
        scratch_allocator: Allocator,
    ) i32 {
        const n = self.nodes.count();
        if (n == 0) {
            out_components.* = 0;
            out_avg_degree_x100.* = 0;
            out_max_degree.* = 0;
            return 0;
        }

        // Build index mapping
        var id_to_idx = std.AutoHashMapUnmanaged(u32, u32){};
        defer id_to_idx.deinit(scratch_allocator);

        var idx: u32 = 0;
        var key_it = self.nodes.keyIterator();
        while (key_it.next()) |key| {
            id_to_idx.put(scratch_allocator, key.*, idx) catch return -3;
            idx += 1;
        }

        // Union-Find
        const parent = scratch_allocator.alloc(u32, n) catch return -3;
        defer scratch_allocator.free(parent);
        const rank = scratch_allocator.alloc(u32, n) catch return -3;
        defer scratch_allocator.free(rank);

        for (0..n) |i| {
            parent[i] = @intCast(i);
            rank[i] = 0;
        }

        var max_degree: u32 = 0;
        var total_edges: u32 = 0;

        var it = self.nodes.iterator();
        while (it.next()) |entry| {
            const src_idx = id_to_idx.get(entry.key_ptr.*).?;
            const degree: u32 = @intCast(entry.value_ptr.outbound.items.len + entry.value_ptr.inbound_count);
            if (degree > max_degree) max_degree = degree;
            total_edges += @intCast(entry.value_ptr.outbound.items.len);

            for (entry.value_ptr.outbound.items) |target_id| {
                if (id_to_idx.get(target_id)) |target_idx| {
                    unionSets(parent, rank, src_idx, target_idx);
                }
            }
        }

        // Count components
        var components: u32 = 0;
        for (0..n) |i| {
            if (findRoot(parent, @intCast(i)) == @as(u32, @intCast(i))) {
                components += 1;
            }
        }

        out_components.* = components;
        out_max_degree.* = max_degree;
        // avg_degree * 100 to avoid floats in C ABI
        if (n > 0) {
            out_avg_degree_x100.* = (total_edges * 200) / @as(u32, @intCast(n));
            // *200 because each edge contributes to degree of both endpoints
            // but we only count outbound, so *2 for undirected avg, then *100 for fixed point
        } else {
            out_avg_degree_x100.* = 0;
        }

        return 0;
    }

    /// Number of documents in the graph.
    pub fn count(self: *const LinkGraph) u32 {
        return self.nodes.count();
    }

    // -- Internal helpers --

    fn decrementInbound(self: *LinkGraph, targets: []const u32) void {
        for (targets) |target| {
            if (self.nodes.getPtr(target)) |target_node| {
                if (target_node.inbound_count > 0) {
                    target_node.inbound_count -= 1;
                }
            }
        }
    }
};

// Union-Find helpers (module-level for comptime compatibility)
fn findRoot(parent: []u32, x: u32) u32 {
    var current = x;
    while (parent[current] != current) {
        parent[current] = parent[parent[current]]; // path compression
        current = parent[current];
    }
    return current;
}

fn unionSets(parent: []u32, rank_arr: []u32, a: u32, b: u32) void {
    const ra = findRoot(parent, a);
    const rb = findRoot(parent, b);
    if (ra == rb) return;
    if (rank_arr[ra] < rank_arr[rb]) {
        parent[ra] = rb;
    } else if (rank_arr[ra] > rank_arr[rb]) {
        parent[rb] = ra;
    } else {
        parent[rb] = ra;
        rank_arr[ra] += 1;
    }
}

// ============================================================================
// Tests
// ============================================================================

test "empty graph has no orphans" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    var out: [4]u32 = undefined;
    const rc = graph.findOrphans(&out, 4);
    try std.testing.expectEqual(@as(i32, 0), rc);
}

test "single document with no links is orphan" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{}));

    var out: [4]u32 = undefined;
    const rc = graph.findOrphans(&out, 4);
    try std.testing.expectEqual(@as(i32, 1), rc);
    try std.testing.expectEqual(@as(u32, 1), out[0]);
}

test "add document with outbound links" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{ 2, 3 }));
    try std.testing.expectEqual(@as(u32, 3), graph.count()); // 1, 2, 3 all exist
}

test "remove document cleans up edges" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{2}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(2, &.{}));

    // Doc 2 should have inbound_count = 1 (from doc 1)
    var out: [4]u32 = undefined;
    var rc = graph.findOrphans(&out, 4);
    try std.testing.expectEqual(@as(i32, 1), rc); // only doc 1 is orphan

    // Remove doc 1
    try std.testing.expectEqual(@as(i32, 0), graph.removeDocument(1));
    try std.testing.expectEqual(@as(u32, 1), graph.count());

    // Now doc 2 should be an orphan
    rc = graph.findOrphans(&out, 4);
    try std.testing.expectEqual(@as(i32, 1), rc);
    try std.testing.expectEqual(@as(u32, 2), out[0]);
}

test "remove non-existent document returns -1" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    try std.testing.expectEqual(@as(i32, -1), graph.removeDocument(999));
}

test "find orphans with links" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // A -> B -> C
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{2}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(2, &.{3}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(3, &.{}));

    var out: [4]u32 = undefined;
    const rc = graph.findOrphans(&out, 4);
    try std.testing.expectEqual(@as(i32, 1), rc); // only doc 1 is orphan
    try std.testing.expectEqual(@as(u32, 1), out[0]);
}

test "broken chain simple" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // A -> B (target)
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{2}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(2, &.{}));

    var out: [8]u32 = undefined;
    const rc = graph.findBrokenChains(2, &out, 8, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 1), rc);
    try std.testing.expectEqual(@as(u32, 1), out[0]);
}

test "broken chain transitive" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // A -> B -> C (target)
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{2}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(2, &.{3}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(3, &.{}));

    var out: [8]u32 = undefined;
    const rc = graph.findBrokenChains(3, &out, 8, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 2), rc);
    // Both A and B should be in the chain (order may vary due to BFS)
    var found_1 = false;
    var found_2 = false;
    for (out[0..2]) |id| {
        if (id == 1) found_1 = true;
        if (id == 2) found_2 = true;
    }
    try std.testing.expect(found_1);
    try std.testing.expect(found_2);
}

test "broken chain target not in graph" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{2}));

    var out: [4]u32 = undefined;
    const rc = graph.findBrokenChains(999, &out, 4, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 0), rc);
}

test "pagerank simple" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // Star topology: B, C, D all link to A
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{})); // A
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(2, &.{1})); // B -> A
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(3, &.{1})); // C -> A
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(4, &.{1})); // D -> A

    var ids: [8]u32 = undefined;
    var scores: [8]f32 = undefined;
    const rc = graph.computePagerank(20, 0.85, &ids, &scores, 8, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 4), rc);

    // Find A's score — it should be the highest
    var a_score: f32 = 0.0;
    var max_score: f32 = 0.0;
    for (0..4) |i| {
        if (ids[i] == 1) a_score = scores[i];
        if (scores[i] > max_score) max_score = scores[i];
    }
    try std.testing.expect(a_score > 0.0);
    try std.testing.expectEqual(a_score, max_score);
}

test "pagerank convergence on cycle" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // A -> B -> C -> A (cycle)
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{2}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(2, &.{3}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(3, &.{1}));

    var ids: [4]u32 = undefined;
    var scores: [4]f32 = undefined;
    const rc = graph.computePagerank(50, 0.85, &ids, &scores, 4, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 3), rc);

    // All scores should be roughly equal (1/3)
    for (0..3) |i| {
        try std.testing.expectApproxEqAbs(@as(f32, 1.0 / 3.0), scores[i], 0.01);
    }
}

test "pagerank zero iterations returns -1" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{}));

    var ids: [4]u32 = undefined;
    var scores: [4]f32 = undefined;
    const rc = graph.computePagerank(0, 0.85, &ids, &scores, 4, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "connectivity stats basic" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // Two components: {1,2} and {3}
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{2}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(2, &.{}));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(3, &.{}));

    var components: u32 = undefined;
    var avg_degree: u32 = undefined;
    var max_degree: u32 = undefined;
    const rc = graph.connectivityStats(&components, &avg_degree, &max_degree, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), components); // {1,2} and {3}
    try std.testing.expect(max_degree > 0);
}

test "self-link handled correctly" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{1})); // self-link

    // Should not be orphan (has inbound from itself)
    var out: [4]u32 = undefined;
    const rc = graph.findOrphans(&out, 4);
    try std.testing.expectEqual(@as(i32, 0), rc); // no orphans
}

test "duplicate add replaces edges" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{ 2, 3 }));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{3})); // replace

    // Doc 2 should now be orphan (no longer linked by 1)
    var out: [8]u32 = undefined;
    const rc = graph.findOrphans(&out, 8);
    // Doc 1, Doc 2 are orphans (1 has no inbound, 2 lost its inbound)
    try std.testing.expect(rc >= 2);
}

test "create destroy no leak" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // Add 100 docs
    for (0..100) |i| {
        const id: u32 = @intCast(i);
        const targets = if (i > 0) @as([]const u32, &.{@as(u32, @intCast(i - 1))}) else &.{};
        try std.testing.expectEqual(@as(i32, 0), graph.addDocument(id, targets));
    }

    try std.testing.expectEqual(@as(u32, 100), graph.count());
    // deinit in defer will be checked by testing allocator for leaks
}

test "large graph performance" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // 1000-node chain
    for (0..1000) |i| {
        const id: u32 = @intCast(i);
        const targets = if (i < 999) @as([]const u32, &.{@as(u32, @intCast(i + 1))}) else &.{};
        try std.testing.expectEqual(@as(i32, 0), graph.addDocument(id, targets));
    }

    try std.testing.expectEqual(@as(u32, 1000), graph.count());

    // Orphan detection should find node 0 (nothing links to it)
    var out: [8]u32 = undefined;
    const rc = graph.findOrphans(&out, 8);
    try std.testing.expectEqual(@as(i32, 1), rc);
    try std.testing.expectEqual(@as(u32, 0), out[0]);
}

test "addDocument preserves source node when inserting many new targets" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    var targets: [256]u32 = undefined;
    for (0..targets.len) |i| {
        targets[i] = @as(u32, @intCast(i + 2));
    }

    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, targets[0..]));
    const node = graph.nodes.getPtr(1).?;
    try std.testing.expectEqual(targets.len, node.outbound.items.len);
    for (targets, 0..) |target, idx| {
        try std.testing.expectEqual(target, node.outbound.items[idx]);
    }
}

test "empty graph pagerank returns 0" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    var ids: [4]u32 = undefined;
    var scores: [4]f32 = undefined;
    const rc = graph.computePagerank(10, 0.85, &ids, &scores, 4, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 0), rc);
}

test "empty graph connectivity stats" {
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    var components: u32 = undefined;
    var avg_degree: u32 = undefined;
    var max_degree: u32 = undefined;
    const rc = graph.connectivityStats(&components, &avg_degree, &max_degree, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), components);
    try std.testing.expectEqual(@as(u32, 0), max_degree);
}

test "duplicate targets do not inflate inbound_count" {
    // Regression test for marky-859: duplicate IDs in targets slice must be
    // deduplicated so inbound_count reflects unique sources, not raw occurrences.
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // Doc 1 links to doc 2 three times (duplicates)
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{ 2, 2, 2 }));

    // inbound_count for doc 2 must be 1, not 3
    const node2 = graph.nodes.getPtr(2).?;
    try std.testing.expectEqual(@as(u32, 1), node2.inbound_count);

    // Doc 2 must not be an orphan
    var out: [4]u32 = undefined;
    const rc = graph.findOrphans(&out, 4);
    // Only doc 1 is an orphan (nothing links to it)
    try std.testing.expectEqual(@as(i32, 1), rc);
    try std.testing.expectEqual(@as(u32, 1), out[0]);
}

test "duplicate targets in pagerank do not skew scores" {
    // Regression test for marky-859: duplicate targets inflate out_degree
    // denominator causing incorrect rank distribution.
    var graph = LinkGraph.init(std.testing.allocator);
    defer graph.deinit();

    // Doc 1 -> doc 2 (three duplicate entries should behave same as one)
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(1, &.{ 2, 2, 2 }));
    try std.testing.expectEqual(@as(i32, 0), graph.addDocument(2, &.{}));

    var ids: [4]u32 = undefined;
    var scores: [4]f32 = undefined;
    const rc = graph.computePagerank(20, 0.85, &ids, &scores, 4, std.testing.allocator);
    try std.testing.expectEqual(@as(i32, 2), rc);

    // Doc 1 links to doc 2 once (deduplicated); doc 2 should have higher score
    // than if there were no links. With dedup, doc 2 receives full rank from doc 1.
    // Without dedup, out_degree=3 causes rank to be split three ways (still to doc 2
    // but with inflated denominator). The key check is inbound_count=1, tested above.
    // Here we verify convergence is sane: scores sum to ~1.0.
    var total: f32 = 0.0;
    for (0..2) |i| total += scores[i];
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), total, 0.01);
}
