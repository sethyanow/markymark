---
id: marky-zsys
title: '[EPIC] Engine Pipeline v2: incremental diffing, edit ranges, zero-copy blob'
status: open
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-n7wx, marky-lpb, marky-686, marky-8d8]
---







## Requirements (IMMUTABLE)

- R1: Zig DocumentEngine exposes content hash via C FFI (`marky_engine_get_content_hash`)
- R2: LSP update path short-circuits blob serialization + deserialization when content hash is unchanged after `engine.update()`
- R3: `DocumentEngine::update()` FFI accepts optional edit range parameters (byte offset + length of changed region)
- R4: Zig engine reuses slugs for headings outside the edit range, skipping slug recomputation for unchanged headings
- R5: `from_blob()` decodes blob text pool directly into bumpalo arena, eliminating the intermediate owned Vec allocation stage
- R6: `DocumentIndex` parameterized on blob lifetime (`DocumentIndex<'blob>`) — borrows text from engine-owned blob instead of copying into arena
- R7: `RealmIndex` and LSP `ServerState` adapted to hold lifetime-parameterized `DocumentIndex<'blob>`

## Success Criteria

- [x] `marky_engine_get_content_hash` returns u64 via FFI; Rust wrapper exposes it on `DocumentEngine`
- [ ] `build_markdown_index_via_engine` returns `None` (no index) when content hash unchanged; callers skip `realm.update_document()`
- [ ] Benchmark: unchanged-content update skips blob serialization + deserialization (measurable via criterion)
- [ ] `marky_engine_update` accepts edit range info; Zig side receives byte offset + old_len + new_len
- [ ] Headings outside edit range reuse previous slugs (verified by test: edit at end of doc, heading slugs not recomputed)
- [ ] LSP `apply_document_changes` threads incremental edit byte bounds to engine update
- [ ] `from_blob_inner` allocates directly into bumpalo arena — no intermediate `DecodedOwnedData` Vecs
- [ ] `DocumentIndex<'blob>` compiles with blob lifetime; text fields borrow from blob data
- [ ] `RealmIndex` and `ServerState` hold `DocumentIndex<'blob>` without lifetime conflicts
- [ ] All existing tests pass after each phase

## Anti-Patterns (FORBIDDEN)

