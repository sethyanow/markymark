---
id: marky-5ig
title: 'Phase 2 Acceptance: Edit Range Threading'
status: open
type: task
priority: 2
parent: marky-686
---



## Context

Phase 2 acceptance gate for marky-686 (Edit Range Threading). All 3 implementation tasks
complete (marky-f1w, marky-v60, marky-enr), all 10/10 success criteria met.

## Deliverables

### 1. Agent Documentation
- Update docs/MEMORY.md with Phase 2 completion status (done in this session)
- CLAUDE.md: no updates needed (internal optimization, no user-facing API change)

### 2. User Walkthrough

Verify Phase 2 end-to-end: edit ranges flow from LSP didChange through FFI to Zig slug reuse.

```bash
# 1. FFI round-trip: engine accepts edit range, slug reuse works
cargo test -p markymark-kernels -- slug_reuse

# 2. LSP threading: apply_document_changes threads ranges to engine
cargo test -p markymark-lsp -- test_apply_

# 3. Phase gate: all edit range tests pass
cargo test -p markymark-kernels -- edit_range
cargo test -p markymark-lsp -- edit_range

# 4. Full workspace green
cargo nextest run
```

Observable outcomes:
- `test_engine_slug_reuse_edit_at_end`: headings before edit reuse slugs (count > 0)
- `test_engine_slug_reuse_zero_range_no_reuse`: zero-range falls back to full computation
- `test_apply_incremental_changes_threads_edit_range`: incremental LSP edits thread ranges
- `test_apply_full_change_passes_none`: Full changes pass None (no range info)
- `test_apply_mixed_full_then_incremental_invalidates_range`: Full resets accumulation

## Success Criteria
- [ ] User reviews walkthrough and confirms Phase 2 works as expected
- [ ] User closes this acceptance task
