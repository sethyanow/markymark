---
id: marky-8d8
title: 'Phase 3: Zero-Copy Blob'
status: open
type: epic
priority: 2
depends_on: [marky-686]
parent: marky-zsys
---

## Context
Parent epic marky-zsys, Phase 3. Depends on Phase 2 (marky-686).
Currently `from_blob()` does two copies: blob text pool → `DecodedOwnedData` (owned Vecs)
→ bumpalo arena (`arena_alloc_str`). This phase first eliminates the intermediate owned Vecs
(3a), then makes DocumentIndex borrow from blob data directly (3b), then propagates the
lifetime through RealmIndex and LSP state (3c).

## Requirements
- R5: `from_blob()` decodes blob text pool directly into bumpalo arena, eliminating intermediate owned Vec allocation
- R6: `DocumentIndex` parameterized on blob lifetime — borrows text from engine-owned blob
- R7: `RealmIndex` and LSP `ServerState` adapted to hold lifetime-parameterized DocumentIndex

## Success Criteria
- [ ] `from_blob_inner` no longer creates `DecodedOwnedData` — decodes directly into arena
- [ ] `owned.rs` intermediate structs removed or reduced to non-text fields only
- [ ] Benchmark: from_blob measurably faster after direct arena decode (Phase 3a alone)
- [ ] `DocumentIndex<'blob>` compiles with blob lifetime parameter
- [ ] Text fields in DocumentIndex entries borrow `&'blob str` from blob data
- [ ] self_cell / DocumentIndexCell reworked to accommodate blob lifetime
- [ ] `RealmIndex` holds DocumentIndex with correct lifetime
- [ ] `ServerState` engine + blob + index lifetime relationships are sound
- [ ] No unsafe lifetime transmutes or 'static escape hatches
- [ ] All existing tests pass after each sub-phase (3a, 3b, 3c independently)

## Anti-Patterns
- NO unsafe lifetime transmutes or 'static extensions (sound lifetime modeling or nothing)
- NO keeping DecodedOwnedData as "compatibility layer" (the whole point is eliminating it)
- NO doing only 3a and deferring 3b+3c (epic requires R6+R7)

## Key Considerations
- `DocumentIndexCell` uses `self_cell` pattern where the arena owns the text. With blob borrowing,
  the cell needs to accommodate an external lifetime. This may require replacing self_cell with
  explicit lifetime management or a different self-referential pattern.
- `ScanBlob<'_>` borrows `&self` from DocumentEngine. For DocumentIndex to borrow from blob,
  the engine must not be mutated while the index is alive. Currently engines are behind
  `Mutex<DocumentEngine>` — the lock scope must ensure blob validity.
- Phase 3a (direct arena decode) is a standalone win that doesn't change the public API.
  3b+3c change the type signature and cascade. If 3b+3c prove infeasible, 3a is still valuable.

## Acceptance Requirements
**Agent Documentation:**
- [ ] CLAUDE.md: update Architecture section if DocumentIndex type signature changes
- [ ] docs/MEMORY.md: update with zero-copy blob decision and lifetime model

**User Walkthrough Must Cover:**
- from_blob produces identical DocumentIndex content (parity test)
- Benchmark comparison: Phase 3a vs Phase 2 baseline
- Lifetime soundness: engine update invalidates old blob, new index created from new blob
- Edge case: concurrent read of index while engine updates (lock semantics)
