---
id: marky-g9h
title: 'Task 2: Benchmark — verify from_engine_result_direct is measurably faster'
status: open
type: task
priority: 2
parent: marky-8d8
---

## Context

Phase 3a benchmark gate for sub-epic marky-8d8. After marky-u9q landed the direct arena
decode path, this task verifies measurable speedup via criterion.

The existing `realm_update.rs` benchmarks measure realm update operations using
`DocumentIndex::from_text()` (which includes engine creation + extraction + index construction).
This task adds a **focused benchmark** isolating only the index construction step, comparing:
1. Old path: `result.to_extraction()` + `from_engine_result_with_frontmatter(&extraction, ...)`
2. New path: `from_engine_result_direct(&result, ...)`

**Blocked by:** marky-u9q (closed — direct decode implemented)
**Unlocks:** Sub-epic benchmark criterion checked off. Confidence to proceed with Phase 3b.

## Requirements

From parent sub-epic marky-8d8:
- Benchmark: direct decode measurably faster than EngineExtraction path (Phase 3a alone)

## Success Criteria

- [ ] New criterion benchmark `index_construction` with two cases: `via_extraction` and `direct`
- [ ] Both cases use pre-created `EngineResult` from a ~50KB doc (same `generate_large_doc` as realm_update.rs)
- [ ] Benchmark isolates index construction only (engine creation + parsing in setup, not measured)
- [ ] `direct` case is measurably faster than `via_extraction` (criterion reports improvement)
- [ ] Results committed to bones log for traceability

## Anti-Patterns

- NO comparing against `from_text()` (that includes engine creation — conflates parse time with construction time)
- NO inventing numeric targets ("must be 2x faster") — measurable improvement is the criterion, not a threshold
- NO running in debug mode — benchmarks require `--release`

## Implementation

### Step 1: RED — Create benchmark file with both cases
**File:** `markymark-index/benches/index_construction.rs`
- Add `[[bench]]` entry in `markymark-index/Cargo.toml`: `name = "index_construction"`, `harness = false`
- Create benchmark with `criterion_group!` and `criterion_main!`
- Reuse `generate_large_doc` from realm_update.rs or duplicate the generator (benchmark files are standalone)
- Two bench functions in one group (`index_construction`):
  - `via_extraction`: setup creates DocumentEngine + gets EngineResult + parses frontmatter.
    Measured: `result.to_extraction()` + `DocumentIndex::from_engine_result_with_frontmatter()`
  - `direct`: same setup. Measured: `DocumentIndex::from_engine_result_direct()`
- Use `iter_batched` with `BatchSize::SmallInput` — EngineResult is cheap to clone via
  re-calling `engine.get_result()` (engine already parsed, result is a pointer copy + FFI call)
- Frontmatter parsing (`helpers::parse_frontmatter_owned`) goes in setup, not measured

### Step 2: Run benchmark, capture results
- `cargo bench -p markymark-index --bench index_construction`
- Criterion produces comparison output showing time per iteration for each case
- Capture the output for bones log

### Step 3: Commit results
- Commit benchmark file
- Log criterion output to bones: `bn log marky-g9h "Benchmark results: ..."`

## Key Considerations

- `engine.get_result()` borrows `&self` from DocumentEngine and returns an owned `EngineResult`.
  For `iter_batched`, create the engine in setup, call `get_result()` per iteration in setup
  (or once if the benchmark framework allows shared setup). The key is that engine creation +
  md4c parse are NOT in the measured section.
- `from_engine_result_with_frontmatter` takes `&EngineExtraction` — extraction must live for
  the duration of index construction. `from_engine_result_direct` takes `&EngineResult` — same
  constraint. Both are scoped to the iteration.
- `generate_large_doc` produces a ~50KB doc with ~40 headings, ~15 tags, ~5 block IDs — exercises
  all element types at realistic scale.
- Non-logic change (benchmark) — TDD escape hatch applies. No failing test needed.
