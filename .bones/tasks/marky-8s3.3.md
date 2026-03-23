---
id: marky-8s3.3
title: Implement Zig SIMD fuzzy matcher for search-symbols
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv]
parent: marky-8s3
---




Create zig/src/kernels/fuzzy_match.zig. SIMD-accelerated fuzzy string matching for heading search (search-symbols MCP tool). Algorithm: fzf-style scoring with bonus for word boundaries, consecutive matches, and case-sensitive exact prefix. Input: query string + array of candidate strings (heading texts). Output: scored results sorted by relevance. Export as marky_fuzzy_match(query, query_len, candidates, candidate_lens, count, scores_out, indices_out, top_k) -> i32. Must handle 100K+ candidates in <10ms. Tests: exact match, prefix match, subsequence match, case insensitivity, empty query, unicode (UTF-8 byte-level matching). Rust FFI wrapper integrates with existing search_symbols in markymark-mcp.

## Design

## Goal
Create zig/src/kernels/fuzzy_match.zig implementing SIMD-accelerated fuzzy string matching for heading search (search-symbols MCP tool). Uses fzf-style scoring with bonuses for word boundaries, consecutive matches, and case-sensitive exact prefix. Must handle 100K+ candidates in <10ms.

## Effort Estimate
10-12 hours

## Success Criteria
- [ ] fuzzy_match.zig compiles and exports marky_fuzzy_match via c_adapter.zig
- [ ] fzf-compatible scoring: word boundary bonus, consecutive match bonus, exact prefix bonus
- [ ] Case-insensitive matching with case-sensitive scoring bonus
- [ ] Handles 100K+ candidates in <10ms (benchmarked)
- [ ] Returns top-K results sorted by score descending
- [ ] Correct UTF-8 byte-level matching (non-ASCII passes through)
- [ ] cd zig && zig build test passes
- [ ] Scalar reference implementation for correctness verification

## Implementation Checklist
- [ ] Create zig/src/kernels/fuzzy_match.zig
- [ ] Create zig/src/reference/fuzzy_match_ref.zig (scalar scoring reference)
- [ ] Implement subsequence matching: each query char must appear in order in candidate
- [ ] Implement scoring: +1 per match, +bonus for word boundary, +bonus for consecutive, +bonus for prefix
- [ ] SIMD acceleration: vectorized byte comparison for initial filtering (skip non-matching candidates)
- [ ] Top-K selection: partial sort or min-heap for returning top K results
- [ ] Handle varying candidate lengths via pointer array + length array
- [ ] Add C ABI export to c_adapter.zig
- [ ] Write unit tests
- [ ] Write benchmark: 100K candidates, various query lengths
- [ ] Update build.zig

## Edge Cases
- Empty query: return all candidates with score 0 (or return top-K by insertion order)
- Empty candidate list (count=0): return 0 results
- Query longer than candidate: candidate cannot match, skip
- All candidates match: top-K selection must work correctly
- No candidates match: return 0 results
- Unicode query: match byte-by-byte (UTF-8 compatible)
- Very long candidate (>65535 bytes): handle without overflow
- top_k=0: return 0 results
- top_k > count: return all matching candidates
- Candidate with null bytes: use length parameter, not null termination
- Tied scores: stable ordering by original index

## Anti-patterns
- NO O(n * m * k) algorithm where n=candidates, m=query_len, k=candidate_len (must be efficient)
- NO heap allocation per candidate (pre-allocate workspace)
- NO sorting all candidates when only top-K needed (use partial sort or heap)
- NO assuming ASCII-only input (UTF-8 byte sequences must not be corrupted)
- NO string copying for comparison (work on pointer+length directly)
- NO ignoring the scoring model (bare subsequence matching without scoring is useless)

## Error Handling
- Null query pointer: return -1
- Zero query length: return 0 written (all match, but no scoring basis)
- Null candidates pointer: return -1
- Null output pointers (scores_out, indices_out): return -1
- Buffer too small for top_k: return -2
- Internal error: return -3

## Test Specifications (what bug does each test catch?)
- test_empty_query: catches null deref or incorrect behavior on empty query
- test_exact_match: catches scoring not giving maximum score to exact matches
- test_prefix_match: catches missing prefix bonus in scoring
- test_subsequence_match: catches incorrect subsequence detection (chars out of order)
- test_no_match: catches false positive when query chars are not present
- test_case_insensitivity: catches case-sensitive rejection of valid case-insensitive match
- test_word_boundary_bonus: catches missing bonus for matches at word boundaries (camelCase, snake_case)
- test_consecutive_bonus: catches missing bonus for consecutive character matches
- test_top_k_selection: catches returning wrong top-K (incorrect sorting or heap management)
- test_100k_candidates_performance: catches O(n^2) algorithm that fails performance target
- test_unicode_query: catches UTF-8 byte corruption in matching
- test_tied_scores_stable: catches non-deterministic ordering of equally-scored candidates
- test_query_longer_than_candidate: catches out-of-bounds read on short candidates
