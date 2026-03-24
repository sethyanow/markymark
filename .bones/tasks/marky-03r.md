---
id: marky-03r
title: 'Phase 3b: Blob-in-owner — eliminate intermediate String allocations'
status: open
type: task
priority: 2
parent: marky-8d8
---




## Context

Parent sub-epic: marky-8d8 (Phase 3: Direct Arena Decode)
Parent epic: marky-zsys (Engine Pipeline v2)

Phase 3a (marky-u9q) created `from_engine_result_direct` which bypasses EngineExtraction
but still double-copies text: blob → `.to_owned()` Strings in Vecs → `arena_alloc_str` in
arena. The double-copy exists because the self_cell closure only sees `&owner`, and the
text_blob lives outside the closure (in the temporary EngineResult).

**Design decision (user-confirmed 2026-03-24):** Instead of the originally planned
`DocumentIndex<'engine>` lifetime parameter (R6/R7), store text_blob bytes in DocumentOwner.
Inside the closure, `read_blob_str(&owner.text_blob, ...)` returns `&'a str` borrowing from
the owner — zero copy per string. This achieves the same optimization without lifetime
cascade through RealmIndex/ServerState.

**Blocked by:** marky-u9q (Phase 3a — closed), marky-g9h (benchmark — closed)
**Unlocks:** Phase 3 acceptance task (all implementation criteria met)

## Requirements

- Eliminate pre-closure Vec collections of owned Strings in `from_engine_result_direct`
- Text fields in DocumentIndex entries borrow from owner.text_blob (via self_cell `'a` lifetime)
- No public API change, no lifetime parameter, no RealmIndex/ServerState cascade
- No unsafe code introduced

## Design

`DocumentOwner` gains `text_blob: Vec<u8>` — a copy of `EngineResult.text_blob()`.
`DocumentIndexCell::new` → `try_new` for fallible blob reads inside the closure.

Inside the closure:
- `read_blob_str(&owner.text_blob, offset, len)?` → `&'a str` (borrows from owner)
- Wiki link `#` splits: `&target[..pos]` — valid subslice, still `&'a str`
- Frontmatter + aliases: still arena-allocated (Rust-side data, not in blob)
- Task state: `&'static str` ("checked" / "unchecked")
- Entry struct arrays: still BumpVec in arena (slice pattern)

EngineResult is NOT stored — only its text_blob bytes are copied. EngineResult is
still temporary (obtained, blob copied, result dropped).

## Implementation

### Step 1: Write failing test — new constructor compiles
File: `markymark-index/src/document/tests/from_engine_direct_tests.rs`
Add `test_from_engine_result_direct_v2_parity` calling the refactored constructor.
Structure mirrors existing parity test — asserts identical output to old path.

Run: `cargo nextest -p markymark-index -E 'test(direct_v2_parity)'`
Expected: compile error (constructor not refactored yet)

### Step 2: Add `text_blob: Vec<u8>` to DocumentOwner
File: `markymark-index/src/document/mod.rs` — add field to struct
Files: `from_engine.rs:192`, `from_engine_direct.rs:37` — add `text_blob: Vec::new()` to existing sites

Run: `cargo nextest -p markymark-index`
Expected: all existing tests pass (no behavior change)

### Step 3: Refactor `from_engine_result_direct` — blob reads inside closure
File: `markymark-index/src/document/from_engine_direct.rs`

- Set `owner.text_blob = result.text_blob().to_vec()`
- Switch `DocumentIndexCell::new` → `try_new` (closure returns `Result<_, KernelError>`)
- Delete all pre-closure Vec collections (headings_data, wiki_data, md_data, etc.)
- Inside closure: iterate typed slices, read text from `owner.text_blob`, build entries
- Typed slices (`result.headings()`, etc.) borrow from EngineResult — iterate OUTSIDE closure, pass numeric data in. Or iterate inside if slices can be made accessible. Executing agent determines what compiles.

Run: `cargo nextest -p markymark-index`
Expected: parity test passes, all tests pass

### Step 4: Full workspace test suite
Run: redirect full suite to file
Expected: no regressions

### Step 5: Benchmark
File: `markymark-index/benches/index_construction.rs`
Run existing benchmark before/after to measure delta.

### Step 6: Update sub-epic criteria
Edit `.bones/tasks/marky-8d8.md` — check off satisfied criteria, annotate N/A criteria.

## Success Criteria

- [ ] `from_engine_result_direct` has zero pre-closure `.to_owned()` calls for blob-derived strings
- [ ] `DocumentOwner.text_blob` holds blob bytes; closure reads from `owner.text_blob`
- [ ] `DocumentIndexCell` uses `try_new` (fallible closure)
- [ ] No unsafe code introduced
- [ ] Existing parity test passes (identical output)
- [ ] All workspace tests pass
- [ ] Benchmark shows improvement over Phase 3a baseline (or at minimum no regression)

## Anti-Patterns

- NO keeping pre-closure Vec collections "for safety" — eliminating them is the point
- NO `unsafe { from_utf8_unchecked }` — use `try_new` for fallible reads
- NO storing full EngineResult in owner — copy only text_blob bytes
- NO lifetime parameter on DocumentIndex — blob-in-owner replaces R6/R7

## Key Considerations

- The typed slices (`result.headings()`, etc.) borrow from EngineResult which is still alive
  during cell construction but NOT accessible inside the closure. Options: (a) iterate outside
  and pass numeric fields into the closure, (b) also store the typed slice data in the owner.
  Option (a) is likely simpler.
- `read_blob_str` validates UTF-8 on every call. The blob bytes are identical to what was
  previously validated — but we re-validate inside the closure because `try_new` handles it.
- Property list splitting (`value.split(',').map(|s| s.trim())`) produces subslices of blob
  strings — still `&'a str`, no arena allocation needed.

## Log

- [2026-03-24T18:24:07Z] [Seth] Task scoped during executing-plans session. Design divergence from R6/R7: user confirmed blob-in-owner approach over lifetime parameter cascade (2026-03-24). Key insight: double-copy exists because text_blob not in self_cell owner. Fix: Vec<u8> in DocumentOwner, try_new for fallible reads, direct &'a str borrows from owner.text_blob.
