---
id: marky-0u5
title: Write Rust FFI wrappers for shared kernels
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ncz, marky-bgl]
---






In markymark-kernels/src/, implement safe Rust wrappers: embed.rs (EmbeddingIndex struct wrapping opaque handle, create/add/search/destroy with Drop impl), similarity.rs (cosine_similarity, jaccard_similarity functions), hash.rs (extract_entity_hashes). All wrappers must: validate inputs, handle error codes, manage buffer allocation. Write Rust tests for each wrapper. Verify: cargo test -p markymark-kernels.

## Design

## Goal
Implement safe Rust FFI wrappers in markymark-kernels/src/ for all shared kernels: embed.rs (EmbeddingIndex struct with Drop), similarity.rs (cosine_similarity, jaccard_similarity), hash.rs (extract_entity_hashes). All wrappers validate inputs, handle error codes, manage buffer allocation, and present a safe Rust API.

## Effort Estimate
6-8 hours

## Success Criteria
- [ ] embed.rs: EmbeddingIndex::new(dims), add(), search(), count(), drop() all work correctly
- [ ] EmbeddingIndex implements Drop to prevent memory leaks
- [ ] similarity.rs: cosine_similarity() and jaccard_similarity() return Result<f32, KernelError>
- [ ] hash.rs: extract_entity_hashes() returns Result<Vec<u64>, KernelError>
- [ ] All FFI error codes mapped to typed Rust errors (KernelError enum)
- [ ] cargo test -p markymark-kernels passes with FFI integration tests
- [ ] cargo clippy -p markymark-kernels -- -D warnings is clean
- [ ] No unsafe outside clearly documented unsafe blocks with SAFETY comments

## Implementation Checklist
- [ ] Define KernelError enum in lib.rs: InvalidInput, BufferTooSmall, InternalError
- [ ] Implement embed.rs: EmbeddingIndex with opaque pointer handle
- [ ] Implement Drop for EmbeddingIndex (calls zig_embedding_index_destroy)
- [ ] Implement similarity.rs: cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32>
- [ ] Implement similarity.rs: jaccard_similarity(set1: &[u64], set2: &[u64]) -> Result<f32>
- [ ] Implement hash.rs: extract_entity_hashes(text: &str) -> Result<Vec<u64>>
- [ ] Add extern "C" declarations in a private ffi module (lib.rs or ffi.rs)
- [ ] Write integration test: EmbeddingIndex lifecycle (create, add 10, search, drop)
- [ ] Write integration test: cosine_similarity with known vectors
- [ ] Write integration test: entity hash extraction on known text

## Edge Cases
- Empty text for entity hashes: should return Ok(empty vec), not error
- Zero-length vectors for cosine: should return Err(InvalidInput)
- EmbeddingIndex double-drop: Drop impl must be idempotent (set handle to null after destroy)
- Very large entity text: wrapper must allocate sufficient initial buffer, retry if -2
- Search with top_k=0: should return Ok(empty vec)
- Search with top_k > count: should return all entries (not error)
- Thread safety: EmbeddingIndex is NOT Send/Sync (opaque Zig pointer). Document this.

## Anti-patterns
- NO unwrap/expect in wrapper code (use Result everywhere)
- NO exposing raw pointers in public API (wrap in safe abstractions)
- NO missing SAFETY comments on unsafe blocks
- NO assuming Zig output buffers are UTF-8 (entity IDs are byte arrays)
- NO Clone derive on EmbeddingIndex (would double-free the opaque handle)
- NO Send/Sync impl on EmbeddingIndex without verifying Zig thread safety

## Error Handling
- FFI returns -1: map to KernelError::InvalidInput with context string
- FFI returns -2: retry with larger buffer (double size, max 3 retries), then KernelError::BufferTooSmall
- FFI returns -3: map to KernelError::InternalError
- Null handle from create: return Err(KernelError::InternalError)
- Buffer allocation failure: let Rust's OOM handler deal with it (don't catch)

## Test Specifications (what bug does each test catch?)
- test_embedding_index_lifecycle: catches use-after-free or double-free in Drop
- test_embedding_index_search_empty: catches null deref when searching empty index
- test_embedding_index_add_and_search: catches incorrect result mapping from C structs to Rust types
- test_cosine_known_vectors: catches incorrect float conversion or argument ordering in FFI call
- test_cosine_empty_vectors: catches missing input validation (should error, not crash)
- test_jaccard_known_sets: catches incorrect pointer/length passing across FFI boundary
- test_entity_hashes_known_text: catches string encoding issues (Rust &str to *const u8 + len)
- test_entity_hashes_empty_text: catches special case handling for zero-length input
- test_error_code_mapping: catches wrong error variant for each FFI error code
- test_buffer_retry_on_overflow: catches infinite loop or missing retry logic when buffer is too small
