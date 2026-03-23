---
id: marky-ncz
title: Port embedding index kernels from forge BRZA
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv, marky-een]
---




Fork zig/src/embeddings.zig from forge BRZA into zig/src/shared/embeddings.zig. Includes: create/destroy index, add embedding, top-K cosine search, count, dimensions. Also port the abi.zig layer. Add C ABI exports to c_adapter.zig: zig_embedding_index_create, _destroy, _add, _search, _count, _dimensions. Write Zig unit tests verifying: create index, add 100 embeddings, search returns correct top-K, destroy frees memory. See brza-spec.md Section 3.3 for API surface.

## Design

## Goal
Fork zig/src/embeddings.zig from forge BRZA into zig/src/shared/embeddings.zig. Port the embedding index: create/destroy index, add embedding, top-K cosine search, count, dimensions. Also port abi.zig layer. Add C ABI exports to c_adapter.zig. Write Zig unit tests verifying the full lifecycle.

## Effort Estimate
6-8 hours

## Success Criteria
- [ ] zig/src/shared/embeddings.zig compiles and passes all unit tests
- [ ] zig/src/shared/abi.zig provides the memory management abstractions
- [ ] C ABI exports present: zig_embedding_index_create, _destroy, _add, _search, _count, _dimensions
- [ ] Unit tests pass: cd zig && zig build test (embedding-specific tests)
- [ ] top-K search returns correct results for 100+ embeddings with known cosine distances
- [ ] Index handles dimensions 128, 384, 768, 1536 (common embedding model dims)
- [ ] Memory: create followed by destroy does not leak (tested via Zig's testing allocator)

## Implementation Checklist
- [ ] Port zig/src/shared/embeddings.zig from forge BRZA (update for 0.15.2 API changes)
- [ ] Port zig/src/shared/abi.zig from forge BRZA
- [ ] Add C ABI exports to zig/src/c_adapter.zig for all 6 embedding functions
- [ ] Write test: create index with dims=384, verify count=0
- [ ] Write test: add 100 random embeddings, verify count=100
- [ ] Write test: search with known query, verify top-K ordering by cosine distance
- [ ] Write test: add duplicate IDs, verify replacement (or error)
- [ ] Write test: create/destroy cycle with no leaks (testing allocator)
- [ ] Write test: search on empty index returns 0 results
- [ ] Update build.zig to compile embeddings.zig and abi.zig into the library

## Edge Cases
- Empty index: search should return 0 results, count=0
- Zero dimensions: create should return null/error (dims must be > 0)
- Null pointer for embedding data: add should return -1
- Large index (10K+ entries): search should still return correctly (may be slow)
- Duplicate IDs: decide on semantics (replace vs reject) and test accordingly
- Query dimensions mismatch: search with wrong-dim query should return -1
- Buffer too small for results: search should return -2 and write up to cap results

## Anti-patterns
- NO heap allocation for caller (Rust allocates result buffers)
- NO Zig 0.14.x allocator patterns (use 0.15.2 std.mem.Allocator interface)
- NO storing Zig-allocated strings across FFI boundary (IDs should be copied internally)
- NO batch Jaccard optimization without benchmarking (FAILED in forge at 1000+ episodes)
- NO ignoring the forge implementation's test patterns (they caught real bugs)

## Error Handling
- Null pointer (index handle): return -1
- Zero dimensions: return -1 from create (null handle)
- Add with null embedding ptr: return -1
- Search with null query ptr: return -1
- Search buffer too small: return -2, write partial results up to cap
- Internal allocation failure: return -3

## Test Specifications (what bug does each test catch?)
- test_create_empty_index: catches uninitialized state bugs (count should be 0, not garbage)
- test_add_and_count: catches off-by-one in insertion counter
- test_search_top_k_ordering: catches incorrect cosine similarity computation or sorting bugs
- test_search_empty_index: catches null deref when index has no entries
- test_create_destroy_no_leak: catches memory leak in index teardown (arena not freed)
- test_zero_dimensions: catches divide-by-zero in cosine normalization
- test_null_embedding_ptr: catches missing null guard on add path
- test_search_buffer_overflow: catches writing past cap boundary in results array
- test_dimensions_mismatch: catches silent corruption when query dim != index dim
