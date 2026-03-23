---
id: marky-ibt
title: 'Phase 4.6: Delete CMd4c FFI types and md4c Rust module'
status: closed
type: task
priority: 2
owner: Seth
parent: marky-0xtn
---





## Context

- Phase 4.5 (marky-qu1) deleted the blob serialization path. The CMd4c FFI path is the next deletion target.
- The CMd4c path was the original "stateless extraction" FFI: Zig md4c parser → CMd4cResult struct → Rust `extract_md4c()` → `Md4cExtraction`. This was used by from_scan (now deleted) for MCP batch and LSP fallback.
- All consumers now use DocumentEngine + CEngineResult. Zero production callers of `extract_md4c` or CMd4c types remain — only a benchmark (`brza_kernels.rs`) and the Zig export tests.
- The md4c **parser** and **extraction_renderer** stay — they're used internally by DocumentEngine. Only the FFI export layer (exports.zig, ffi_types.zig) and its Rust mirrors (md4c module) are deleted.
- Deletion order: Rust consumers first (remove extern block + module), then Zig producers (remove exports + types). Same pattern as Phase 4.5.

**Blocked by:** marky-qu1 (closed — blob path deleted)
**Unlocks:** Epic criteria 7 and 9 fully satisfied. Leaves only criteria 1-2 (CEngineResult validation) and criteria 11-14 (generation, reserved fields, tests, hooks) as remaining work.

## Requirements

- R5 continuation (from epic): ffi_types.zig CMd4c* types, exports.zig extract/free — ALL deleted (criterion 7)
- R5 continuation (from epic): markymark-kernels/src/md4c/ module — ALL deleted (criterion 9)
- Anti-pattern (from epic): NO deleting ExtractionRenderer — used internally by DocumentEngine

## Implementation

**Rust-first, then Zig.** Within Zig: remove references BEFORE deleting files.

1. **Delete Rust md4c module** — Remove entire `markymark-kernels/src/md4c/` directory (mod.rs 743L, tests.rs 636L).

2. **Update markymark-kernels/src/lib.rs** — Remove `pub mod md4c;` declaration (line 22). Remove `pub use md4c::{extract_md4c, Md4cExtraction, Md4cHeading, Md4cLink};` re-export (line 27). Remove the `//! - [`md4c`]` doc comment (line 15).

3. **Remove md4c benchmark from brza_kernels.rs** — Remove the `md4c_extract_only` benchmark arm (lines 432-443). Rename `bench_md4c_vs_tree_sitter` → `bench_engine_from_text` (line 401). Update `criterion_group!` macro entry (line 457) to use the new name.

4. **Verify Rust compiles** — `cargo check --workspace --all-targets`. Fix any dead imports, unused warnings.

5. **Remove md4c export imports from c_adapter.zig** — Remove `_ = @import("md4c/exports.zig");` from the comptime export block (line 28) and from the test block (line 991). Remove the `// md4c FFI exports + tests` comment (line 990).

6. **Remove FFI roundtrip test from extraction_renderer_tests_xml_tags.zig** — Delete the "xml_tags: FFI roundtrip via marky_md4c_extract" test (lines 213-239) and its section comment (line 213). This test uses `exports.marky_md4c_extract` which will no longer exist. Keep all other xml_tags tests — they test the extraction_renderer directly.

7. **Delete Zig CMd4c FFI files** — Remove `zig/src/md4c/ffi_types.zig` (166L), `zig/src/md4c/exports.zig` (661L), `zig/src/md4c/exports_tests.zig` (247L). References were already removed in steps 5-6.

8. **Check for cascading dead code** — Run `cargo check --workspace --all-targets`. Run `zig build test`. Address any dead code warnings, unused imports, or orphaned types.

9. **Verify** — `cargo test --workspace` (all pass), `cargo clippy --all-targets --workspace` (clean), `zig build test` (exit 0), verify zero CMd4c/marky_md4c_extract/marky_md4c_free references remain in workspace (excluding internal ExtractionRenderer which uses different types).

## Key Considerations

