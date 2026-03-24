---
id: marky-1ic
title: Short-circuit blob pipeline when content hash unchanged
status: closed
type: task
priority: 2
parent: marky-lpb
---




## Context
Phase 1, Seam 1b of epic marky-zsys. Parent sub-epic marky-lpb.

**Blocked by:** marky-a02 (content_hash() FFI — now closed, method available)
**Unlocks:** Phase 1 completion — when this task closes, marky-lpb's remaining criteria
(ServerState hash storage, Option return, short-circuit in change paths, benchmark) are met,
enabling the Phase 1 acceptance task.

After Phase 1a, `DocumentEngine::content_hash()` returns a u64 via FFI. This task uses
that hash to skip the expensive blob serialization + deserialization + arena copy pipeline
when the document's structural content hasn't changed.

The three callers of `build_markdown_index_via_engine` in `markymark-lsp/src/state/mod.rs`:
- `open_document` (line 256) — always needs an index (first indexing, uses `realm.add_document`)
- `change_document` (line 276) — CAN skip when hash unchanged (uses `realm.update_document`)
- `apply_document_changes` (line 368) — CAN skip when hash unchanged (uses `realm.update_document`)

## Requirements
- R2 (from parent epic): LSP update path short-circuits blob serialization + deserialization
  when content hash is unchanged after `engine.update()`

## Implementation

**Code state (SRE verified 2026-03-24):** `build_markdown_index_via_engine` already returns
`Option<DocumentIndex>` (for the stale-index fallback case). Callers (`open_document`,
`change_document`, `apply_document_changes`) already handle `None`. The remaining work is
introducing the `EngineState` wrapper and hash comparison logic.

**Test access:** `build_markdown_index_via_engine` is private. Tests must go in a
`#[cfg(test)] mod tests` block inside `state/mod.rs`, or make the method `pub(crate)`.

1. **Write failing tests** (`markymark-lsp/src/state/mod.rs` — internal `#[cfg(test)]` module):
   - `test_build_index_returns_none_for_unchanged_content`: create `ServerState`, call
     `build_markdown_index_via_engine("file:///test.md", "# Hello\n")` → returns `Some`.
     Call again with same URI and same text → returns `None`.
   - `test_build_index_returns_some_for_changed_content`: same setup, call with
     `"# Hello\n"` then `"# Hello\n## World\n"` → both return `Some`.
   - `test_build_index_first_call_always_returns_some`: new URI always returns `Some`
     (no previous hash to compare).
   - `test_build_index_returns_some_for_reverted_content`: call with `"# Hello\n"`,
     then `"# World\n"`, then `"# Hello\n"` again → all three return `Some` (hash
     changes each time, even when reverting to original content). Catches: comparing
     against initial hash instead of last hash.

2. **Introduce `EngineState` wrapper struct** (`markymark-lsp/src/state/mod.rs`):
   Define `struct EngineState { engine: DocumentEngine, last_hash: u64 }` near `ServerState` (~line 96).
   Change `engines` field type from `HashMap<String, Mutex<DocumentEngine>>` to
   `HashMap<String, Mutex<EngineState>>` (line 111).
   Update `close_document` (~line 430) — `self.engines.remove()` still works, no logic change.

3. **Add hash comparison in existing engine update path** (~line 214, `Ok(())` branch):
   After `engine.update(&masked)` succeeds:
   - Call `engine.content_hash()` to get the new hash.
   - Compare with `engine_state.last_hash`.
   - If equal → return `None` (no structural change, skip blob pipeline).
   - If different → continue to `index_from_engine_result()`. **Only update
     `engine_state.last_hash = new_hash` when index build succeeds.** On failure,
     keep old hash so the next call retries the full pipeline.
   - Access pattern: the mutex guard already dereferences to `DocumentEngine`; with
     `EngineState`, destructure or field-access through the guard instead.

4. **Set initial hash on new engine creation path** (~line 252):
   After `DocumentEngine::new(&masked)` succeeds:
   - Call `engine.content_hash()`.
   - Store `EngineState { engine, last_hash: hash }` in the engines map.
   - Return `Some(index)` as before.

5. **Run tests and clippy**:
   - `cargo test -p markymark-lsp` — new hash tests + all existing tests pass
   - `cargo clippy -p markymark-lsp` — clean

