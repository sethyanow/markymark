---
id: marky-n7wx
title: '[EPIC] RealmIndex v2: string interning, stem index, incremental updates, lazy cold indexes'
status: open
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-io3h
---




## Design

## Requirements (IMMUTABLE)

- RealmIndex cross-doc indexes use lasso string interner (Rodeo) — no .to_string() for slugs, tags, block IDs in add_document/remove_document hot paths
- Wiki link stem resolution is O(1) via a dedicated stem-to-URI index — no linear scan of all documents
- Single-document edits patch cross-doc indexes incrementally (diff old vs new headings/tags/blocks) rather than remove-all + re-add-all
- Cold indexes (tag_to_docs, key_path_to_docs) are lazily computed on first query after invalidation, not eagerly maintained on every edit
- All existing LSP and MCP features produce identical results (correctness parity)
- DocumentIndex public API unchanged — interning is internal to RealmIndex

## Success Criteria (MUST ALL BE TRUE)

- [ ] Zero .to_string() calls in add_document/remove_document for heading slugs, tag names, block IDs as HashMap keys
- [ ] find_uri_by_stem() is O(1) HashMap lookup, not O(D) linear scan
- [ ] Single-char edit in 50KB doc within 1000-doc vault: RealmIndex update >=3x faster than current remove+add
- [ ] tag_to_docs and key_path_to_docs not rebuilt on edits that don't change tags/key-paths
- [ ] All existing tests pass (cargo nextest)
- [ ] Zero clippy warnings, fmt clean
- [ ] Memory usage does not regress (interner overhead < string duplication savings)

## Anti-Patterns (FORBIDDEN)

- ❌ NO changing DocumentIndex public API (scope: interning is RealmIndex-internal, DocumentIndex stays arena-based)
- ❌ NO global mutable interner (safety: Rodeo owned by RealmIndex, not leaked as global static)
- ❌ NO eager string resolution from Spur on hot paths (performance: resolve Spur to &str only when returning to callers, not during internal lookups)
- ❌ NO removing remove_document capability (correctness: documents must be fully removable from all indexes)
- ❌ NO breaking the engine pipeline fallback (safety: from_scan fallback in LSP must continue to work)
- ❌ NO using ThreadedRodeo (unnecessary: RealmIndex is single-threaded, Rodeo is simpler and lighter)

## Approach

Bottom-up in 4 layers, each independently testable:

**Layer 1 (String Interning):** Add lasso::Rodeo to RealmIndex. Replace HashMap<String, ...> keys with HashMap<Spur, ...> for slug_to_headings, block_to_location, tag_to_docs. In add_document, intern slugs/tags/blocks via rodeo.get_or_intern(). Remove path uses rodeo.get() for zero-allocation lookup. Public query methods resolve Spur to &str at boundaries.

**Layer 2 (Stem Index):** Add stem_to_uris: HashMap<Spur, Vec<DocumentUri>> maintained during add/remove. find_uri_by_stem becomes O(1) interner lookup + HashMap get. Stems interned via same Rodeo.

**Layer 3 (Incremental Updates):** Replace remove+add pattern with diff-based patch. On edit: compute new headings/tags/blocks, diff against stored old set (Spur equality is O(1)), patch only changed entries in cross-doc maps. Requires storing per-document contribution metadata.

**Layer 4 (Lazy Cold Indexes):** Wrap tag_to_docs and key_path_to_docs in lazy builders. Set dirty flag on document changes, rebuild only on first query. Hot indexes (slug_to_headings, block_to_location) stay eager.

## Architecture

### New Dependencies
- lasso crate (string interning, Rodeo for single-threaded use)

### Modified Files
- markymark-index/src/realm/mod.rs — RealmIndex struct, add/remove/query methods
- markymark-index/src/realm/types.rs — ResolvedHeading/ResolvedBlock unchanged (String fields preserved)
- markymark-index/tests/realm_index.rs — new tests per layer
- markymark-index/src/resolution.rs — Use stem index for wiki link resolution (Layer 2)
- markymark-index/Cargo.toml — Add lasso dependency

### Data Flow Change
Current: did_change -> remove_document(full String alloc + scan) -> add_document(full String alloc rebuild)
Layer 1: did_change -> remove_document(Spur lookup, zero alloc) -> add_document(Spur keys, fewer allocs)
Layer 3: did_change -> diff_and_patch(old_spurs, new_spurs) -> update only changed cross-doc entries

## Design Rationale

### Problem
RealmIndex does ~52 extra String allocations per document edit at 50-heading scale (1 per heading HashMap key + 1 per block key + 1 per tag key, plus N+B+T allocations in remove path). find_uri_by_stem is O(D) for every wiki link. Cold indexes rebuilt on every edit.

### Research Findings

**Codebase:**
- realm/mod.rs:72-137 — add_document clones slug into HashMap key AND ResolvedHeading (redundant)
- realm/mod.rs:171-244 — remove_from_cross_doc_indexes allocates N+B+T Strings just to look up HashMap keys
- resolution.rs:44-45 — find_uri_by_stem iterates ALL documents, no index
- lsp/src/state/mod.rs:243-322 — apply_document_changes calls remove+add on every 75ms debounce

**External:**
- lasso crate: Rodeo is non-concurrent interner, Spur is u32 (4 bytes vs 24+ bytes for String)

### Approaches Considered

#### 1. Bottom-up: Rodeo interner, layers build on top ✓
Chosen because each layer independently valuable, Spur equality enables cheap incremental diff in Layer 3.

#### 2. Top-down: redesign update model first ❌
REJECTED: Incremental diff without interning still needs String comparison. Foundation must come first.

#### 3. Clean-room RealmIndex rewrite ❌
REJECTED: Large blast radius, hard to validate parity.

### Scope Boundaries

**In scope:** String interning, stem index, incremental updates, lazy cold indexes, correctness/perf testing.

**Out of scope:** DocumentIndex API changes, zero-copy blob borrowing, engine incremental diffing, concurrent index updates, SemanticIndex optimization.

### Open Questions
- Should interner be shared with DocumentIndex? (default: no, RealmIndex-internal only)
- Spur size: u32 default vs u16 for vaults <65K unique strings? (default: u32, profile later)

## Design Discovery

### Key Decisions Made

| Question | User Answer | Implication |
|----------|-------------|-------------|
| Epic scope? | All 4 layers | Comprehensive optimization, iterative delivery |
| Interner library? | lasso (Rodeo) | New dependency, proven crate |
| Layer ordering? | 1->2->3->4 | Foundation first, each layer builds on previous |
| Approach? | Bottom-up (A) | Interner enables cheap equality for incremental |
| Rodeo vs ThreadedRodeo? | Rodeo | RealmIndex is single-threaded, simpler API |
| Intern URIs? | No | Low dedup, high overhead (D2) |
| Public API change? | Keep String fields | Resolve Spur at query boundaries (D3) |