- The ExtractionRenderer and its types (StoredHeading, StoredLink, etc.) are INTERNAL to DocumentEngine. Only the FFI projection types (CMd4c*) and export functions are deleted. The `zig/src/md4c/` directory retains all files except exports.zig, exports_tests.zig, and ffi_types.zig.
- The `markymark-kernels/src/md4c/mod.rs` contains both the CMd4c repr(C) mirrors AND the `extract_md4c()` function with its `Md4cExtraction` wrapper. All are deleted — `extract_md4c` has zero production callers.
- The benchmark `bench_md4c_vs_tree_sitter` has two arms: `engine_from_text` (stays — uses CEngineResult path) and `md4c_extract_only` (goes — uses deleted CMd4c path). **Decision (SRE):** Rename function to `bench_engine_from_text`, keep engine arm, update criterion_group entry. The engine arm exercises the live CEngineResult path and is not redundant with `bench_bulk_reindex` (which measures multi-doc reindex throughput, not single-doc extraction latency).
- `extraction_renderer_tests_xml_tags.zig` has one test (line 215) that does an FFI roundtrip via `marky_md4c_extract`. This test must be deleted. All other tests in that file test the renderer directly and stay. No other test in the file imports `exports.zig`.
- After this task, the only FFI exports for document extraction are `marky_engine_*` (create, update, get_result, free_result, destroy). The parallel CMd4c stateless path is gone.

### SRE Adversarial Findings

**Dependency Treachery: Zig file deletion ordering**
- Assumption: Steps can be executed sequentially as numbered
- Betrayal: `c_adapter.zig` and `extraction_renderer_tests_xml_tags.zig` import `md4c/exports.zig`. Deleting the file before removing imports breaks `zig build test`
- Consequence: Broken intermediate build between steps
- Mitigation: Reordered steps 5→6→7 to 5(remove refs)→6(remove refs)→7(delete files)

**Dependency Treachery: Benchmark criterion_group macro**
- Assumption: Removing the benchmark arm is sufficient
- Betrayal: If function is renamed but criterion_group entry isn't updated, compilation fails
- Consequence: Build error on `cargo bench`
- Mitigation: Step 3 explicitly requires updating criterion_group entry with new name

**Encoding Boundaries: Benchmark function naming**
- Assumption: Keeping old name `bench_md4c_vs_tree_sitter` with only engine arm is acceptable
- Betrayal: Name becomes semantically misleading — function no longer compares md4c vs tree-sitter
- Consequence: Future confusion about what the benchmark measures
- Mitigation: Rename to `bench_engine_from_text` — name matches the one remaining benchmark ID

## Success Criteria

- [x] markymark-kernels/src/md4c/ directory deleted (mod.rs, tests.rs)
- [x] md4c module declaration and re-exports removed from lib.rs
- [x] md4c_extract_only arm removed, function renamed to bench_engine_from_text, criterion_group updated
- [x] zig/src/md4c/ffi_types.zig deleted
- [x] zig/src/md4c/exports.zig deleted
- [x] zig/src/md4c/exports_tests.zig deleted
- [x] md4c/exports.zig imports removed from c_adapter.zig
- [x] FFI roundtrip test removed from extraction_renderer_tests_xml_tags.zig
- [x] No CMd4c*/marky_md4c_extract/marky_md4c_free references remain in workspace
- [x] ExtractionRenderer and its internal types preserved (NOT deleted)
- [x] cargo test --workspace passes (983/983)
- [x] cargo clippy --all-targets passes (clean with -D warnings)
- [x] zig build test passes

## Anti-Patterns

- FORBIDDEN: deleting extraction_renderer.zig or any internal md4c parser files — DocumentEngine depends on them
- FORBIDDEN: deleting StoredHeading, StoredLink, or other engine-internal types — only CMd4c* projection types go
- FORBIDDEN: deleting engine/ffi_types.zig or engine/exports.zig — those are the CEngineResult path (the REPLACEMENT)
- FORBIDDEN: keeping CMd4c types "for future use" — zero consumers since Phase 3
- FORBIDDEN: deleting the `engine_from_text` benchmark arm when removing the md4c benchmark — it exercises the live CEngineResult path
