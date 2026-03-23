---
id: marky-0xtn
title: 'Epic: Eliminate blob serialization — expand CEngineResult FFI pattern'
status: open
type: epic
priority: 2
depends_on: [marky-xfgb, marky-llj, marky-ut8, marky-zcj, marky-7ru, marky-qu1, marky-ibt]
labels: [architecture, blob-removal, ffi]
---















## Context

The Zig DocumentEngine currently serializes all parse state into a flat binary blob (serialize.zig + blob.zig), transfers it across the FFI boundary as raw bytes, and Rust deserializes it back (from_blob/*.rs). This was introduced to avoid hundreds of per-field FFI getter functions.

Since then, the CMd4cResult pattern proved that a single C-ABI struct with pointer arrays + text_blob achieves the same goal with 2 FFI calls and zero serialization. This epic expands that proven pattern to cover everything DocumentEngine produces, eliminating the blob layer entirely.

**Scope expansion (brainstorm 2026-02-23):** After architectural rethink, this epic now eliminates BOTH the blob path AND the CMd4cResult/from_scan path, establishing DocumentEngine as the single extraction+enrichment pipeline for all consumers (LSP, MCP, tests).

**Prerequisite for:** marky-cixz (incremental md4c block-level reparse), which assumes direct FFI access post-blob-removal (R11).

## Current State

Two parallel FFI paths exist:

**Path 1 — Blob (DocumentEngine hot path):**
- Zig: DocumentEngine.parseAll() → serializeState() → flat binary blob
- Rust: engine.get_blob() → from_blob() → DocumentIndex
- FFI surface: 4 functions (create, update, get_blob, destroy)
- Code: ~2,328 lines (serialize.zig 408, blob.zig 595, from_blob/*.rs 1,325)

**Path 2 — CMd4cResult (ExtractionRenderer):**
- Zig: marky_md4c_extract() → C-ABI struct with arrays + text_blob
- Rust: reads CMd4cResult directly via repr(C), convert_result() copies to owned
- FFI surface: 2 functions (extract, free) + 12 repr(C) struct types
- Code: ~900 lines (ffi_types.zig, exports.zig, md4c/mod.rs)
- Used by: from_scan (MCP batch, LSP fallback)

**Key finding (research 2026-02-23):** from_scan and from_blob produce IDENTICAL DocumentIndex features. The blob is pure transport overhead. from_ast has ZERO production callers (test-only).

## Requirements

- R1: CEngineResult C-ABI struct covers ALL DocumentEngine output (13 element types + metadata)
- R2: from_engine_result() produces identical DocumentIndex as from_blob() for all test inputs
- R3: LSP hot path uses CEngineResult instead of blob
- R4: MCP batch path uses persistent DocumentEngines + CEngineResult instead of from_scan
- R5: All blob code deleted (serialize.zig, blob.zig, from_blob/*.rs)
- R6: All CMd4cResult code deleted (CMd4c* types, exports.zig extract/free, md4c module, ScanBackend)
- R7: All existing tests pass (Zig + Rust 1123+)
- R8: No performance regression on LSP did_change path
- R9: DocumentIndex::from_text() convenience function replaces from_ast/from_scan/from_blob in all tests
- R10: Stale index cache in LSP for engine failure fallback
- R11: CEngineResult includes generation counter for future incremental support
- R12: Frontmatter extraction stays in Rust (YAML/TOML, not markdown)

## Design

### Approach: Engine-everywhere

DocumentEngine is the sole source of truth for all DocumentIndex construction.

Before (3 paths):
  LSP:  Engine → serialize → blob → from_blob → DocumentIndex
  MCP:  text → CMd4cResult → from_scan → DocumentIndex
  Test: tree-sitter → from_ast → from_scan → DocumentIndex

After (1 path):
  ALL:  Engine → get_result → CEngineResult → from_engine_result → DocumentIndex

Concern separation:
  Zig DocumentEngine: parsing + extraction + enrichment (positions, slugs, metadata)
  CEngineResult: structured C-ABI transport (no serialization)
  Rust from_engine_result: arena allocation + index construction
  Frontmatter: stays in Rust (YAML/TOML parsing, not markdown)

Lifecycle:
  LSP: persistent engine per URI (existing HashMap)
  MCP: persistent engine per file (new HashMap)
  Fallback: stale cached DocumentIndex on engine failure (new)

### CEngineResult Type (C-ABI contract)

CEngineResult extern struct containing:
- 13 element arrays (pointer + count pairs): headings, links, code_spans, tags, block_ids, tasks, embeds, callouts, block_refs, query_blocks, link_defs, properties, xml_tags
- Metadata: line_starts + count, text_blob + len, token_estimate (u32), content_hash (u64)
- Incrementality: generation (u64, monotonic counter)
- Future: _reserved[32] bytes for partial result fields

### FFI exports

marky_engine_get_result(handle, *CEngineResult) → i32
marky_engine_free_result(*CEngineResult) → void

### Rust side

- repr(C) mirrors of all CEngine* types in markymark-kernels
- DocumentEngine::get_result() → EngineResult wrapper
- DocumentIndex::from_engine_result() replaces from_blob
- DocumentIndex::from_engine_result_with_frontmatter() for MCP
- DocumentIndex::from_text() convenience = ephemeral engine → get_result → from_engine_result → destroy

### Deletion Scope (~4,479 lines)

Zig blob: serialize.zig (408) + blob.zig (595) = ~1,003
Zig CMd4c exports: ffi_types.zig CMd4c* types (~166) + exports.zig extract/free (~660) = ~826
Rust from_blob: from_blob/ directory (929) = ~929
Rust from_scan: from_scan.rs (448) + from_ast.rs (26) = ~474
Rust CMd4c FFI: markymark-kernels/src/md4c/ module = ~747
Rust ScanBackend: markymark-core/src/scanner/ module = ~500

### Addition Scope (~1,230 lines)

Zig CEngine types: engine/ffi_types.zig = ~200
Zig get_result: engine/get_result.zig = ~250
Rust CEngine mirrors: markymark-kernels/src/engine_ffi.rs = ~400
Rust from_engine: markymark-index/src/document/from_engine.rs = ~300
Stale cache + MCP engines: LSP state + MCP engine = ~80

### Net: ~-3,249 lines

## Phasing

### Phase 1: Add CEngineResult alongside existing paths (CLOSED - marky-e0kp)
- CEngine* types in Zig + Rust mirrors
- get_result/free_result exports
- from_engine_result() in Rust
- Parity tests: from_engine_result == from_blob for diverse inputs
- NOTHING DELETED yet

### Phase 2: Switch LSP to CEngineResult (CLOSED - marky-2nxj)
- LSP calls get_result instead of get_blob
- Add stale index cache (last-good DocumentIndex per URI)
- Remove from_scan fallback in LSP

### Phase 3: Switch MCP to persistent engines (OPEN - marky-xfgb)
- MCP creates/updates DocumentEngines per file
- from_engine_result_with_frontmatter for MCP batch
- from_scan no longer called in production

### Phase 4: Delete dead code
- Blob path (serialize.zig, blob.zig, from_blob/)
- CMd4c export path (ffi_types.zig CMd4c*, exports.zig extract/free)
- from_scan, from_ast, ScanBackend, Md4cScanBackend
- md4c module in markymark-kernels
- scanner module in markymark-core
- Add from_text() convenience, migrate all test callers

## Success Criteria

- [ ] CEngineResult struct matches DocumentEngine state exactly (13 types + 3 metadata)
- [ ] Parity tests prove from_engine_result == from_blob for diverse inputs
- [x] LSP hot path uses get_result/from_engine_result (no get_blob) — marky-2nxj
- [x] MCP batch uses persistent engines + from_engine_result (no from_scan) — marky-xfgb
- [x] Stale index cache returns last-good DocumentIndex on engine failure — marky-xfgb
- [x] serialize.zig, blob.zig, from_blob/ — ALL deleted — marky-qu1
- [ ] ffi_types.zig CMd4c* types, exports.zig extract/free — ALL deleted
- [x] from_scan.rs, from_ast.rs, ScanBackend trait — ALL deleted — marky-ut8, marky-zcj, marky-7ru
- [ ] markymark-kernels/src/md4c/ module — ALL deleted
- [x] DocumentIndex::from_text() works for all test cases — marky-llj
- [ ] All tests passing (Zig + Rust)
- [ ] Pre-commit hooks passing
- [ ] generation field present in CEngineResult (u64, monotonic)
- [ ] _reserved[32] bytes in CEngineResult for future incremental fields

## Anti-Patterns

- NO per-field FFI getter functions (CEngineResult solves it with structured C-ABI)
- NO new serialization format (C-ABI structs transfer directly)
- NO keeping from_scan as production fallback (engine-everywhere means one code path; stale index is the fallback)
- NO changing DocumentEngine internal types (CEngine* are FFI projections; StoredHeading etc. stay as-is inside Zig)
- NO frontmatter in Zig (YAML/TOML parsing stays in Rust)
- NO deleting ExtractionRenderer (used internally by DocumentEngine; only the CMd4c export layer on top goes)
- NO dual FFI types in production (CMd4cResult AND CEngineResult coexisting; Phase 1 temporary overlap only)
- NO empty index on engine failure (stale cache means last-good result, never empty)

## Design Rationale

### Approaches Considered

**1. Engine-everywhere with persistent MCP engines (CHOSEN)**
Single CEngineResult FFI type replaces both blob and CMd4cResult. All consumers use DocumentEngine. MCP keeps engines alive per-workspace for future incremental.

**2. Two-tier FFI (REJECTED):** CEngineResult for LSP, CMd4cResult stays for MCP. Maintains two parallel FFI type sets — the root problem.

**3. Lift enrichment to Rust (REJECTED):** CMd4cResult universal, Rust computes positions/slugs. Moves work wrong direction (Zig→Rust for perf-sensitive ops).

## Key Decisions

| Question | Answer | Implication |
|----------|--------|-------------|
| Architecture approach | Engine-everywhere | Single FFI type, single code path |
| MCP engines | Persistent per file | HashMap per workspace, enables future incremental |
| LSP fallback | Stale index cache | Cache last-good DocumentIndex per URI |
| Test migration | from_text() replaces all 3 constructors | ~80 callers, mechanical rename |
| Nullable encoding | length=0 means None | Empty string and no value equivalent for markdown |

## Log

- [2026-03-23T16:44:36Z] [Seth] Phase 3 (marky-xfgb) closed. Next: marky-llj (Phase 4.1) — add from_text() convenience, migrate 18 from_ast callers to from_text. This unblocks from_ast deletion and then from_scan/from_blob deletion sweep.
- [2026-03-23T17:46:44Z] [Seth] Phase 4.1 (marky-llj) closed. from_text() landed, all 18 from_ast callers migrated. from_ast has zero callers — module is dead and eligible for deletion. Next: Phase 4.2 — migrate from_scan_with_frontmatter callers to from_text, then delete from_ast.rs, from_scan.rs, and from_blob/.