- NO pre-parse text hashing on Rust side to skip engine.update() — the Zig content hash is post-extraction and covers structural changes, not just text identity. Pre-parse hashing would miss frontmatter masking edge cases. (Use the engine's own hash, not a competing one.)
- NO breaking the engine's "old state preserved on failure" contract — update() must remain atomic: parse succeeds → swap, parse fails → old state untouched. Hash comparison happens AFTER successful parse, not as a gate to skip parsing.
- NO storing previous DocumentIndex in ServerState for unchanged-content reuse — the index is owned by RealmIndex after handoff. The short-circuit returns None to signal "skip update," not a cached copy.
- NO incremental md4c parsing — md4c is streaming single-pass; edit ranges are post-parse optimizations, not parse optimizations.
- NO lifetime elision tricks (`'static` transmutes, unsafe lifetime extensions) to avoid the Phase 3 propagation work — the whole point is sound lifetime modeling.

## Approach

Three-phase optimization of the engine pipeline, attacking the update hot path from highest-ROI to lowest:

Phase 1 exposes the content hash that Zig already computes and uses it to short-circuit the expensive blob serialization + deserialization when the extracted structure hasn't changed. The parse still runs (md4c is fast), but the ~2ms of blob + arena work is skipped on no-op structural edits.

Phase 2 threads LSP edit range information through the FFI boundary so the Zig engine knows which byte region changed. This enables slug reuse for headings outside the edit range — slug computation involves allocation and normalization work that can be skipped when the heading text didn't change.

Phase 3 eliminates the double-copy in blob deserialization (blob → owned Vecs → arena) first by decoding directly into the arena (3a), then by making DocumentIndex borrow from blob data instead of copying at all (3b-3c). The lifetime parameterization is a significant refactor that cascades through RealmIndex and LSP state.

## Architecture

```
LSP didChange
  → apply_document_changes(uri, changes: Vec<DocumentChange>)
    → apply text edits to stored document text
    → collect edit byte bounds [Phase 2c]
    → build_markdown_index_via_engine(uri, text, edit_ranges?)
      → mask frontmatter
      → engine.update(masked, edit_ranges?)     [Phase 2a: FFI plumbing]
        → Zig: parseAll (full md4c reparse)
        → Zig: reuse slugs for unchanged headings [Phase 2b]
        → new content_hash computed
      → engine.get_content_hash()               [Phase 1a: FFI]
      → compare with stored hash                [Phase 1b: short-circuit]
        → if unchanged: return None (skip downstream)
        → if changed:
          → engine.get_blob()                   (lazy serialization)
          → DocumentIndex::from_blob()          [Phase 3a: direct arena decode]
                                                [Phase 3b: borrow from blob]
          → return Some(index)
    → if Some(index): realm.update_document(uri, index)
      → (RealmIndex already fast-paths identical contributions)
```

Key files:
- `zig/src/engine/document.zig` — DocumentEngine struct, update(), parseAll(), content_hash field
- `zig/src/kernels/content_hash.zig` — content_hash() computation
- `markymark-kernels/src/engine.rs` — Rust FFI wrapper (DocumentEngine, ScanBlob, extern "C" fns)
- `markymark-index/src/document/from_blob/mod.rs` — from_blob_inner, decode_owned_data → arena copy
- `markymark-index/src/document/from_blob/owned.rs` — DecodedOwnedData intermediate structs
- `markymark-lsp/src/state/mod.rs` — ServerState, build_markdown_index_via_engine, apply_document_changes
- `markymark-index/src/realm/mod.rs` — RealmIndex::update_document

## Phases

### Phase 1: Content Hash Short-Circuit
**Scope:** R1, R2
**Gate:**
- `cargo test -p markymark-kernels -- content_hash` → passes (FFI exposes hash)
- `cargo test -p markymark-lsp -- hash_unchanged_skip` → passes (short-circuit verified)
- `cargo bench -p markymark-index -- realm_update` → unchanged-content case measurably faster than baseline

**Seams (2 tasks):**
- **1a — FFI hash exposure:** Add `marky_engine_get_content_hash` to Zig C exports + Rust extern + `DocumentEngine::content_hash()` method. Pure plumbing, no behavior change.
- **1b — Rust short-circuit:** Store hash alongside engine in `ServerState.engines`. After `engine.update()`, compare old vs new hash. `build_markdown_index_via_engine` returns `Option<DocumentIndex>`. Callers skip `realm.update_document()` when `None`.

### Phase 2: Edit Range Threading
**Scope:** R3, R4
**Gate:**
- `cargo test -p markymark-kernels -- edit_range` → passes (FFI accepts ranges)
- `cargo test -p markymark-index -- slug_reuse` → passes (headings outside edit range reuse slugs)
- `cargo test -p markymark-lsp -- edit_range_threading` → passes (LSP threads ranges to engine)

**Seams (3 tasks):**
- **2a — API plumbing:** Extend `marky_engine_update` C signature to accept `edit_offset`, `edit_old_len`, `edit_new_len` (all u32, 0/0/0 = no range info). Rust `DocumentEngine::update` gains optional `EditRange` parameter. No behavior change yet — the Zig side ignores the values.
- **2b — Zig slug reuse:** In `parseAll` or `update`, when edit range is provided, compare new headings with stored headings. For headings whose text bytes are entirely outside the edit range, reuse the previous slug instead of recomputing. Requires storing previous heading offsets.
- **2c — LSP integration:** In `apply_document_changes`, collect the cumulative edit byte bounds from `IncrementalByteBounds` and pass them through `build_markdown_index_via_engine` to `engine.update(text, edit_range)`.

### Phase 3: Zero-Copy Blob
**Scope:** R5, R6, R7
**Gate:**
- `cargo test -p markymark-index -- from_blob` → passes (direct arena decode works)
- `cargo test --workspace` → passes (lifetime propagation compiles and works)
- `cargo bench -p markymark-index -- realm_update` → from_blob measurably faster than Phase 2 baseline

**Seams (3 tasks):**
- **3a — Direct arena decode:** Rewrite `from_blob_inner` to decode blob text pool slices directly into bumpalo arena via `arena_alloc_str(arena, &blob[offset..offset+len])`, eliminating the intermediate `DecodedOwnedData` structs and their owned Vecs. Public API unchanged.
- **3b — Lifetime parameterization:** Introduce `DocumentIndex<'blob>` where text fields (`&'blob str`) borrow from blob data. `DocumentIndexCell` / `self_cell` may need rethinking — the arena is no longer the sole text owner.
- **3c — State propagation:** Update `RealmIndex` and `ServerState` to hold `DocumentIndex<'blob>` where `'blob` is tied to the engine's blob lifetime. This is the cascade: every consumer of DocumentIndex must handle the lifetime parameter.

## Agent Failure Mode Catalog

### Phase 1
| Shortcut | Rationalization | Pre-block |
|----------|----------------|-----------|
| Hash Rust-side text instead of using Zig content hash | "Faster to hash before parsing" | Anti-pattern: NO pre-parse text hashing. Zig hash is post-extraction and covers frontmatter masking. |
| Return stale DocumentIndex from cache on hash match | "Same hash means same index" | Anti-pattern: NO storing previous DocumentIndex. Return None, let caller skip update. |
| Skip tests, rely on "it compiles" | "FFI is simple plumbing" | Gate requires specific test names to pass. |

### Phase 2
| Shortcut | Rationalization | Pre-block |
|----------|----------------|-----------|
| Pass full-text byte range (0..len) as edit range | "Technically correct, just no optimization" | Test must verify headings OUTSIDE edit range reuse slugs — pass-through defeats this. |
| Skip 2b, only do 2a+2c plumbing | "Plumbing is the hard part, optimization can come later" | Phase gate requires slug_reuse test to pass. Plumbing without behavior is not done. |
| Compute slug reuse on Rust side instead of Zig | "Zig FFI boundary is complex" | Slug computation lives in Zig (document_helpers.zig). Reuse logic belongs where slugs are computed. |

### Phase 3
| Shortcut | Rationalization | Pre-block |
|----------|----------------|-----------|
| Use unsafe lifetime transmutes to avoid propagation | "'static is fine, the blob outlives usage" | Anti-pattern: NO lifetime elision tricks. Sound modeling or nothing. |
| Do 3a only, defer 3b+3c | "Direct arena decode is good enough" | 3a is a standalone win but epic requires R6+R7. Phase isn't done without them. |
| Keep DecodedOwnedData as "compatibility layer" | "Easier to have both paths" | 3a's entire point is eliminating the intermediate. Keeping it defeats the optimization. |

## Seam Contracts

### Phase 1 → Phase 2
**Delivers:** `DocumentEngine::content_hash()` method on Rust side. `build_markdown_index_via_engine` returns `Option<DocumentIndex>`. Hash stored in `ServerState.engines`.
**Assumes:** Phase 2 extends the same `DocumentEngine::update()` API and `build_markdown_index_via_engine` function.
**If wrong:** Phase 2 plumbing must be compatible with the Option return type and hash storage. If the API shape changes, both phases touch the same functions.

### Phase 2 → Phase 3
**Delivers:** Edit range info flows through FFI. Slug reuse logic in Zig. `build_markdown_index_via_engine` passes edit ranges.
**Assumes:** Phase 3 changes `from_blob` internals and DocumentIndex type signature. The edit range plumbing (Phase 2) and short-circuit logic (Phase 1) are orthogonal to from_blob changes.
**If wrong:** If Phase 3's lifetime parameterization changes how `build_markdown_index_via_engine` handles DocumentIndex, the Option<DocumentIndex> return from Phase 1 and the edit range threading from Phase 2 may need signature updates. But the logic is independent.

## Design Rationale

### Problem
The engine pipeline (md4c parse → blob serialize → blob deserialize → arena copy) costs ~4.7ms at 50KB and is now the dominant hot-path cost after RealmIndex v2 reduced cross-doc index overhead. On every keystroke, the full pipeline runs even when the edit doesn't change document structure.

### Research Findings
**Codebase:**
- `zig/src/engine/document.zig:62` — `content_hash: u64` already computed on every parse via `content_hash_mod.content_hash(text.ptr, text.len)` at line 583. NOT exposed via FFI.
- `markymark-kernels/src/engine.rs:16-24` — C FFI has only 4 functions: create, update, get_blob, destroy. No hash getter.
- `markymark-lsp/src/state/mod.rs:291-381` — `apply_document_changes` receives `Vec<DocumentChange>` with incremental edit ranges, applies them to text, then DISCARDS the range info before calling engine.
- `markymark-lsp/src/state/mod.rs:170-174` — `build_markdown_index_via_engine`: calls `engine.update()` → `engine.get_blob()` → `from_blob_with_frontmatter()` on every change. No hash check.
- `markymark-index/src/document/from_blob/mod.rs:88-113` — `from_blob_inner` calls `decode_owned_data` (blob → owned Vecs) then copies owned Vecs into bumpalo arena. Two copies.
- `markymark-index/src/realm/mod.rs:266-294` — `update_document` already has fast path: `old_contrib == new_contrib` skips cross-doc index ops. But the expensive work happens before this check.

**Benchmarks (marky-8d08):**
- md4c extract-only at 50KB: 4.686ms (Rust criterion, macOS ARM64)
- Full from_scan pipeline: 9.436ms vs tree-sitter's 26.662ms (2.8x speedup)
- Extraction allocation overhead dominates at scale, not raw parse speed

### Approaches Considered

#### 1. Three-phase pipeline optimization (selected)
**Chosen because:** Attacks the hot path in ROI order. Phase 1 is low-effort high-impact (skip blob work when unchanged). Phase 2 leverages existing LSP edit info. Phase 3 is the deep refactor for maximum savings. Each phase delivers standalone value.

#### 2. Pre-parse text hash to skip engine.update() entirely
**Why explored:** If text hasn't changed, skip everything including md4c parse.
**REJECTED BECAUSE:** LSP shouldn't send didChange if text didn't change. The content hash covers post-frontmatter-masking text — a Rust-side pre-hash on raw text would miss masking edge cases. The parse is fast (~2.5ms); the expensive work is downstream.
**DO NOT REVISIT UNLESS:** Profiling shows md4c parse itself is the bottleneck (currently it's not — allocation and blob work dominate).

#### 3. Replace bumpalo arena with direct blob borrowing (skip arena entirely)
**Why explored:** Eliminate all copies — DocumentIndex just references blob bytes.
**REJECTED BECAUSE:** This is Phase 3b-3c of the selected approach. Rejecting it as a STANDALONE approach because it requires the lifetime cascade refactor without the incremental wins from Phase 1-2 first.
**DO NOT REVISIT UNLESS:** Someone proposes a way to do zero-copy without lifetime propagation (e.g., arena-less DocumentIndex).

### Scope Boundaries
**In scope:** LSP update hot path optimization. FFI changes. from_blob refactor. Lifetime propagation.
**Out of scope:** md4c incremental parsing (not possible — streaming single-pass). MCP engine path (separate update flow). Embedding/semantic index optimization.

### Open Questions
- Phase 3b: How does `self_cell` interact with blob lifetime? `DocumentIndexCell` currently owns the arena. If text borrows from blob, the cell pattern may need replacement or augmentation.
- Phase 2b: Should slug reuse apply to ALL element types (links, tags, etc.) or just headings? Headings have the most expensive slug computation. Links/tags might not benefit enough to justify the complexity.

## Design Discovery

### Key Decisions Made
| Question | Answer | Implication |
|----------|--------|-------------|
| Phasing order? | Incremental diffing → Edit ranges → Zero-copy | Phase 1 is lowest effort, highest standalone ROI. Phase 3 is highest effort, deferred until proven needed. |
| Edit range source? | LSP didChange `ContentChangeEvent` | Already received in `apply_document_changes`. Need to thread through, not create new source. |
| Zero-copy scope? | Full lifetime refactor (DocumentIndex<'blob>) | Cascades to RealmIndex + ServerState. Not bounded to from_blob internals. |
| One epic or multiple? | One epic, three phases | Agents scope to individual seam tasks within phases, not whole phases at once. |

### Dead-End Paths
- Pre-parse Rust-side text hashing: explored, rejected — misses frontmatter masking, LSP shouldn't send unchanged text.

### Open Concerns
- Phase 3 lifetime cascade is the riskiest part. self_cell currently manages the arena lifetime. Adding a blob lifetime may require rethinking DocumentIndex's ownership model.
- Phase 2b slug reuse requires storing previous heading offsets across updates. Need to verify this doesn't bloat the Zig DocumentEngine struct significantly.
