---
id: marky-zcj
title: 'Phase 4.3: Migrate from_scan test callers to from_text + delete from_scan.rs'
status: open
type: task
priority: 2
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

   Callers to triage (from LSP findReferences):
   - `scan_tests.rs` — 1 caller (line 8). Check: does it import from_scan for tests or just use it?
   - `md4c_scan_tests.rs` — 3 callers (lines 8, 36, 37). Likely testing the Md4c scan backend specifically — candidate for DELETE.
   - `incremental_tests.rs` — 1 caller (line 61). Check what it tests.
   - `parity_tests.rs` — 2 callers (lines 24, 190). Scan-vs-blob parity — candidate for DELETE (both paths being removed).
   - `feature_tests.rs` — 4 callers (lines 102, 256, 279, 355). Check: testing index features or scan-specific behavior?
   - `brza_kernels.rs` — 2 callers (lines 393, 462). Benchmarks — candidate for RESTRUCTURE.

2. **For each MIGRATE caller**: Replace `DocumentIndex::from_scan(text, &Md4cScanBackend)` with `DocumentIndex::from_text(text)`. Remove `ScanBackend`/`Md4cScanBackend` imports if no longer needed in the file.

3. **For each DELETE caller**: Remove the entire test function. If the entire test file becomes empty, delete the file and its mod declaration.

4. **For benchmarks**: If they compare scan-vs-engine, keep only the engine path benchmark. If they test scan-only, delete.

5. **Delete from_scan.rs**:
   - Remove `markymark-index/src/document/from_scan.rs`
   - Remove `mod from_scan;` from `document/mod.rs`

6. **Clean up stale comment** in `markymark-lsp/src/state/mod.rs` — line referencing "from_scan with Md4cScanBackend" in a comment (no code dependency, just stale documentation).

7. **Verify**: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, zero `from_scan` references in markymark-index (the function, not the string literal in docs/comments).

## Key Considerations

- Some tests may use `from_scan` but actually test generic DocumentIndex behavior (headings, links). These should be migrated, not deleted — they provide coverage.
- `scan_tests.rs` and `md4c_scan_tests.rs` may be entirely scan-path tests. If so, entire files are deleted.
- `parity_tests.rs` tests scan-vs-blob parity. Both paths are being removed. However, some parity tests might have value as engine-vs-blob parity — check before deleting.
- `feature_tests.rs` likely tests DocumentIndex features via the scan constructor. These should mostly migrate to from_text.
- Benchmarks in `brza_kernels.rs` that use from_scan may compare BRZA scan performance. After from_scan is deleted, these benchmarks become dead code. Check if they benchmark the scan path or the BRZA kernels directly.
- After this task, `ScanBackend` trait references will remain in markymark-core (trait definition + Md4cScanBackend impl + scanner tests) — those are a separate deletion task.

## Success Criteria

- [ ] All from_scan callers in test/bench code migrated to from_text or deleted (with reasoning)
- [ ] from_scan.rs deleted — file removed, mod declaration removed
- [ ] No from_scan function references remain in markymark-index (excluding comments)
- [ ] Stale "from_scan with Md4cScanBackend" comment updated in LSP state/mod.rs
- [ ] cargo test --workspace passes
- [ ] cargo clippy --workspace -- -D warnings passes

## Anti-Patterns

- No blanket deletion of test files without checking what behavior they test — some tests verify DocumentIndex features through from_scan and should be migrated, not deleted
- No keeping from_scan.rs "for reference" — it has zero production callers, delete it
- FORBIDDEN: deleting ScanBackend trait or markymark-core scanner module in this task — separate scope
- FORBIDDEN: deleting from_blob/ or blob.zig in this task — separate scope
- No changing from_text semantics — it's a proven, tested convenience function
