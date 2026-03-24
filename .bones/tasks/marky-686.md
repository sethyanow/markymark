---
id: marky-686
title: 'Phase 2: Edit Range Threading'
status: open
type: epic
priority: 2
depends_on: [marky-lpb, marky-f1w, marky-v60]
parent: marky-zsys
---





## Context
Parent epic marky-zsys, Phase 2. Depends on Phase 1 (marky-lpb).
LSP already receives incremental edit ranges in `apply_document_changes` but discards
them after applying text edits. This phase threads that information through to the Zig
engine so it can reuse slugs for headings outside the edited region.

## Requirements
- R3: `DocumentEngine::update()` FFI accepts optional edit range parameters (byte offset + old_len + new_len)
- R4: Zig engine reuses slugs for headings outside the edit range, skipping slug recomputation

## Success Criteria
- [x] `marky_engine_update` C signature extended with `edit_offset`, `edit_old_len`, `edit_new_len` (u32)
- [x] Rust `DocumentEngine::update()` accepts optional `EditRange` parameter
- [x] Zero-values (0/0/0) mean "no range info" — Zig side falls back to full slug computation
- [ ] Zig `update()` stores previous heading offsets + slugs for comparison
- [ ] Headings whose byte range is entirely before or after the edit range reuse previous slugs
- [ ] Test: edit at end of document, heading slugs at start not recomputed (verified via hash or direct comparison)
- [ ] Test: edit inside a heading causes that heading's slug to be recomputed
- [ ] LSP `apply_document_changes` computes cumulative edit byte bounds and passes to engine
- [ ] Test: LSP threads incremental change ranges to engine update
- [ ] All existing tests pass

## Anti-Patterns
- NO incremental md4c parsing (md4c is streaming single-pass; edit ranges are post-parse optimizations)
- NO computing slug reuse on Rust side (slug computation is in Zig, reuse logic belongs there)
- NO passing full-text range (0..len) as edit range (defeats the optimization, tests will catch this)

## Key Considerations
- Multiple incremental edits in one `apply_document_changes` call need to be collapsed into a
  single bounding range, or the engine needs to accept multiple ranges. Bounding box is simpler.
- Heading "outside edit range" needs careful byte math: after the edit, heading byte offsets in
  the NEW text are shifted. Comparison must use pre-edit offsets for old headings, post-edit for new.
- Slug reuse should apply at minimum to headings. Whether to extend to other element types
  (links, tags) is a scope question for task creation — start with headings only.

## Acceptance Requirements
**Agent Documentation:**
- [ ] CLAUDE.md: no updates needed (internal optimization)
- [ ] docs/MEMORY.md: update with edit range threading decision

**User Walkthrough Must Cover:**
- FFI round-trip: update with edit range, verify slug reuse for unchanged headings
- Edge case: edit that spans a heading boundary forces slug recomputation
- LSP integration: incremental didChange events produce correct edit ranges at engine
