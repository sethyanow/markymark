---
id: marky-llj
title: 'Phase 4.1: Add DocumentIndex::from_text() and migrate from_ast callers'
status: closed
type: task
priority: 2
parent: marky-0xtn
---





## Context

- Phases 1-3 of marky-0xtn are complete. All consumers (LSP, MCP) use persistent DocumentEngine + CEngineResult.
- Phase 4 is the deletion sweep. Before deleting from_ast/from_scan/from_blob, tests need a replacement constructor.
- `from_text()` = ephemeral DocumentEngine + CEngineResult. No blob, no scan, no tree-sitter AST.
- 18 `from_ast` call sites across 11 files (all tests/benches). All use the pattern: `parse(source).unwrap()` → `from_ast(ast)`.
- `from_text(source)` collapses that to a single call: parse frontmatter in Rust, mask, ephemeral engine, get_result, from_engine_result_with_frontmatter, drop engine.

**Blocked by:** marky-xfgb (closed — Phase 3 done)
**Unlocks:** from_ast module deletion, from_scan test caller migration, and ultimately Phase 4 blob/CMd4c cleanup

## Requirements

- R9 (from epic): DocumentIndex::from_text() convenience function replaces from_ast/from_scan/from_blob in all tests.
- This task covers: add from_text() + migrate the 18 from_ast callers. from_scan callers are a separate task.

## Implementation

**Primary file:** `markymark-index/src/document/from_engine.rs` (add `from_text()`)
**Reference:** `markymark-mcp/src/engine/mod.rs` (same pattern in `build_markdown_index_via_engine`)

1. Add `DocumentIndex::from_text(text: &str) -> Self` to `from_engine.rs`.
   - Signature: `pub fn from_text(text: &str) -> Self`
   - Parse frontmatter: `let (fm, aliases) = crate::parse_frontmatter_owned(text);`
   - Mask frontmatter: `let masked = crate::mask_frontmatter(text);`
   - Ephemeral engine: `DocumentEngine::new(&masked).expect("from_text: engine create")`
   - Get result: `engine.get_result().expect("from_text: get_result")`
   - Convert: `result.to_extraction().expect("from_text: to_extraction")`
   - Build: `Self::from_engine_result_with_frontmatter(&extraction, fm, aliases)`
   - Engine dropped on return (Zig cleanup via Drop).
   - Note: `expect()` is appropriate — this is a test convenience, not production code.

2. Re-export `from_text` from `markymark-index/src/lib.rs` (same path as `from_ast`, `from_scan`).

3. Write a test for `from_text()` in `markymark-index/src/document/tests/`:
   - Verify headings, links, tags extracted from a mixed markdown document.
   - Verify frontmatter preserved.
   - Verify empty input produces empty index (not panic).

4. Migrate from_ast callers (18 references across 11 files):
   - `markymark-index/src/document/tests/mod.rs` (lines 12, 205): replace `parse(s).unwrap()` → `from_ast(ast)` with `from_text(s)`.
   - `markymark-index/src/document/tests/scan_tests.rs` (line 14): same pattern.
   - `markymark-index/src/realm/tests.rs` (lines 9, 29): same pattern.
   - `markymark-index/tests/realm_index.rs` (line 11): same pattern.
   - `markymark-index/tests/resolution.rs` (line 13): same pattern.
   - `markymark-index/tests/document_index.rs` (line 12): same pattern.
   - `markymark-mcp/src/engine/search.rs` (line 209): test helper `make_index()`.
   - `markymark-mcp/src/graph.rs` (line 258): test helper `make_index()`.
   - `markymark-kernels/benches/brza_kernels.rs` (lines 406, 477): bench helpers.
   - `markymark-index/benches/memory.rs` (lines 128, 138, 262, 278): bench helpers.
   - `markymark-index/benches/realm_update.rs` (lines 106, 120): bench helpers.
   - Each migration: remove `markymark_parser::parse` import, add `DocumentIndex::from_text` import, replace `parse(s).unwrap()` → `DocumentIndex::from_ast(ast)` with `DocumentIndex::from_text(s)`.

5. Verify: `cargo test --workspace` passes, `cargo clippy --workspace -- -D warnings` passes.

## Key Considerations

- `from_text()` uses `expect()` internally — acceptable for test convenience, NOT for production. Document this in the function's doc comment.
- `DocumentEngine::new("")` works (re-verified: `test_engine_empty_input` passes, 2026-03-23). Empty input should produce empty index, not panic.
- markymark-index already depends on markymark-kernels (Cargo.toml line 19) — no new dependency needed for DocumentEngine access in benches or tests.
- The `markymark-parser` import (`parse()`) can be removed from migrated test files since `from_text()` handles parsing internally.
- After this task, `from_ast` should have ZERO callers, making the `from_ast.rs` module deletable in a follow-up task.
- **SRE: Line numbers in step 4 are approximate** — from the prior session before Phase 3 commits. Use LSP findReferences on `from_ast` to get current positions.
- **SRE: Frontmatter-only edge case** — test should cover input that is entirely frontmatter (e.g., `---\ntitle: x\n---\n`) with no markdown body. Expect: frontmatter entries populated, headings/links/tags empty.

## Success Criteria

- [x] `DocumentIndex::from_text(text: &str) -> Self` exists and works for empty, frontmatter-only, and mixed markdown
- [x] All 18 from_ast call sites migrated to from_text (verified: zero from_ast callers in tests/benches)
- [x] from_text is re-exported from markymark-index crate root
- [x] Test for from_text verifies headings, frontmatter, and empty input
- [x] cargo test --workspace passes
- [x] cargo clippy --workspace -- -D warnings passes

## Anti-Patterns

- No expect/unwrap in production code — from_text() is test-only convenience
- No keeping from_ast callers "for now" — all 18 must be migrated
- No adding from_text to production paths — production uses from_engine_result_with_frontmatter directly
- FORBIDDEN: leaving markymark_parser::parse imports in migrated files — from_text() internalizes the parse step

## Log

- [2026-03-23T17:46:38Z] [Seth] Completed: from_text() added to from_engine.rs (6 lines, ephemeral DocumentEngine). All 18 from_ast callers migrated across 11 files. Zero from_ast callers remain. Slug difference discovered: engine slugifies @ as - vs Rust strips it — updated test expectation. Pre-existing unused_mut warning fixed in realm_isolation.rs. All tests pass (workspace), clippy clean.
- [2026-03-23T17:51:01Z] [Seth] Debrief: from_text() implemented in 6 lines, 18 callers migrated mechanically. Slug difference discovered (engine @ → -, Rust strips @) — test expectation updated. Pre-existing clippy warning fixed. Reflections: skeleton accuracy excellent — only inaccuracy was the Cargo.toml dependency concern (already existed). Next task scoped: marky-ut8 (Phase 4.2 — from_scan_with_frontmatter production callers + from_ast deletion).