6. **Benchmark verification**:
   - Run `cargo bench -p markymark-index -- realm_update` for baseline reference.
   - The existing bench exercises `realm.update_document()` directly (not the LSP layer).
     The short-circuit prevents reaching `update_document`, so the savings are above this bench.
   - Key question at implementation time: is a new LSP-level bench needed, or is demonstrating
     that `None` is returned (test) + existing bench timing sufficient for the criterion?

## Success Criteria
- [x] `EngineState` wrapper struct stores `DocumentEngine` + `last_hash: u64`
- [x] `ServerState.engines` uses `HashMap<String, Mutex<EngineState>>`
- [x] `build_markdown_index_via_engine` returns `Option<DocumentIndex>` (pre-existing)
- [x] Returns `None` when content hash is unchanged after `engine.update()`
- [x] Returns `Some(index)` on first call for a URI (no previous hash)
- [x] Returns `Some(index)` when content hash changes
- [x] `change_document` skips `realm.update_document()` when `None` (pre-existing)
- [x] `apply_document_changes` skips `realm.update_document()` when `None` (pre-existing)
- [x] `open_document` handles `Option` return correctly (pre-existing)
- [x] Test: None returned for unchanged content
- [x] Test: Some returned for changed content
- [x] Test: first call always returns Some
- [x] Test: Some returned for reverted content (A→B→A)
- [x] Benchmark: existing bench operates below short-circuit level; test proves None returned (blob pipeline skipped). realm_update bench: ~22-29µs. Savings are above this level (~2ms blob/arena work per epic analysis).
- [x] `cargo test -p markymark-lsp` passes (208 tests, 0 failures)
- [x] `cargo clippy -p markymark-lsp` clean

## Anti-Patterns
- NO pre-parse text hashing on Rust side (use engine's content_hash, not a competing hash)
- NO caching previous DocumentIndex to return on hash match (anti-pattern from parent epic — return None, not a cached copy)
- NO changing engine.update() call order — update must still run (the parse is cheap; the blob/arena work is expensive)
- NO skipping the hash check on new engine creation path (always set initial hash, always return Some)
- NO removing the fallback_scan_with_frontmatter paths (they handle error recovery; wrap in Some)
- NO introducing the hash comparison in open_document — only change_document and apply_document_changes benefit
- NO updating last_hash when index_from_engine_result fails — must keep old hash so next call retries
- NO marking pre-existing criteria as task deliverables — Option return type and caller handling are pre-existing, not this task's work

## Key Considerations

### Hash comparison goes AFTER engine.update(), not before
The engine must parse to compute the new hash. The short-circuit saves blob serialization +
deserialization + arena copy (~2ms), NOT the parse (~2.5ms). This is by design — the parse is
needed to get the hash.

### Frontmatter masking and the hash
`build_markdown_index_via_engine` masks frontmatter before calling `engine.update()`. The hash
is computed on the masked text. A frontmatter-only change (same markdown body) will change the
hash if masking produces different byte sequences. This is conservative and correct.

### open_document always returns Some
The first call for any URI creates a new engine — there's no previous hash to compare. The
`None` path only activates on subsequent calls with unchanged content. `open_document` should
still handle `None` defensively (log + fallback) but it's structurally impossible on first call.

### Only update last_hash on successful index build
If `engine.update()` succeeds but `index_from_engine_result()` fails, do NOT update
`last_hash`. If we update the hash but fail to build the index, the next call with the
same content would compare equal hashes and return `None` — skipping a rebuild that was
actually needed. Keeping the old hash forces the next call to retry the full pipeline.

### Mutex<EngineState> access pattern
All callers hold `&mut self` on `ServerState`, so the mutex is uncontested. The `lock()` call
is defense-in-depth, not contention management. The `EngineState` wrapper doesn't change this.

## Log

- [2026-03-24T08:00:17Z] [Seth] Debrief: EngineState wrapper + hash comparison implemented in ~30 lines of production code. Option return type and caller handling were pre-existing (SRE caught stale skeleton steps). 8 new tests including adversarial battery (empty, UTF-8, close-reopen, whitespace). 208 total tests pass, clippy clean. Reflections: skeleton steps 3-4 were pre-done — SRE correction was valuable. No workarounds, no surprises beyond stale skeleton. EngineState pattern established for future per-engine metadata.
