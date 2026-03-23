---
id: marky-xfgb
title: 'Phase 3: Switch MCP markdown indexing to CEngineResult persistent engines'
status: closed
type: feature
priority: 2
parent: marky-0xtn
---






## Context

- marky-e0kp added additive CEngineResult FFI and conversion path.
- marky-2nxj switched LSP markdown indexing to CEngineResult with stale-first fallback.
- Epic marky-0xtn still requires MCP migration (R4) before blob/CMd4c cleanup can proceed.

## Requirements

Replace MCP markdown indexing path from scan-based extraction to persistent DocumentEngine + CEngineResult conversion.

## Implementation

**Primary file:** `markymark-mcp/src/engine/mod.rs`
**Reference implementation:** `markymark-lsp/src/state/mod.rs` (Phase 2, `build_markdown_index_via_engine`)

1. Add `engines: HashMap<String, std::sync::Mutex<DocumentEngine>>` field to `RealmData` in `markymark-mcp/src/engine/mod.rs`.
   - `DocumentEngine` is `Send` but NOT `Sync` (raw `*mut c_void` handle). Wrapping in `std::sync::Mutex` satisfies the `Sync` bound required by `RwLock<HashMap<String, RealmData>>` on `RuntimeEngine`.
   - Key by `DocumentUri::as_str().to_string()` (same convention as LSP).
   - Update `RealmData::new()` to initialize `engines: HashMap::new()`.

2. Rewrite `index_root_into_realm()` markdown branch to use engine path:
   - For each markdown file: parse frontmatter, mask source, then:
     - If engine exists for this URI: lock, update with masked text, get result.
     - If no engine: `DocumentEngine::new(&masked)`, get result, insert into `realm.engines`.
   - Convert result: `engine.get_result()` → `result.to_extraction()` → `DocumentIndex::from_engine_result_with_frontmatter()`.
   - Collect `(uri, DocumentIndex)` tuples, bulk-add via `realm.index.add_documents()`.

3. Add fallback chain (matching LSP pattern from `build_markdown_index_via_engine`):
   - On engine update/create failure: try stale engine snapshot via `get_result()` on the existing engine.
   - On result conversion failure: check `realm.index.get_document(&uri).is_some()` for stale cached index (return `None` to preserve it).
   - On no stale state available: fall back to `DocumentIndex::from_scan_with_frontmatter()`.
   - Log warnings at each fallback transition.

4. Update `unindex_root_from_realm()` to also remove engines for files under the removed root:
   - After collecting URIs to remove, also `realm.engines.remove()` for each.

5. Remove the `Md4cScanBackend` import/usage from production `index_root_into_realm` (keep only as fallback).

6. Add tests in `markymark-mcp/src/engine/tests/`:
   - `test_engine_index_markdown_success`: create engine, get result, verify DocumentIndex has expected headings/links.
   - `test_engine_index_update`: index file, update with changed content, verify index reflects changes.
   - `test_engine_fallback_on_update_failure`: force engine update failure, verify stale snapshot returned (not empty index).
   - `test_engine_fallback_scan_when_no_stale`: force engine create failure on first index, verify scan fallback produces valid index.
   - `test_engine_cleanup_on_root_removal`: add root with engines, remove root, verify engines HashMap is empty for that root's files.
   - `test_engine_frontmatter_preserved`: index file with YAML frontmatter + aliases, verify they appear in DocumentIndex.

## Key Considerations

- Input edges: empty docs, frontmatter-only docs, unicode headings, large markdown files.
- State transitions: first add, repeated updates, remove, and missing-entry update behavior.
- Failure modes: engine create fail, update fail, result fetch fail, conversion fail, lock poison.
- Concurrency: `index_root_into_realm` takes `&mut RealmData` (exclusive access via RwLock write guard), so engine Mutex locks are uncontested in practice. Mutex is for Sync bound, not contention.
- Data integrity: preserve frontmatter aliases, headings/tags/links parity with existing behavior.
- Regression guardrails: no unwrap/expect in production paths and explicit warning logs for fallbacks.
- Cleanup: no dead scan-only markdown branch left in touched MCP code.
- Engine cleanup: `unindex_root_from_realm` must remove engines alongside documents — leaked engines are a memory leak (Zig heap allocations).
- Fallback nuance: when `has_stale_index` is true and engine fails, return `None` to preserve the existing index in the RealmIndex (same pattern as LSP). When `has_stale_index` is false, fall back to scan to avoid empty results.

### Adversarial Failure Catalog

