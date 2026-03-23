---
id: marky-ut8
title: 'Phase 4.2: Migrate from_scan_with_frontmatter production callers + delete from_ast.rs'
status: closed
type: task
priority: 2
owner: Seth
parent: marky-0xtn
---




## Context

- Phase 4.1 (marky-llj) added `from_text()` and migrated all 18 from_ast callers. from_ast now has zero callers.
- `from_scan_with_frontmatter` still has 5 callers (excl. definition): 2 production fallbacks (MCP + LSP), 1 MCP AddRoot handler, 1 LSP test, 1 dead from_ast.rs call.
- The MCP AddRoot handler (`CoreEngine::execute` at mod.rs:589-624) duplicates `index_root_into_realm` using from_scan — a Phase 3 gap.
- `from_scan` (no frontmatter) has 13 callers in test/bench code — separate task scope.

**Blocked by:** marky-llj (closed — from_text available)
**Unlocks:** from_ast.rs deletion, from_scan_with_frontmatter caller elimination from production. After this, from_scan_with_frontmatter callers are only in from_scan.rs itself (called by the from_scan_inner chain) and from_scan test code.

## Requirements

- R9 (from epic): DocumentIndex::from_text() convenience function replaces from_ast/from_scan/from_blob in all tests.
- R4 (from epic): MCP batch path uses persistent engines + from_engine_result (no from_scan).
- This task covers: migrate from_scan_with_frontmatter production callers to from_text/engine path, delete from_ast.rs.

## Implementation

1. Write equivalence test: in `markymark-mcp/src/engine/tests/`, verify that `DocumentIndex::from_text(text)` produces the same headings, tags, links, and frontmatter as the current `fallback_scan_with_frontmatter(text)` for a mixed document. Run → should pass.

2. Replace MCP `fallback_scan_with_frontmatter` body (mod.rs:262-265):
   - Change body to: `DocumentIndex::from_text(text)`
   - Remove `Md4cScanBackend` import if no other caller uses it in this function's scope.

3. Replace MCP `AddRoot` handler (mod.rs:589-697, full span): The inline scan loop duplicates `index_root_into_realm`. Replace the multi-phase implementation with:
   - Keep Phase 1 (validate + register root, lines 589-597) — this is fast sync under write lock.
   - Replace Phases 2-4 (lines 599-696) with: re-acquire write lock, get `&mut RealmData`, call `index_root_into_realm(root, realm_data).await`, return `CoreOperationResult::RealmInfo`.
   - **Concurrency trade-off:** The current handler releases the write lock during file I/O (Phase 2) and semantic embedding (Phase 3) for concurrency. `index_root_into_realm` requires `&mut RealmData`, so the write lock must be held throughout. This is acceptable: AddRoot is rare (user-initiated), startup already uses this same pattern (`from_workspace_roots`), and the goal is correctness (engine path) not optimal concurrency. The stale-race detection (lines 667-683) becomes unnecessary and should be removed.
   - **Semantic embedding:** `index_root_into_realm` calls `realm.index.add_documents().await` which handles semantic embedding internally (if feature enabled), unlike the current handler's explicit Phase 3 embedding loop. Verify `add_documents` handles the semantic-search path by checking its behavior with `#[cfg(feature = "semantic-search")]`.
   - Remove `Md4cScanBackend` import if no longer needed after this change.

4. Replace LSP `fallback_scan_with_frontmatter` body (state/mod.rs:164-168):
   - Same swap to: `DocumentIndex::from_text(text)`
   - Remove `Md4cScanBackend` import if no longer needed.

5. Migrate LSP test (state_tests.rs:747):
   - Replace `from_scan_with_frontmatter(&masked, &Md4cScanBackend, fm, aliases)` with `DocumentIndex::from_text(text)`.
   - Remove frontmatter parse+mask setup lines since from_text handles them internally.

6. Delete `from_ast.rs`:
   - Remove `markymark-index/src/document/from_ast.rs`
   - Remove `mod from_ast;` from `document/mod.rs:6`

7. Update compile_fail doctest in `document/mod.rs:131-141`:
   - Currently uses `Parser` + `from_ast` to demonstrate arena lifetime safety
   - Replace with `from_text("# Title")` — the replacement doctest:
     ```rust
     /// ```compile_fail
     /// use markymark_index::DocumentIndex;
     ///
     /// fn leak_index_text() -> &'static str {
     ///     let index = DocumentIndex::from_text("# Title");
     ///     index.headings()[0].text
     /// }
     /// ```
     ```
   - This should still fail to compile because `text` is arena-backed (`&'a str`) and can't outlive the `DocumentIndex`. Expected error: lifetime mismatch / borrowed value does not live long enough.

8. Verify: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, zero from_scan_with_frontmatter callers in production code (MCP + LSP).

## Key Considerations

- `fallback_scan_with_frontmatter` is a fallback for engine failures. Replacing its body with `from_text` changes the fallback from scan→engine rather than scan→scan. This is correct: `from_text` creates an ephemeral engine, and if the persistent engine failed, an ephemeral one may succeed (fresh state, no stale handle). If `from_text` also fails, it panics — but this is acceptable because the fallback is last resort before stale cache.
- **AddRoot concurrency regression (acceptable):** The MCP AddRoot handler (mod.rs:589-697) uses a 4-phase locking pattern: Phase 1 validates under write lock, releases lock, Phase 2-3 do I/O and semantic embedding without lock, Phase 4 re-acquires write lock to insert. Replacing with `index_root_into_realm` holds the write lock for the entire duration (file I/O + parsing + engine creation). This blocks other realm operations during AddRoot. Acceptable because: (1) AddRoot is rare/user-initiated, (2) startup already uses this pattern, (3) correct engine path matters more than optimal concurrency for a rarely-called operation. The stale-race detection (lines 667-683) is safely removable since the lock is held throughout.
- **Semantic embedding path in AddRoot:** The current handler has an explicit Phase 3 embedding loop. `index_root_into_realm` delegates to `add_documents()` which handles semantic embedding internally. Verify that `add_documents` correctly embeds when `semantic-search` feature is enabled — don't just assume.
- After this task, `from_scan_with_frontmatter` will still be referenced by from_scan's internal chain and test code. The from_scan.rs file itself won't be deletable yet — that's a follow-up task.
- The compile_fail doctest change is load-bearing: it proves arena lifetime safety. Verify the doctest still fails to compile for the right reason after the change.

