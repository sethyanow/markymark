---
id: marky-8xt
title: 'marky-8s3.3 follow-up: SIMD/top-k fuzzy matcher API parity'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8s3.3]
parent: marky-8s3
---



Follow-up for marky-8s3.3: current implementation integrates fuzzy scoring through per-candidate API (query+candidate -> score) and runtime sorting in Rust. Original task design expected array-based candidate API with top-k selection and explicit 100K<10ms benchmark target. Create dedicated task to implement SIMD prefilter/array API/top-k path and benchmark evidence against the original acceptance criteria.

## Design

## Goal
Implement the remaining scope from marky-8s3.3 by adding a batched fuzzy-matching API with top-k selection, deterministic ranking, and benchmark evidence suitable for high-candidate search-symbols workloads.

## Effort Estimate
8-12 hours

## Success Criteria
- [ ] New batched C ABI function is exported from `zig/src/c_adapter.zig` with explicit pointer/length/top-k parameters and documented return codes.
- [ ] Scalar reference path and kernel path produce identical ranked results for the same fixture inputs (verified by tests).
- [ ] Top-k output is deterministic: sort by score descending, then candidate index ascending on ties.
- [ ] Benchmark for 100K candidates is implemented and result is recorded in bead notes with command, dataset description, and measured runtime.
- [ ] Existing per-candidate `marky_fuzzy_match` API remains compatible and current `search_symbols_` tests stay green.
- [ ] Verification commands succeed:
  - `bash -lc 'cd zig && zig build test'`
  - `cargo test -p markymark-kernels`
  - `cargo test -p markymark-mcp search_symbols_`

## Implementation Checklist
- [ ] Add scalar reference matcher at `zig/src/reference/fuzzy_match_ref.zig` for correctness oracle.
- [ ] Add batched fuzzy kernel at `zig/src/kernels/fuzzy_match.zig` with candidate pointer/length arrays and top-k selection logic.
- [ ] Wire exports in `zig/src/c_adapter.zig` (new batched export + existing single-candidate export unchanged).
- [ ] Add Rust FFI wrappers in `markymark-kernels/src/scan.rs` for batched call and safe argument validation.
- [ ] Re-export wrapper API from `markymark-kernels/src/lib.rs`.
- [ ] Integrate batched path in `markymark-mcp/src/runtime_engine.rs` for SearchSymbols candidate ranking.
- [ ] Add focused tests in:
  - `zig/src/c_adapter.zig`
  - `markymark-kernels/src/scan.rs`
  - `markymark-mcp/tests/runtime_engine_tests.rs`
- [ ] Add/record benchmark command and output in issue notes.

## Key Considerations
- **Empty input contract:** define behavior for empty query and empty candidate set; scalar and kernel must match exactly.
- **UTF-8 safety:** matching remains byte-level but must never read out-of-bounds on multibyte text.
- **Capacity safety:** reject invalid output buffers and top-k > capacity with explicit error code.
- **Performance path:** avoid full sort when K << N; use bounded selection strategy.
- **Determinism:** identical inputs must produce identical outputs across runs.

## Anti-patterns
- ❌ No `unwrap()`/`expect()` in production Rust FFI path.
- ❌ No `todo!()`/`unimplemented!()` placeholders in merged code.
- ❌ No O(N log N) full-sort-only implementation for top-k selection in hot path.
- ❌ No per-candidate heap allocation inside tight scoring loop.
- ❌ No silent fallback that ignores non-zero FFI return codes.

## Test Specifications (must catch real bugs)
- `test_batch_top_k_stable_ties`: catches nondeterministic tie ordering bugs.
- `test_batch_invalid_capacity_returns_error`: catches potential buffer overrun behavior.
- `test_batch_empty_query_contract`: catches contract drift between scalar and kernel paths.
- `test_batch_no_match_returns_zero_written`: catches stale output/written handling bugs.
- `test_batch_subsequence_ranking_order`: catches scoring regressions in gap/consecutive handling.
- `test_batch_case_insensitive_match`: catches accidental case-sensitive behavior regressions.
- `test_batch_large_fixture_correct_top_k`: catches incorrect selection logic under larger N.
- `test_runtime_engine_uses_ranked_results`: catches integration regressions in SearchSymbols ordering.

## Verification Commands
- `bash -lc 'cd zig && zig build test'`
- `cargo test -p markymark-kernels`
- `cargo test -p markymark-mcp search_symbols_`
