---
id: marky-zcj
title: 'Phase 4.3: Migrate from_scan test callers to from_text + delete from_scan.rs'
status: closed
type: task
priority: 2
owner: Seth
parent: marky-0xtn
---




## Context

- Phase 4.2 (marky-ut8) migrated all production callers of `from_scan_with_frontmatter` to `from_text` and deleted `from_ast.rs`. `from_scan_with_frontmatter` now has zero callers (only the definition in from_scan.rs remains).
- `from_scan` (no frontmatter) has 13 callers in test/bench code across 6 files. Zero production callers.
- The `ScanBackend` trait and `Md4cScanBackend` impl remain in markymark-core — used by from_scan.rs and test code. Deleting from_scan.rs will make them eligible for removal (separate task scope).
- `from_scan_inner` is internal to from_scan.rs (called by both from_scan and from_scan_with_frontmatter).

**Blocked by:** marky-ut8 (closed — from_scan_with_frontmatter has zero callers)
**Unlocks:** from_scan.rs deletion, which makes ScanBackend trait + Md4cScanBackend + scanner module eligible for deletion. Partial progress toward epic criterion "from_scan.rs, from_ast.rs, ScanBackend trait — ALL deleted."

## Requirements

- R9 (from epic): DocumentIndex::from_text() convenience function replaces from_ast/from_scan/from_blob in all tests.
- This task covers: migrate all from_scan test/bench callers to from_text (or delete tests that ONLY test the scan path), then delete from_scan.rs.

## Implementation

1. **Triage each from_scan caller** — categorize callers into:
   - **MIGRATE**: Tests that verify DocumentIndex output (headings, links, frontmatter, etc.) — change `from_scan` to `from_text`. These test index behavior, not the scan path.
   - **DELETE**: Tests that specifically verify `ScanBackend`/`Md4cScanBackend` behavior or scan-vs-blob parity via the scan path. These test a code path being deleted.
   - **RESTRUCTURE**: Benchmarks comparing scan-vs-engine performance. These may need restructuring or deletion.

   SRE-verified triage (LSP findReferences confirmed 13 callers in 6 files, 2026-03-23):

   - `scan_tests.rs` — MIGRATE. 1 from_scan call in `build_index_from_scan` helper (line 8), used by ~15 tests. Most test DocumentIndex features (headings, links, tags, toc, ranges). Change helper to use `from_text`. DELETE `test_parity_headings` (line 106) — it compares scan vs engine, becomes tautological after migration. Review `test_from_engine_unchanged` (line 98) — may be redundant if it duplicates migrated tests.
   - `md4c_scan_tests.rs` — DELETE entire file. All 6 tests test Md4cScanBackend specifically (headings, links, parity). After migration these would duplicate scan_tests.rs coverage. Remove mod declaration in tests/mod.rs.
   - `incremental_tests.rs` — DELETE entire `scan_all_fallback_tests` module (lines 7-73), including `FailingScanAllBackend` struct. Tests from_scan's scan_all fallback behavior — a code path being deleted. Other tests in this file (test_embeds_from_ast etc.) are unaffected — they use `build_index` helper (from_text).
   - `parity_tests.rs` — DELETE 2 test functions: `test_from_blob_parity_with_from_scan` (line 8), `test_from_blob_xml_tags_parity_with_scan` (line 182). Both test blob-vs-scan parity. Keep 3 remaining tests that test blob behavior directly.
   - `feature_tests.rs` — DELETE 4 test functions: `test_from_blob_code_span_parity_with_from_scan` (line 93), `test_from_blob_callout_parity_with_from_scan` (line 248), `test_from_blob_block_ref_parity_with_from_scan` (line 271), `test_from_blob_properties_match_from_scan` (line 349). All are blob-vs-scan parity tests. Keep remaining 23 blob tests.
   - `brza_kernels.rs` — DELETE 2 scan benchmark functions: `zig_scan_backend_600_docs` (line 389), `md4c_from_scan` (line 460). Keep engine benchmarks (`engine_from_text_600_docs`, `engine_from_text`). Update criterion group names if needed after removing scan variants.