### Failure Catalog (Adversarial Planning)

**Temporal Betrayal: MCP fallback_scan_with_frontmatter**
- Assumption: If the persistent engine failed, an ephemeral engine via `from_text` will succeed (fresh state).
- Betrayal: The Zig FFI library itself is corrupted or unloadable (not just stale engine state). Both persistent and ephemeral engines fail identically.
- Consequence: `from_text` panics. But the callsite is inside `build_markdown_index_via_engine` which tries stale cache FIRST, then falls back to scan. After this change, the fallback path is: stale cache → from_text (panic on failure). If stale cache exists, the panic path is never reached for the same document.
- Mitigation: Structural — the stale index cache (preceding fallback) handles the common case (re-edit of a previously-successful document). The panic case only triggers for a document that has NEVER been successfully indexed AND the engine is fundamentally broken. This is a startup-time failure, not a runtime failure for established workspaces.

**State Corruption: AddRoot handler — partial engine state on panic**
- Assumption: `index_root_into_realm` either completes fully or fails cleanly.
- Betrayal: Processing file 50 of 200, `from_text`/`build_markdown_index_via_engine` panics. Write lock is poisoned. Files 1-49 have engines in `realm.engines` and indexes in `realm.index`. Files 51-200 are missing.
- Consequence: RwLock poisoning makes all subsequent realm operations fail. The partial state is moot because nothing can read it.
- Mitigation: This is the same risk the current handler has — `from_scan_with_frontmatter` could also panic. The engine path is actually safer: `build_markdown_index_via_engine` catches engine failures and falls back, only panicking if BOTH engine AND fallback fail. The current handler calls `from_scan_with_frontmatter` which panics directly on any scan failure. Net improvement.

**Dependency Treachery: compile_fail doctest — wrong failure reason**
- Assumption: The doctest fails to compile because arena-backed `&'a str` can't outlive `DocumentIndex`.
- Betrayal: `from_text` is not re-exported from `markymark_index`, so the doctest fails on "unresolved import" instead of lifetime error.
- Consequence: The doctest "passes" (it fails to compile) but proves nothing about arena lifetime safety.
- Mitigation: Verify `from_text` is accessible as `markymark_index::DocumentIndex::from_text` — check that `from_engine.rs` (where `from_text` lives) is pub and that `DocumentIndex` is re-exported. If not, the doctest must use the correct import path. After writing the doctest, run `cargo test --doc -p markymark-index` and inspect the actual compile error message to confirm it's a lifetime error.

**Input Hostility: from_text with frontmatter-only documents**
- Assumption: `from_text` handles documents that are 100% frontmatter (no markdown body).
- Betrayal: After `mask_frontmatter`, the entire text is whitespace. `DocumentEngine::new("")` with empty/whitespace-only input might produce unexpected results.
- Consequence: Empty headings/tags arrays — correct behavior. No failure expected, but the equivalence test (Step 1) should include a frontmatter-only test case.
- Mitigation: Include a frontmatter-only document in the equivalence test.

**Encoding Boundaries / Temporal Betrayal: Skipped categories by component**
- MCP/LSP fallback body replacement: Encoding boundaries N/A — both functions receive `&str` (Rust guarantees UTF-8), Zig FFI handles C string conversion internally (already tested by persistent engine path). Resource exhaustion N/A — same memory profile as current path.
- from_ast.rs deletion: All runtime categories N/A — pure compile-time change. Zero callers verified by LSP.
- LSP test migration: All failure categories N/A — test code only, no production impact. The test exercises the same `from_text` path that production uses.

## Success Criteria

- [x] MCP `fallback_scan_with_frontmatter` uses `from_text` (no from_scan_with_frontmatter)
- [x] MCP `AddRoot` handler uses `index_root_into_realm` (no inline scan loop)
- [x] LSP `fallback_scan_with_frontmatter` uses `from_text` (no from_scan_with_frontmatter)
- [x] LSP test migrated to `from_text`
- [x] `from_ast.rs` deleted — file removed, mod declaration removed
- [x] compile_fail doctest updated to use `from_text` — verified the compile error is a lifetime error (not unresolved import)
- [x] cargo test --workspace passes
- [x] cargo clippy --workspace -- -D warnings passes
- [x] Equivalence test includes frontmatter-only document case

## Anti-Patterns

- No keeping from_scan_with_frontmatter in production fallback — from_text is the replacement
- No deleting from_scan.rs yet — it still has 13 test callers that are a separate task
- FORBIDDEN: changing fallback_scan_with_frontmatter to return Result — it's a last-resort fallback, panic on failure is acceptable
- No removing the stale index cache fallback — that stays (it precedes the scan fallback in the fallback chain)
- FORBIDDEN: skipping `cargo test --workspace --features semantic-search` verification for AddRoot changes — the semantic embedding path in AddRoot is behind a feature flag and must be tested
- FORBIDDEN: leaving Phases 2-4 of AddRoot handler partially replaced — the entire inline scan loop (lines 599-696) must be replaced, not patched piecemeal
