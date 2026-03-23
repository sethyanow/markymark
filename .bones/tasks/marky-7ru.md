---
id: marky-7ru
title: 'Phase 4.4: Delete markymark-core scanner module (ScanBackend + impls)'
status: open
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

1. **Update brza_kernels.rs benchmark** — Delete the correctness assertion block (lines ~420-443) that uses `Md4cScanBackend.scan_headings()` to compare heading counts against tree-sitter. The `md4c_extract_only` benchmark already validates md4c extraction works. Remove `Md4cScanBackend` and `ScanBackend` imports from the file. Keep `md4c_backend` variable deletion clean (it's only used by the assertion and the deleted md4c_from_scan benchmark).

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
- The `zig-kernels` feature flag in markymark-core may become partially hollow after scanner deletion — it might only gate other scanner-unrelated items. Check what else is behind this feature flag.

## Success Criteria

- [ ] ScanBackend trait deleted (markymark-core/src/scanner/mod.rs)
- [ ] Md4cScanBackend and ZigScanBackend deleted (markymark-core/src/scanner/md4c.rs)
- [ ] Scanner types deleted (markymark-core/src/scanner/types.rs)
- [ ] Scanner tests deleted (markymark-core/src/scanner/tests.rs)
- [ ] Scanner re-exports removed from markymark-core prelude
- [ ] No ScanBackend/Md4cScanBackend/ZigScanBackend references remain in workspace
- [ ] cargo test --workspace passes
- [ ] cargo clippy --all-targets passes

## Anti-Patterns

- FORBIDDEN: deleting markymark-kernels/src/md4c/ module in this task — separate scope (still used by benchmarks)
- FORBIDDEN: deleting from_blob/ or blob paths — separate scope
- No keeping scanner types "for compatibility" — zero consumers remain
- No partial deletion (e.g., keeping types.rs "for reference") — the module is dead, delete it all
