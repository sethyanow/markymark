---
id: marky-03r
title: 'Phase 3b: Blob-in-owner — eliminate intermediate String allocations'
status: active
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

### Step 1: Baseline — verify existing parity test passes, then extend
File: `markymark-index/src/document/tests/from_engine_direct_tests.rs`
Run existing `test_from_engine_result_direct_parity` to confirm current behavior.
This is a refactor of internals — the existing parity test IS the regression gate.
No new test needed for RED step; the existing test covers the public contract.

Run: `cargo nextest -p markymark-index -E 'test(direct_parity)'`
Expected: passes (baseline confirmation before refactoring internals)

### Step 2: Add `text_blob: Vec<u8>` to DocumentOwner
File: `markymark-index/src/document/mod.rs` — add field to struct
Files: `from_engine.rs:192`, `from_engine_direct.rs:37` — add `text_blob: Vec::new()` to existing sites

Run: `cargo nextest -p markymark-index`
Expected: all existing tests pass (no behavior change)

### Step 3: Refactor `from_engine_result_direct` — blob reads inside closure
File: `markymark-index/src/document/from_engine_direct.rs`

- Set `owner.text_blob = result.text_blob().to_vec()`
- Switch `DocumentIndexCell::new` → `try_new` (closure returns `Result<_, KernelError>`)
- Replace pre-closure Vec collections of owned Strings with Vecs of Copy-type C structs
  (e.g., `result.headings()?.to_vec()` → `Vec<CEngineHeading>`). Numeric metadata still
  collected pre-closure; only the `.to_owned()` text copies are eliminated.
- Inside closure: iterate the C struct Vecs, call `read_blob_str(&owner.text_blob, offset, len)?`
  to get `&'a str` directly from owner — no arena_alloc_str for blob-sourced text.
- Typed slices (`result.headings()`, etc.) borrow from EngineResult which is NOT accessible
  inside the closure. Collect via `.to_vec()` on the typed slices before closure entry.
  The C structs (CEngineHeading, CEngineLink, etc.) are all Copy — no heap String allocation.

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

- [x] `from_engine_result_direct` has zero `.to_owned()` calls for blob-derived strings (pre-closure AND inside closure)
- [x] Blob-sourced text fields inside closure use `read_blob_str(&owner.text_blob, ...)` directly — NOT `arena_alloc_str`
- [x] `DocumentOwner.text_blob` holds blob bytes; closure reads from `owner.text_blob`
- [x] `DocumentIndexCell` uses `try_new` (fallible closure)
- [x] No unsafe code introduced
- [x] Existing parity test passes (identical output)
- [x] All workspace tests pass (1290/1290)
- [x] Benchmark shows improvement: 21% (1KB), 31% (10KB), 63% (100KB) faster than via_extraction

## Anti-Patterns

- NO pre-closure `.to_owned()` on blob-derived strings — Vecs of Copy-type C structs (numeric metadata) are expected; Vecs of owned Strings are not
- NO `arena_alloc_str` for blob-sourced text inside closure — use `read_blob_str(&owner.text_blob, ...)` directly; only frontmatter/aliases (Rust-side data) use arena allocation
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
- **Edge: wiki link '#' at boundary positions.** `target.find('#')` at pos 0 → empty page, full
  heading. At end → full page, empty heading. Both are valid — existing tests should cover.
  Verify existing wiki link edge case tests still pass after refactor.
- **Edge: empty text_blob with non-zero element counts.** If blob is empty but typed slices
  report elements, `read_blob_str` will return `KernelError` (out-of-bounds). `try_new` propagates
  this correctly. No special handling needed.
- **Edge: property value_type=1 (list) with empty value.** `"".split(',')` → one empty string
  item, not empty list. Matches pre-refactor behavior (same split logic, different source).
- **self_cell 1.2.2** provides `try_new` — verified in workspace Cargo.toml. Closure returns
  `Result<DocumentDependent<'_>, KernelError>`.

### Adversarial Failure Catalog (SRE)

**Resource: DocumentOwner.text_blob duplication**
- Assumption: Extra Vec copy is acceptable memory
- Betrayal: During construction, both EngineResult blob AND owner.text_blob exist simultaneously (2x spike)
- Consequence: Transient — EngineResult dropped immediately after try_new. Steady-state is one copy. Pre-refactor had LARGER transient spike (owned Strings > raw bytes)
- Mitigation: Structural — same bounded-by-document-size pattern as before, strictly better

**Temporal: self_cell try_new single-call guarantee**
- Assumption: Closure executes once, synchronously
- Betrayal: Future self_cell version could change semantics
- Mitigation: Version pinned at 1.2.2 in workspace Cargo.toml. Bump requires audit

**State: try_new Err drops owner cleanly**
- Assumption: Failed try_new doesn't leak arena or blob
- Betrayal: None — Bump drops in bulk, Vec drops normally. Rust drop semantics handle this
- Mitigation: Structural — no action needed

**Encoding: multibyte UTF-8 split at blob boundary**
- Assumption: Zig extraction produces UTF-8-aligned offset/length pairs
- Betrayal: Bad Zig offset splits a multibyte character
- Consequence: read_blob_str → from_utf8 → Err → try_new propagates KernelError. Correct fail-loud behavior
- Mitigation: Structural — identical validation to pre-refactor. Zig bug surfaces as error, not silent corruption

**Input: zero-length string at blob end**
- Assumption: read_blob_str(blob, blob.len(), 0) works
- Betrayal: None — verified: blob.get(n..n) where n==len returns Some(&[]). from_utf8(&[]) → Ok("")
- Mitigation: N/A — works correctly

**Concurrency: EngineResult pointer validity during typed slice iteration**
- Assumption: EngineResult internal pointers valid throughout pre-closure collection
- Betrayal: Concurrent engine update or free would dangle pointers
- Mitigation: Structural — &EngineResult borrow prevents Drop. DocumentEngine behind Mutex prevents concurrent update. No concurrent free possible in safe code

## Log

- [2026-03-24T18:24:07Z] [Seth] Task scoped during executing-plans session. Design divergence from R6/R7: user confirmed blob-in-owner approach over lifetime parameter cascade (2026-03-24). Key insight: double-copy exists because text_blob not in self_cell owner. Fix: Vec<u8> in DocumentOwner, try_new for fallible reads, direct &'a str borrows from owner.text_blob.