2. **For each MIGRATE caller**: Replace `DocumentIndex::from_scan(text, &Md4cScanBackend)` with `DocumentIndex::from_text(text)`. Remove `ScanBackend`/`Md4cScanBackend` imports if no longer needed in the file.

3. **For each DELETE caller**: Remove the entire test function. If the entire test file becomes empty, delete the file and its mod declaration.

4. **For benchmarks**: If they compare scan-vs-engine, keep only the engine path benchmark. If they test scan-only, delete.

5. **Delete from_scan.rs**:
   - Remove `markymark-index/src/document/from_scan.rs`
   - Remove `mod from_scan;` from `document/mod.rs`

6. **Clean up stale comment** in `markymark-lsp/src/state/mod.rs` — line referencing "from_scan with Md4cScanBackend" in a comment (no code dependency, just stale documentation).

7. **Verify**: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, zero `from_scan` references in markymark-index (the function, not the string literal in docs/comments).

## Key Considerations

- `scan_tests.rs` tests DocumentIndex features through a `build_index_from_scan` helper. Most tests verify headings, links, tags, ranges — generic behavior. Migrate by switching helper to from_text. BUT `test_parity_headings` becomes tautological (scan-vs-engine → from_text-vs-from_text) — delete it.
- `md4c_scan_tests.rs` is entirely Md4cScanBackend tests. Delete the whole file — after migration, test coverage would duplicate scan_tests.rs.
- `incremental_tests.rs` contains `scan_all_fallback_tests` module (lines 7-73) with a custom `FailingScanAllBackend` struct. Delete the entire module, not just the from_scan call — the struct and all its ScanBackend impl methods are dead after from_scan.rs is deleted.
- `parity_tests.rs` and `feature_tests.rs` are in `from_blob/tests/` — delete individual blob-vs-scan parity test functions only. Other blob-only tests stay (they'll be deleted when from_blob/ is removed in a later task).
- `brza_kernels.rs` benchmarks compare scan-vs-engine in criterion groups. Delete scan benchmark functions, keep engine variants. Check if criterion group setup needs cleanup after removing scan variants (e.g., unnecessary `md4c_backend` variable declarations).
- After this task, `ScanBackend` trait references will remain in markymark-core (trait definition + Md4cScanBackend impl + scanner tests + ZigScanBackend) — those are a separate deletion task.
- `ZigScanBackend` and `Md4cScanBackend` imports in test files may be used ONLY by from_scan calls. After migration/deletion, clean up orphaned imports to avoid clippy warnings.

## Success Criteria

- [x] All from_scan callers in test/bench code migrated to from_text or deleted (with reasoning)
- [x] from_scan.rs deleted — file removed, mod declaration removed
- [x] No from_scan function references remain in markymark-index (excluding comments)
- [x] Stale "from_scan with Md4cScanBackend" comment updated in LSP state/mod.rs
- [x] cargo test --workspace passes
- [x] cargo clippy --workspace -- -D warnings passes

## Anti-Patterns

- No blanket deletion of test files without checking what behavior they test — some tests verify DocumentIndex features through from_scan and should be migrated, not deleted
- No keeping from_scan.rs "for reference" — it has zero production callers, delete it
- FORBIDDEN: deleting ScanBackend trait or markymark-core scanner module in this task — separate scope
- FORBIDDEN: deleting from_blob/ or blob.zig in this task — separate scope
- No changing from_text semantics — it's a proven, tested convenience function
- No keeping parity tests that compare two paths that both resolve to from_text after migration — they become tautological (scan-vs-engine → from_text-vs-from_text). Delete them.
- No leaving dead modules (e.g., `scan_all_fallback_tests` in incremental_tests.rs) — if from_scan is deleted, the module's custom ScanBackend impl and all tests inside are dead code
