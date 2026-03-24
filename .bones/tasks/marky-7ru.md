---
id: marky-7ru
title: 'Phase 4.4: Delete markymark-core scanner module (ScanBackend + impls)'
status: closed
type: task
priority: 2
parent: marky-0xtn
---




## Context

- Phase 4.3 (marky-zcj) deleted from_scan.rs — the only production consumer of ScanBackend.
- The scanner module in markymark-core now has zero production consumers. Its only references are: its own tests, the brza_kernels.rs benchmark correctness assertion, and prelude re-exports.
- Md4cScanBackend wraps `markymark_kernels::md4c::extract_md4c()`. Deleting Md4cScanBackend does NOT require deleting the md4c module (separate task scope).
- ZigScanBackend also wraps md4c extraction (despite the name — it uses the same FFI path).

**Blocked by:** marky-zcj (closed — from_scan.rs deleted, scanner module's only production consumer removed)
**Unlocks:** Epic criterion 8 fully satisfied ("from_scan.rs, from_ast.rs, ScanBackend trait — ALL deleted"). Scanner types removed from markymark-core public API.

## Requirements

- R6 (from epic): All CMd4cResult code deleted — ScanBackend trait is part of this (it's the Rust-side abstraction over CMd4c extraction)
- This task covers: delete entire markymark-core/src/scanner/ module, remove prelude re-exports, clean up benchmark references

## Implementation

1. **Update brza_kernels.rs benchmark** — Remove `Md4cScanBackend` and `ScanBackend` imports (line 4). Delete the `md4c_backend` variable (line 408) and the entire correctness assertion block (lines 408-431) that uses `Md4cScanBackend.scan_headings()` to compare heading counts against tree-sitter. The `md4c_extract_only` benchmark (line 459-470) already validates md4c extraction works and calls `markymark_kernels::md4c::extract_md4c()` directly — unaffected by scanner deletion.

2. **Delete scanner test file** — Remove `markymark-core/src/scanner/tests.rs` (~32 Md4cScanBackend test references).

3. **Delete scanner md4c implementation** — Remove `markymark-core/src/scanner/md4c.rs` (Md4cScanBackend + ZigScanBackend impl).

4. **Delete scanner types** — Remove `markymark-core/src/scanner/types.rs` (HeadingResult, LinkResult, TagResult, BlockIdResult, ScanAllResult, ScanError, CodeSpanResult, TaskResult, EmbedResult, CalloutResult, BlockRefResult, QueryBlockResult, LinkDefinitionResult, PropertyResult, XmlTagResult, ScanLinkType).

5. **Delete scanner mod.rs** — Remove `markymark-core/src/scanner/mod.rs` (ScanBackend trait definition, re-exports).

6. **Update markymark-core/src/lib.rs** — Remove `mod scanner;` declaration. Remove prelude re-exports: `ZigScanBackend` (line 169), `ScanBackend`, `ScanAllResult`, `ScanError` (line 170).

7. **Check for cascading dead code** — After removing scanner module, check if any other markymark-core types/functions become dead. Run `cargo check` and address any unused import/dead_code warnings.

8. **Verify**: `cargo test --workspace`, `cargo clippy --all-targets`, zero ScanBackend/Md4cScanBackend/ZigScanBackend references in the workspace.

## Key Considerations

- The scanner module has a `#[cfg(feature = "zig-kernels")]` gate on the md4c submodule and its re-exports. Deletion removes the entire module regardless of feature flags — no conditional cleanup needed.
- Scanner types (HeadingResult etc.) are re-exported via `pub use types::*` in scanner/mod.rs. Any external consumers would break. Verified: zero external consumers remain after from_scan.rs deletion.
- The brza_kernels.rs benchmark will still have `md4c_extract_only` which calls `markymark_kernels::md4c::extract_md4c()` directly — this does NOT go through ScanBackend and is unaffected.
- After this task, the `markymark-kernels/src/md4c/` module remains (used by md4c_extract_only benchmark). That's a separate deletion task.
- **SRE-verified (2026-03-23):** The `zig-kernels` feature flag in markymark-core becomes COMPLETELY hollow after scanner deletion — all 10 `cfg(feature = "zig-kernels")` references are in the scanner module. The feature still activates the `markymark-kernels` optional dep (used by markymark-index and markymark-kernels Cargo.toml), but no code in markymark-core consumes it. Cleanup deferred to a future task — not a blocker for this deletion.

## Success Criteria

- [x] ScanBackend trait deleted (markymark-core/src/scanner/mod.rs)
- [x] Md4cScanBackend and ZigScanBackend deleted (markymark-core/src/scanner/md4c.rs)
- [x] Scanner types deleted (markymark-core/src/scanner/types.rs)
- [x] Scanner tests deleted (markymark-core/src/scanner/tests.rs)
- [x] Scanner re-exports removed from markymark-core prelude
- [x] No ScanBackend/Md4cScanBackend/ZigScanBackend references remain in workspace
- [x] cargo test --workspace passes (1085 passed, 0 failed)
- [x] cargo clippy --all-targets passes (zero warnings with -D warnings)

## Anti-Patterns

- FORBIDDEN: deleting markymark-kernels/src/md4c/ module in this task — separate scope (still used by benchmarks)
- FORBIDDEN: deleting from_blob/ or blob paths — separate scope
- No keeping scanner types "for compatibility" — zero consumers remain
- No partial deletion (e.g., keeping types.rs "for reference") — the module is dead, delete it all

## Log

- [2026-03-23T20:39:33Z] [Seth] Debrief: Clean deletion — 4 scanner files + tests deleted, brza_kernels.rs assertion block removed, prelude re-exports cleaned. All architecture claims verified by SRE. Net -1368 lines. Reflections: No surprises — zero cascading dead code, skeleton matched reality except minor line number offset (corrected during SRE). zig-kernels feature in markymark-core now completely hollow (all cfg references were in scanner). Next task marky-qu1 scoped: blob serialization path deletion (~3,600 lines across Rust from_blob + Zig serialize/blob).
