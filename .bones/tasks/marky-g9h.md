---
id: marky-g9h
title: 'Task 2: Benchmark — verify from_engine_result_direct is measurably faster'
status: active
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
- NO adjusting criterion parameters (sample size, warm-up, BatchSize, measurement time) after seeing initial results to change the outcome — design correctly from the start, report honest numbers (ref: fail-benchmark-chasing)
- NO rationalizing "no measurable difference" as a methodology problem to be solved — if criterion reports no improvement, that IS the result; log it honestly and escalate

## Implementation

### Step 1: Create benchmark file with both cases
**File:** `markymark-index/benches/index_construction.rs`
- Add `[[bench]]` entry in `markymark-index/Cargo.toml`: `name = "index_construction"`, `harness = false`
- Create benchmark with `criterion_group!` and `criterion_main!`
- Duplicate `generate_large_doc` from realm_update.rs (benchmark files are standalone, not library code)
- Two bench functions in one group (`index_construction`):

**Setup (IDENTICAL for both cases — not measured):**
1. `let doc = generate_large_doc(0);`
2. `let engine = DocumentEngine::new(&doc).unwrap();`
3. `let result = engine.get_result().unwrap();`
4. `let (fm, aliases) = helpers::parse_frontmatter_owned(&doc);`

**`via_extraction` measured section:**
1. `let extraction = result.to_extraction().unwrap();`
2. `DocumentIndex::from_engine_result_with_frontmatter(&extraction, fm, aliases);`

**`direct` measured section:**
1. `DocumentIndex::from_engine_result_direct(&result, fm, aliases).unwrap();`

- Use `iter_batched` with `BatchSize::SmallInput` — setup runs per iteration
- In setup, call `engine.get_result().unwrap()` to get a fresh EngineResult (FFI call, no re-parse)
- Clone `fm` and `aliases` in setup so each iteration has fresh owned data

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
- **Honest expectations:** The direct path still uses intermediate Vec collection (blob → owned
  Strings in Vecs → arena) because DocumentIndexCell's self_cell closure can't hold EngineResult
  borrows. The savings come from skipping EngineExtraction struct allocation + fewer intermediate
  data structures (no EngineHeading/EngineLink/etc. structs). Improvement may be modest. Report
  what criterion shows — do NOT adjust methodology if results are smaller than expected.
- **If no measurable improvement:** Log the honest results to bones. Escalate to user — this is
  a valid outcome that informs Phase 3b/3c design decisions. Do not treat it as a failure to fix.

## Log

- [2026-03-24T16:58:32Z] [Seth] Benchmark results — NO measurable improvement. via_extraction: [16.759 µs 17.068 µs 17.350 µs], direct: [17.025 µs 17.273 µs 17.526 µs]. Confidence intervals overlap. Both paths go through owned String intermediaries (self_cell constraint). The EngineExtraction overhead is negligible at this doc size. Escalating to user — this informs Phase 3b/3c design: the real win requires eliminating the owned String intermediary via lifetime parameterization (DocumentIndex<'engine>), not just bypassing EngineExtraction.
