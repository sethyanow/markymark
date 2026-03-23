---
id: marky-59i
title: Implement token_estimate and content_hash Zig kernels
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
---



Create zig/src/kernels/token_estimate.zig: approximate BPE token count using SIMD word boundary detection and average tokens-per-word multiplier (~1.3 for English). Create zig/src/kernels/content_hash.zig: FNV-1a 64-bit hash of text content. Export as marky_estimate_tokens and marky_content_hash. Tests: known token counts for sample texts, hash determinism, hash collision resistance on similar texts.

## Design

## Goal
Create zig/src/kernels/token_estimate.zig (approximate BPE token count using SIMD word boundary detection) and zig/src/kernels/content_hash.zig (FNV-1a 64-bit content fingerprint). These are simpler utility kernels — one returns a count, the other returns a hash.

## Effort Estimate
4-6 hours

## Success Criteria
- [ ] token_estimate.zig exports marky_estimate_tokens, returns approximate BPE count
- [ ] content_hash.zig exports marky_content_hash, returns deterministic u64 hash
- [ ] Token estimate within 20% of actual tiktoken count on English prose
- [ ] Content hash is deterministic: same input always produces same hash
- [ ] Content hash has good distribution: different inputs produce different hashes (tested on 1000 samples)
- [ ] cd zig && zig build test passes
- [ ] FNV-1a implementation matches reference (known test vectors from spec)

## Implementation Checklist
- [ ] Create zig/src/kernels/token_estimate.zig
- [ ] SIMD word boundary detection: count spaces, punctuation, newlines
- [ ] Apply tokens-per-word multiplier (~1.3 for English text)
- [ ] Handle edge cases: all-whitespace, all-punctuation, code (higher token density)
- [ ] Create zig/src/kernels/content_hash.zig
- [ ] Implement FNV-1a 64-bit (offset basis: 14695981039346656037, prime: 1099511628211)
- [ ] SIMD opportunity: process 8 bytes at a time with SIMD multiply/XOR
- [ ] Add C ABI exports to c_adapter.zig
- [ ] Write tests per specification below
- [ ] Update build.zig

## Edge Cases
- Empty input (len=0): token_estimate returns 0, content_hash returns FNV offset basis (hash of empty string)
- All whitespace: token_estimate returns 0 (no tokens)
- Single character: token_estimate returns 1
- Very long input (>1MB): both functions should handle without issue (streaming/incremental)
- Non-ASCII UTF-8: token estimate may be less accurate (BPE varies for non-English)
- Binary content: content_hash works on any bytes; token_estimate may overcount
- Identical content: content_hash must return identical values (no randomness)

## Anti-patterns
- NO using random seeds in content_hash (must be deterministic)
- NO counting only spaces for token estimation (punctuation and newlines also create boundaries)
- NO FNV-1a implementation that differs from the spec (check against known test vectors)
- NO returning 0 for content_hash on non-empty input (that would collide with empty)
- NO heap allocation in either kernel

## Error Handling
- Null text pointer: token_estimate returns 0, content_hash returns 0 (or FNV basis, document choice)
- Zero length: token_estimate returns 0, content_hash returns FNV offset basis

## Test Specifications (what bug does each test catch?)
- test_token_estimate_empty: catches divide-by-zero or null deref on empty input
- test_token_estimate_single_word: catches off-by-one in word counting
- test_token_estimate_english_prose: catches grossly inaccurate multiplier (>20% off tiktoken)
- test_token_estimate_code: catches assumption that code has same density as prose
- test_token_estimate_all_whitespace: catches counting whitespace as tokens
- test_content_hash_empty: catches uninitialized hash state (should be FNV offset basis)
- test_content_hash_deterministic: catches non-determinism (random seed, timestamp, etc.)
- test_content_hash_known_vectors: catches FNV-1a algorithm bugs (compare to spec test vectors)
- test_content_hash_distinct_inputs: catches trivial hash function that returns same value
- test_content_hash_avalanche: catches poor distribution (single bit flip should change ~50% of bits)
- test_token_estimate_large_input: catches performance regression or overflow on >1MB input
