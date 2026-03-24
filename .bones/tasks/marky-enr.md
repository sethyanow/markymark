---
id: marky-enr
title: 'Task 3: LSP edit range threading — pass didChange byte bounds to engine'
status: closed
type: task
priority: 2
parent: marky-686
---







## Context

Phase 2 (marky-686) of Engine Pipeline v2 (marky-zsys). Third and final task — LSP integration.

Task 1 (marky-f1w) extended the FFI to accept edit range params. Task 2 (marky-v60) implemented
Zig-side slug reuse for headings before edit_offset. This task completes Phase 2 by threading
actual LSP edit range information from `apply_document_changes` through to the engine.

Currently, `build_markdown_index_via_engine` always calls `engine.update(&masked, None)`. The
`IncrementalByteBounds` computed from each `DocumentChange::Incremental` is used only for text
replacement, then discarded. This task passes that information through to the engine.

**Blocked by:** marky-v60 (Zig slug reuse — closed)
**Unlocks:** Closing sub-epic marky-686 (Phase 2 complete), unblocking Phase 3 (marky-8d8)

## Requirements

From parent sub-epic marky-686:
- R3 (final): LSP `apply_document_changes` computes cumulative edit byte bounds and passes to engine
- R4 (integration): Edit ranges flow end-to-end from LSP didChange to Zig slug reuse

## Success Criteria

- [ ] `build_markdown_index_via_engine` signature accepts `Option<EditRange>`, passes to `engine.update()`
- [ ] `apply_document_changes` accumulates bounding box from incremental changes: min(start_byte), total old_len, total new_len
- [ ] `DocumentChange::Full` in a change sequence invalidates accumulated range (passes `None`)
- [ ] `change_document` continues to pass `None` (full-text replacement, no incremental info)
- [ ] Test: apply incremental changes → engine receives non-None edit range → slug_reuse_count > 0
- [ ] Test: apply Full change → engine receives None → slug_reuse_count == 0
- [ ] Test: mixed Full + Incremental sequence → last Full invalidates range
- [ ] All existing tests pass (1290 workspace at session start)

## Anti-Patterns

- NO computing edit ranges on the Rust side independent of IncrementalByteBounds (reuse existing computation)
- NO passing per-change ranges to the engine (engine accepts one range per update; accumulate a bounding box)
- NO using u32::MAX or sentinel values for "no range" — use Option<EditRange> with None
- NO changing IncrementalByteBounds struct (it's correct for text replacement; the bounding box is a new accumulation)
- NO accumulating range from skipped edits (end_before_start=true → edit not applied, must not contribute to bounding box)

## Implementation

### Step 1: RED — Write test for incremental changes threading edit range
**File:** `markymark-lsp/src/state/mod.rs` (tests module)
- `test_apply_incremental_changes_threads_edit_range`:
  Create ServerState, open a multi-heading doc, apply incremental change at end,
  verify slug_reuse_count > 0 on the engine (proves edit range was passed through).
  Access engine via `state.engines.get(uri).lock().engine.slug_reuse_count()`.
- **Expected:** compile error or assertion failure — edit range not threaded yet

### Step 2: RED — Write test for Full change invalidating range
**File:** `markymark-lsp/src/state/mod.rs` (tests module)
- `test_apply_full_change_passes_none`:
  Open doc, apply Full change, verify slug_reuse_count == 0 (no reuse with None range)
- **Expected:** passes vacuously (build_markdown_index_via_engine already passes None)

### Step 3: GREEN — Add edit_range parameter to build_markdown_index_via_engine
**File:** `markymark-lsp/src/state/mod.rs`
- Change signature: `fn build_markdown_index_via_engine(&mut self, uri: &DocumentUri, text: &str, edit_range: Option<EditRange>)`
- Import `EditRange` from `markymark_kernels::engine`
- Pass `edit_range` to `engine_state.engine.update(&masked, edit_range)` instead of `None`
- Update all 3 callers:
  - `apply_document_changes` (line 446): will pass accumulated range (Step 4)
  - `change_document` (line 354): pass `None`
  - `open_document` (line 333): pass `None` (no incremental info on open)

### Step 4: GREEN — Accumulate bounding box in apply_document_changes
**File:** `markymark-lsp/src/state/mod.rs`
- Before the change loop: `let mut accumulated: Option<(usize, usize, usize)> = None;`
  Tuple: (min_start_byte, total_old_len, total_new_len)
- In the `Incremental` arm, ONLY when edit is applied (NOT when skipped by end_before_start):
  After bounds computation and after text replacement, update via match:
  ```rust
  accumulated = Some(match accumulated {
      Some((min_start, total_old, total_new)) => (
          min_start.min(bounds.start_byte),
          total_old + (bounds.old_end_byte - bounds.start_byte),
          total_new + new_text.len(),
      ),
      None => (
          bounds.start_byte,
          bounds.old_end_byte - bounds.start_byte,
          new_text.len(),
      ),
  });
  ```
  Note: clamped edits (start_clamped/end_clamped) ARE applied and MUST contribute to the bounding box.
- In the `Full` arm: set `accumulated = None` (full replacement invalidates)
- After the loop: convert via `.map()`:
  ```rust
  let edit_range = accumulated.map(|(start, old_len, new_len)| EditRange {
      offset: start as u32,
      old_len: old_len as u32,
      new_len: new_len as u32,
  });
  ```
- Pass to `build_markdown_index_via_engine(uri, &final_text, edit_range)`

### Step 5: Verify — Run all tests
- `cargo nextest -p markymark-lsp` — LSP tests pass
- `cargo nextest` — full workspace passes

### Step 6: Final verification and commit

## Key Considerations

- **Frontmatter masking shifts nothing:** `mask_frontmatter` replaces frontmatter bytes with spaces,
  preserving all byte offsets. So edit ranges computed on raw text are valid against masked text.
- **Bounding box semantics:** Multiple incremental edits in one didChange call each shift subsequent
  byte offsets. The bounding box (min start, cumulative old_len, cumulative new_len) is an
  approximation — it may over-estimate the changed region, causing fewer slug reuses than optimal.
  This is conservative (correct, not maximal optimization). The Zig side handles over-large ranges
  gracefully (fewer headings qualify for reuse).
- **u32 truncation:** `IncrementalByteBounds` uses `usize`. `EditRange` uses `u32`. For documents
  > 4GB, the cast would truncate. Not a real concern — LSP doesn't handle 4GB markdown files.
  Use `as u32` directly.
- **change_document vs apply_document_changes:** `change_document` receives full text (no incremental
  info), correctly passes `None`. Only `apply_document_changes` has incremental change data.
- **Skipped edits:** When `end_before_start` is true, the edit is skipped (`continue` at line 422).
  These must NOT contribute to the bounding box — the text was not modified by this edit.
- **Clamped edits:** When `start_clamped` or `end_clamped` is true, the edit IS applied with adjusted
  byte positions. These MUST contribute to the bounding box — the text was modified, just at clamped
  positions. The `start_byte`/`old_end_byte` in the bounds struct already reflect clamping.

## Log

- [2026-03-24T15:35:20Z] [Seth] Task complete. Threaded LSP edit ranges to Zig engine. apply_document_changes accumulates bounding box via match on Option tuple, converts via .map() to EditRange, passes through build_markdown_index_via_engine to engine.update(). Full changes invalidate range (None). Skipped edits excluded. 3 async tests added. 1289/1289 workspace tests pass. Clippy clean.
