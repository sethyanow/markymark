---
id: marky-8d8
title: 'Phase 3: Direct Arena Decode'
status: open
type: epic
priority: 2
depends_on: [marky-686, marky-u9q, marky-g9h, marky-03r, marky-bt9]
parent: marky-zsys
---









## Context
Parent epic marky-zsys, Phase 3. Depends on Phase 2 (marky-686, now closed).

**Architecture update (2026-03-24):** Epic originally targeted `from_blob()`/`DecodedOwnedData`
— all eliminated by marky-0xtn (blob serialization removed). The double-copy now lives in:
1. `convert_engine_result()` in engine_ffi.rs: reads CEngineResult.text_blob → owned Strings in EngineExtraction
2. `from_engine_result_inner()` in from_engine.rs: copies EngineExtraction Strings → bumpalo arena

Phase 3a eliminates the EngineExtraction intermediary by reading text_blob directly into the arena.
Phase 3b/3c parameterize DocumentIndex on text_blob lifetime for zero-copy borrowing.

## Requirements
- R5: Direct arena decode — `from_engine_result_direct` reads CEngineResult.text_blob directly into bumpalo arena, eliminating EngineExtraction owned Strings
- R6: `DocumentIndex` parameterized on engine lifetime — borrows text from Zig text_blob
- R7: `RealmIndex` and LSP `ServerState` adapted to hold lifetime-parameterized DocumentIndex

## Success Criteria
- [x] `from_engine_result_direct` decodes CEngineResult.text_blob into arena — no intermediate EngineExtraction
- [x] EngineExtraction intermediary not used in the LSP hot path (old path may remain as fallback)
- [x] Benchmark: direct decode measurably faster than EngineExtraction path (Phase 3a alone)
- [x] Text fields borrow from owner.text_blob via self_cell `'a` lifetime (blob-in-owner replaces R6/R7 lifetime parameter — user confirmed 2026-03-24)
- [x] DocumentIndexCell uses try_new for fallible blob reads inside closure
- [x] No lifetime parameter on DocumentIndex — no RealmIndex/ServerState cascade needed
- [x] No unsafe lifetime transmutes or 'static escape hatches
- [x] All existing tests pass (1290/1290 workspace tests green)
- [x] Benchmark: blob-in-owner 21-63% faster than via_extraction path (scales with doc size)

## Anti-Patterns
- NO unsafe lifetime transmutes or 'static extensions (sound lifetime modeling or nothing)
- NO keeping EngineExtraction in the hot path as "compatibility layer" after 3a
- NO doing only 3a and deferring 3b+3c (epic requires R6+R7)

## Key Considerations
- `CEngineResult.text_blob` is Zig-owned memory, valid until `marky_engine_free_result` is called
  (which happens on `EngineResult::drop`). Phase 3a copies into arena (safe — arena outlives the
  EngineResult borrow). Phase 3b/3c borrow from it (requires sound lifetime modeling).
- `DocumentIndexCell` uses `self_cell` pattern where the arena owns the text. With text_blob
  borrowing, the cell needs to accommodate an external lifetime. May require replacing self_cell
  with explicit lifetime management.
- `EngineResult` is obtained via `engine.get_result()` which borrows `&self` from DocumentEngine.
  For Phase 3b/3c, DocumentIndex must not outlive the EngineResult. Currently engines are behind
  `Mutex<DocumentEngine>` — the lock scope must ensure text_blob validity.
- Phase 3a (direct arena decode) is a standalone win that doesn't change the public API.
  3b+3c change the type signature and cascade. If 3b+3c prove infeasible, 3a is still valuable.

## Acceptance Requirements
**Agent Documentation:**
- [ ] CLAUDE.md: update Architecture section if DocumentIndex type signature changes
- [ ] docs/MEMORY.md: update with direct arena decode decision and lifetime model

**User Walkthrough Must Cover:**
- Direct decode produces identical DocumentIndex content (parity test)
- Benchmark comparison: Phase 3a vs Phase 2 baseline
- Lifetime soundness: engine update invalidates old text_blob, new index created from new result
- Edge case: concurrent read of index while engine updates (lock semantics)
