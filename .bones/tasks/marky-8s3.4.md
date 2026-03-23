---
id: marky-8s3.4
title: Implement Zig link graph engine
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3
---



Create zig/src/kernels/link_graph.zig. Graph data structure for document link network. Adjacency list with SIMD-accelerated traversal. Operations: add_document(id, outbound_links[]), remove_document(id), find_orphans() -> docs with zero inbound links, find_broken_chains(broken_target) -> all docs transitively linking to broken target, compute_pagerank(iterations, damping) -> importance scores, connectivity_stats() -> {connected_components, avg_degree, max_degree}. Handle lifecycle: create/destroy graph. Export C ABI functions. Tests: small graph (5 docs), medium (100 docs), orphan detection, broken chain propagation, pagerank convergence. Builds on forge petgraph patterns but Zig-native for SIMD.

## Design

## Goal
Create zig/src/kernels/link_graph.zig implementing a graph data structure for the document link network. Supports add/remove documents, orphan detection, broken chain analysis, PageRank computation, and connectivity statistics. Uses adjacency list with SIMD-accelerated traversal for graph operations.

## Effort Estimate
14-16 hours

## Success Criteria
- [ ] link_graph.zig compiles with C ABI exports for create, destroy, add_document, remove_document, find_orphans, find_broken_chains, compute_pagerank, connectivity_stats
- [ ] Adjacency list representation with O(1) document lookup by ID
- [ ] Orphan detection correctly finds docs with zero inbound links
- [ ] Broken chain analysis correctly finds all docs transitively linking to a broken target
- [ ] PageRank converges within specified iterations (verified against reference)
- [ ] Connectivity stats: connected components, avg degree, max degree all correct
- [ ] Memory: create + add N docs + destroy has zero leaks (testing allocator)
- [ ] cd zig && zig build test passes

## Implementation Checklist
- [ ] Create zig/src/kernels/link_graph.zig
- [ ] Define graph handle type (opaque pointer for FFI)
- [ ] Implement adjacency list with document ID -> outbound edges mapping
- [ ] Also maintain reverse adjacency (inbound edges) for orphan detection
- [ ] Implement create/destroy lifecycle with arena allocator
- [ ] Implement add_document(id, outbound_links[]): add node and edges
- [ ] Implement remove_document(id): remove node, clean up edges in both directions
- [ ] Implement find_orphans: scan for nodes with zero inbound count
- [ ] Implement find_broken_chains(target): BFS/DFS reverse traversal from target
- [ ] Implement compute_pagerank(iterations, damping): iterative PageRank with SIMD dot products
- [ ] Implement connectivity_stats: union-find for connected components, degree counting
- [ ] Add C ABI exports to c_adapter.zig
- [ ] Write tests per specification
- [ ] Update build.zig

## Edge Cases
- Empty graph: orphans returns 0, stats has 0 components, PageRank returns empty
- Single document with no links: is an orphan (zero inbound from others)
- Self-link: document links to itself — handle in degree counting
- Circular links: A -> B -> C -> A — PageRank must converge, not diverge
- Broken target not in graph: find_broken_chains should handle gracefully (return 0)
- Remove document that doesn't exist: return error code, don't crash
- Very large graph (10K+ nodes): must handle without stack overflow (use iterative, not recursive)
- Duplicate add_document with same ID: update edges, not duplicate node
- PageRank with damping=0 or damping=1: edge cases in convergence

## Anti-patterns
- NO recursive DFS for large graphs (stack overflow risk — use iterative with explicit stack)
- NO O(V*E) for orphan detection (maintain inbound degree counter, O(V) scan)
- NO heap allocation per query (allocate workspace once, reuse)
- NO assuming document IDs are contiguous integers (use hash map for ID lookup)
- NO ignoring memory cleanup in destroy (must free all internal allocations)

## Error Handling
- Null handle: return -1 for all operations
- Add with null ID: return -1
- Remove non-existent ID: return -1
- Buffer too small for orphan/broken results: return -2, write as many as fit
- PageRank with 0 iterations: return -1
- Internal allocation failure: return -3

## Test Specifications (what bug does each test catch?)
- test_empty_graph: catches null deref on operations with no documents
- test_add_single_document: catches basic add and lookup failure
- test_add_with_outbound_links: catches edge creation failure
- test_remove_document: catches dangling edge references after removal
- test_find_orphans_simple: catches incorrect inbound degree tracking
- test_find_orphans_with_links: catches false positive orphan when doc has inbound links
- test_broken_chain_simple: catches incorrect reverse traversal
- test_broken_chain_transitive: catches stopping at first hop instead of full transitive closure
- test_pagerank_simple: catches incorrect PageRank computation (compare to known values)
- test_pagerank_convergence: catches divergence on circular graphs
- test_connectivity_components: catches incorrect union-find for connected components
- test_self_link: catches infinite loop or double-counting in self-referencing document
- test_large_graph: catches O(n^2) performance regression on 1000+ node graph
- test_create_destroy_no_leak: catches memory leak via Zig testing allocator
- test_duplicate_add: catches node duplication instead of edge update
