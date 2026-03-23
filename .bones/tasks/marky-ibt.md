---
id: marky-ibt
title: 'Phase 4.6: Delete CMd4c FFI types and md4c Rust module'
status: open
type: task
priority: 2
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

1. **Delete Rust md4c module** — Remove entire `markymark-kernels/src/md4c/` directory (mod.rs 743L, tests.rs 636L).

2. **Update markymark-kernels/src/lib.rs** — Remove `pub mod md4c;` declaration. Remove `pub use md4c::{extract_md4c, Md4cExtraction, Md4cHeading, Md4cLink};` re-export (line 27).

3. **Remove md4c benchmark from brza_kernels.rs** — Remove the `md4c_extract_only` benchmark arm from `bench_md4c_vs_tree_sitter` (lines 432-443). Either rename the function to reflect it's now engine-only, or remove the entire function if `engine_from_text` benchmarking is already covered by `bench_bulk_reindex`. Remove `bench_md4c_vs_tree_sitter` from the `criterion_group!` macro (line 457) if the function is removed entirely.

4. **Verify Rust compiles** — `cargo check --workspace --all-targets`. Fix any dead imports, unused warnings.

5. **Delete Zig CMd4c FFI files** — Remove `zig/src/md4c/ffi_types.zig` (166L), `zig/src/md4c/exports.zig` (661L), `zig/src/md4c/exports_tests.zig` (247L).

6. **Remove md4c export imports from c_adapter.zig** — Remove `_ = @import("md4c/exports.zig");` from the comptime export block (line 28) and from the test block (line 991). Remove the `// md4c FFI exports + tests` comment (line 990).

7. **Remove FFI roundtrip test from extraction_renderer_tests_xml_tags.zig** — Delete the "xml_tags: FFI roundtrip via marky_md4c_extract" test (lines 213-239) and its section comment (line 213). This test uses `exports.marky_md4c_extract` which will no longer exist. Keep all other xml_tags tests — they test the extraction_renderer directly.

8. **Check for cascading dead code** — Run `cargo check --workspace --all-targets`. Run `zig build test`. Address any dead code warnings, unused imports, or orphaned types.

9. **Verify** — `cargo test --workspace` (all pass), `cargo clippy --all-targets --workspace` (clean), `zig build test` (exit 0), verify zero CMd4c/marky_md4c_extract/marky_md4c_free references remain in workspace (excluding internal ExtractionRenderer which uses different types).

## Key Considerations

- The ExtractionRenderer and its types (StoredHeading, StoredLink, etc.) are INTERNAL to DocumentEngine. Only the FFI projection types (CMd4c*) and export functions are deleted. The `zig/src/md4c/` directory retains all files except exports.zig, exports_tests.zig, and ffi_types.zig.
- The `markymark-kernels/src/md4c/mod.rs` contains both the CMd4c repr(C) mirrors AND the `extract_md4c()` function with its `Md4cExtraction` wrapper. All are deleted — `extract_md4c` has zero production callers.
- The benchmark `bench_md4c_vs_tree_sitter` has two arms: `engine_from_text` (stays — uses CEngineResult path) and `md4c_extract_only` (goes — uses deleted CMd4c path). Decision: keep `engine_from_text` arm under a renamed function, or remove entirely if redundant with `bench_bulk_reindex`.
- `extraction_renderer_tests_xml_tags.zig` has one test (line 215) that does an FFI roundtrip via `marky_md4c_extract`. This test must be deleted. All other tests in that file test the renderer directly and stay.
- After this task, the only FFI exports for document extraction are `marky_engine_*` (create, update, get_result, free_result, destroy). The parallel CMd4c stateless path is gone.

## Success Criteria

- [ ] markymark-kernels/src/md4c/ directory deleted (mod.rs, tests.rs)
- [ ] md4c module declaration and re-exports removed from lib.rs
- [ ] md4c benchmark removed or converted in brza_kernels.rs
- [ ] zig/src/md4c/ffi_types.zig deleted
- [ ] zig/src/md4c/exports.zig deleted
- [ ] zig/src/md4c/exports_tests.zig deleted
- [ ] md4c/exports.zig imports removed from c_adapter.zig
- [ ] FFI roundtrip test removed from extraction_renderer_tests_xml_tags.zig
- [ ] No CMd4c*/marky_md4c_extract/marky_md4c_free references remain in workspace
- [ ] ExtractionRenderer and its internal types preserved (NOT deleted)
- [ ] cargo test --workspace passes
- [ ] cargo clippy --all-targets passes
- [ ] zig build test passes

## Anti-Patterns

- FORBIDDEN: deleting extraction_renderer.zig or any internal md4c parser files — DocumentEngine depends on them
- FORBIDDEN: deleting StoredHeading, StoredLink, or other engine-internal types — only CMd4c* projection types go
- FORBIDDEN: deleting engine/ffi_types.zig or engine/exports.zig — those are the CEngineResult path (the REPLACEMENT)
- FORBIDDEN: keeping CMd4c types "for future use" — zero consumers since Phase 3
- FORBIDDEN: deleting the `engine_from_text` benchmark arm when removing the md4c benchmark — it exercises the live CEngineResult path
