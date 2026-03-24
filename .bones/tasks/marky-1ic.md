---
id: marky-1ic
title: Short-circuit blob pipeline when content hash unchanged
status: open
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

1. **Write failing tests** (`markymark-lsp/src/state/mod.rs` or new test file):
   - `test_build_index_returns_none_for_unchanged_content`: create `ServerState`, call
     `build_markdown_index_via_engine("file:///test.md", "# Hello\n")` → returns `Some`.
     Call again with same URI and same text → returns `None`.
   - `test_build_index_returns_some_for_changed_content`: same setup, call with
     `"# Hello\n"` then `"# Hello\n## World\n"` → both return `Some`.
   - `test_build_index_first_call_always_returns_some`: new URI always returns `Some`
     (no previous hash to compare).

2. **Introduce `EngineState` wrapper struct** (`markymark-lsp/src/state/mod.rs`):
   Define `struct EngineState { engine: DocumentEngine, last_hash: u64 }` near `ServerState`.
   Change `engines` field type: `HashMap<String, Mutex<EngineState>>`.
   Update `close_document` (line 384-388) which does `self.engines.remove()` — no logic change needed.

3. **Change `build_markdown_index_via_engine` return type** to `Option<DocumentIndex>`:
   - **Existing engine path** (line 156-196): after `engine.update(&masked)` succeeds, call
     `engine.content_hash()`. Compare with `engine_state.last_hash`. If equal → update
     `last_hash` (defensive), return `None`. If different → continue to `get_blob()` +
     `from_blob_with_frontmatter()`, update `last_hash`, return `Some(index)`.
   - **New engine path** (line 197-235): create engine, get initial hash, store
     `EngineState { engine, last_hash: hash }`, return `Some(index)` (always).
   - **Fallback paths**: all return `Some(fallback_scan_with_frontmatter(text))`.
   - **Poisoned mutex path**: return `Some(fallback)` (can't compare hash, rebuild conservatively).

4. **Update callers** (`markymark-lsp/src/state/mod.rs`):
   - `open_document` (line 256): `if let Some(index) = self.build_markdown_index_via_engine(...)`.
     First call for a URI always returns Some — but defend with an else log-warn + fallback.
   - `change_document` (line 276): `if let Some(index) = self.build_markdown_index_via_engine(...)`.
     `None` → skip `realm.update_document()` entirely.
   - `apply_document_changes` (line 368): same pattern as `change_document`.

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
- [ ] `EngineState` wrapper struct stores `DocumentEngine` + `last_hash: u64`
- [ ] `ServerState.engines` uses `HashMap<String, Mutex<EngineState>>`
- [ ] `build_markdown_index_via_engine` returns `Option<DocumentIndex>`
- [ ] Returns `None` when content hash is unchanged after `engine.update()`
- [ ] Returns `Some(index)` on first call for a URI (no previous hash)
- [ ] Returns `Some(index)` when content hash changes
- [ ] `change_document` skips `realm.update_document()` when `None`
- [ ] `apply_document_changes` skips `realm.update_document()` when `None`
- [ ] `open_document` handles `Option` return correctly (always gets Some)
- [ ] Test: None returned for unchanged content
- [ ] Test: Some returned for changed content
- [ ] Test: first call always returns Some
- [ ] Benchmark: unchanged-content update measurably faster (or documented why existing bench is sufficient)
- [ ] `cargo test -p markymark-lsp` passes
- [ ] `cargo clippy -p markymark-lsp` clean

## Anti-Patterns
- NO pre-parse text hashing on Rust side (use engine's content_hash, not a competing hash)
- NO caching previous DocumentIndex to return on hash match (anti-pattern from parent epic — return None, not a cached copy)
- NO changing engine.update() call order — update must still run (the parse is cheap; the blob/arena work is expensive)
- NO skipping the hash check on new engine creation path (always set initial hash, always return Some)
- NO removing the fallback_scan_with_frontmatter paths (they handle error recovery; wrap in Some)
- NO introducing the hash comparison in open_document — only change_document and apply_document_changes benefit

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

### Mutex<EngineState> access pattern
All callers hold `&mut self` on `ServerState`, so the mutex is uncontested. The `lock()` call
is defense-in-depth, not contention management. The `EngineState` wrapper doesn't change this.