**Temporal Betrayal: Re-indexing same root**
- Assumption: `index_root_into_realm` is only called once per root.
- Betrayal: MCP calls it again (root re-added, workspace refresh). Engines already exist for every file.
- Consequence: If implementation only handles the `DocumentEngine::new()` path, re-indexing silently skips engine updates and returns stale results.
- Mitigation: Step 2 already requires update-if-exists / create-if-not logic. Implementation must branch on `realm.engines.get(uri_str)`.

**Temporal Betrayal: File deleted between index cycles**
- Assumption: All engine entries correspond to live files.
- Betrayal: File deleted after indexing. Engine persists in `realm.engines`. `collect_documents()` won't find it on next index call, so the engine is never updated again.
- Consequence: Stale engine accumulates. Not a correctness issue (orphan engine is never consulted), but leaked Zig memory until root removal.
- Mitigation: Acceptable — `unindex_root_from_realm` cleans up all engines under the root prefix. Engines are bounded by workspace file count.

**Dependency Treachery: Stale engine `get_result()` semantics**
- Assumption: `get_result()` after a failed `update()` returns an error.
- Betrayal: `get_result()` returns the LAST SUCCESSFUL parse result (Zig engine caches). This is by design — it's the stale snapshot.
- Consequence: None if understood correctly — this IS the stale fallback mechanism. But if misunderstood, the implementation might treat stale snapshot success as "update succeeded."
- Mitigation: Log clearly distinguishes "update failed, using stale snapshot" from "update succeeded, using fresh result." The LSP reference implementation already models this correctly.

**State Corruption: Panic during bulk `add_documents()`**
- Assumption: All engines and document indexes are added atomically.
- Betrayal: `realm.index.add_documents(markdown_docs).await` panics mid-batch. Some engines were created but their DocumentIndexes never reached the RealmIndex.
- Consequence: On next index call, orphan engines exist without corresponding index entries. `has_stale_index` returns false for those URIs even though engines exist.
- Mitigation: Acceptable — `DocumentEngine::drop` (verified at `engine.rs:242`) cleans up Zig memory on process exit or root removal. Next index call updates the orphan engines and adds their indexes. No silent data loss.

**Input Hostility: Frontmatter-only file (all-spaces after masking)**
- Assumption: `DocumentEngine::new()` receives non-empty markdown content.
- Betrayal: File is `---\ntitle: foo\n---\n` — after masking, text is all spaces/newlines.
- Consequence: Engine produces empty extraction. `from_engine_result_with_frontmatter()` builds DocumentIndex with frontmatter but zero headings/links.
- Mitigation: This is correct behavior — frontmatter-only files have no markdown structure. Verified: `test_engine_empty_input` in `engine.rs:285` proves `DocumentEngine::new("")` works.

## Success Criteria

- [x] MCP markdown path has zero production scan-based calls in migrated flow (verify: no `from_scan_with_frontmatter` in `index_root_into_realm` happy path)
- [x] Persistent engine state exists for MCP markdown lifecycle (`RealmData.engines: HashMap<String, Mutex<DocumentEngine>>`)
- [x] Fallback ordering is stale-first and scan-second when stale is unavailable (verify: test for both fallback paths)
- [x] Engine cleanup on root removal (`unindex_root_from_realm` removes engines for affected files)
- [x] Tests cover: success path, update failure → stale fallback, create failure → scan fallback, conversion failure → stale fallback, root removal cleanup, frontmatter preservation
- [x] cargo test --package markymark-mcp passes
- [x] cargo check passes workspace-wide
- [x] cargo clippy --package markymark-mcp -- -D warnings passes

## Anti-Patterns

- No per-field getter APIs
- No blob transport reintroduction
- No unwrap/expect in production MCP flow
- No empty-index return when stale snapshot exists
- No TODO/stub implementations — every path must be fully implemented
- No skipping engine cleanup in `unindex_root_from_realm` — leaked engines = leaked Zig heap memory
- FORBIDDEN: implementing only happy path and skipping fallback chain. Both stale-first and scan-second fallbacks are required.
- FORBIDDEN: testing only happy path. Each failure mode (create, update, conversion) must have a dedicated test.
- FORBIDDEN: keeping `Md4cScanBackend` as the primary path and adding engine as "optional" — engine IS the primary path, scan is fallback only.

## Log

- [2026-03-23T16:37:52Z] [Seth] Phase 3 complete. Replaced scan-based MCP markdown indexing with persistent DocumentEngine + CEngineResult path. RealmData now holds HashMap<String, Mutex<DocumentEngine>>. Fallback chain: stale engine snapshot → scan (no stale). Engine cleanup wired into unindex_root_from_realm. 5 new tests, 209 total MCP tests pass. Clippy clean. Committed 3116d80.
