---
id: marky-v60
title: 'Task 2: Zig slug reuse — skip makeSlug for headings before edit range'
status: open
type: task
priority: 2
parent: marky-686
---

## Context

Phase 2 (marky-686) of Engine Pipeline v2 (marky-zsys). Second task — behavioral core.

Task 1 (marky-f1w) extended the FFI to accept edit range params. This task implements the Zig-side
logic: skip `makeSlug` for headings whose `source_offset` is before the edit range, reusing the
slug from the previous parse instead.

**Scope: before-edit-offset only.** Headings AFTER the edit range are NOT eligible for reuse because
`makeSlug` depends on dedup context (preceding headings with same base slug). If a heading was
added/removed in the edit range, headings after it may need different `-N` suffixes. Headings before
the edit range are safe: nothing before them changed, so dedup context is identical.

**Blocked by:** marky-f1w (FFI edit range params — closed)
**Unlocks:** Task 3 (LSP threading — passes actual edit ranges from didChange to engine)

## Requirements

From parent sub-epic marky-686:
- R4: Zig engine reuses slugs for headings outside the edit range, skipping slug recomputation

## Success Criteria

- [ ] `DocumentEngine` has `slug_reuse_count: u32` field, reset to 0 on each update
- [ ] `update()` saves old headings' `(source_offset, slug)` before calling parseAll
- [ ] Heading processing skips `makeSlug` for headings with `source_offset < edit_offset`, duping old slug instead
- [ ] `slug_reuse_count` incremented for each reused slug
- [ ] Zero-value edit range (0/0/0) bypasses reuse logic entirely (full recompute, count stays 0)
- [ ] `marky_engine_get_slug_reuse_count` C export + Rust `slug_reuse_count()` wrapper
- [ ] Zig test: edit at end of document → headings at start reuse slugs (slug_reuse_count > 0)
- [ ] Zig test: edit inside heading → that heading's slug recomputed (count reflects partial reuse)
- [ ] Rust FFI test: edit range after headings → slug_reuse_count > 0
- [ ] Rust FFI test: zero-value range → slug_reuse_count == 0
- [ ] All existing tests pass

## Anti-Patterns

- NO reusing slugs for headings at or after `edit_offset + new_len` (dedup suffix may differ)
- NO modifying `parseAll` signature (it's used by `create()` too — keep update-specific logic in `update()`)
- NO pointer reuse across freeState boundary (old slug memory freed — must dupe bytes)
- NO skipping reuse for zero-value range silently (must be an explicit check, tested)

## Implementation

### Step 1: RED — Write Zig test for slug reuse (edit at end)
**File:** `zig/src/engine/document_test.zig`
- `test "slug reuse: edit at end preserves earlier slugs"`:
  Create engine with multi-heading doc, update with edit range past all headings, assert `engine.slug_reuse_count > 0`
- **Expected:** compile error — `slug_reuse_count` field doesn't exist

### Step 2: RED — Write Zig test for slug recomputation (edit inside heading)
**File:** `zig/src/engine/document_test.zig`
- `test "slug reuse: edit inside heading forces recomputation"`:
  Create engine, update with edit range covering a heading, assert partial reuse count and correct new slug
- **Expected:** compile error — same field missing

### Step 3: GREEN — Add slug_reuse_count field to DocumentEngine
**File:** `zig/src/engine/document.zig`
- Add `slug_reuse_count: u32 = 0` field to `DocumentEngine` struct
- Reset to 0 at start of `update()`

### Step 4: GREEN — Implement slug reuse in update()
**File:** `zig/src/engine/document.zig`
- In `update()`, remove the three `_ = edit_*;` discards
- Before calling `parseAll`:
  a. Allocate temp array of `struct { offset: u32, slug: []const u8 }` from old `self.headings`
  b. Copy each old heading's `source_offset` and dupe its `slug` bytes (old memory freed later)
- After `parseAll` succeeds, before installing new headings: iterate new headings
  - If `edit_offset == 0 AND edit_old_len == 0 AND edit_new_len == 0`: skip reuse entirely
  - For each new heading with `source_offset < edit_offset`: find old heading at same offset, if found: free new slug, assign duped old slug, increment `slug_reuse_count`
  - Otherwise: keep parseAll-computed slug
- Free temp old-slug array after processing

### Step 5: GREEN — Expose slug_reuse_count via FFI
**Files:**
- `zig/src/engine/exports.zig`: add `marky_engine_get_slug_reuse_count(handle) -> u32`
- `markymark-kernels/src/engine.rs`: add extern declaration + `pub fn slug_reuse_count(&self) -> u32`

### Step 6: Verify — Run all tests
- Zig tests pass (document_test)
- `cargo nextest -p markymark-kernels` — Rust kernel tests pass
- `cargo nextest` — full workspace passes

### Step 7: Write Rust-side FFI tests
**File:** `markymark-kernels/src/engine.rs` (tests module)
- `test_engine_slug_reuse_edit_at_end`: multi-heading doc, update with edit range after all headings, assert `slug_reuse_count() > 0`
- `test_engine_slug_reuse_zero_range_no_reuse`: update with `None`, assert `slug_reuse_count() == 0`

### Step 8: Final verification and commit

## Key Considerations

- **Temp allocation for old slugs:** Old headings are in `self.headings` which gets freed by `freeState()`.
  Must dupe slug bytes into temp storage before parseAll runs. Use `self.allocator` for the temp array;
  free it after the reuse pass. Heading count is typically < 100, so this is small.
- **Matching old→new headings:** For headings before `edit_offset`, the `source_offset` is identical
  in old and new text (no byte shift). Linear scan over old headings is fine (O(n*m) where n,m < 100).
- **freeState ordering:** `update()` currently calls `freeState()` AFTER parseAll succeeds (line 122).
  The old heading data lives until freeState. So the temp copy must happen before parseAll (because
  parseAll might fail, and we don't want to leak the temp on error). Use errdefer to free temp on failure.
- **The heading loop lives inside parseAll (lines 298-324).** To skip makeSlug, we'd need to either
  modify parseAll or post-process. Anti-pattern says don't modify parseAll's signature. The post-processing
  approach: parseAll computes all slugs, then update() replaces eligible ones. This computes slugs
  we'll throw away for reused headings — acceptable for v1. If profiling shows this matters, refactor
  the loop later.
- **Dedup safety guarantee:** Headings before `edit_offset` are safe because their preceding heading
  context is identical (all preceding headings are also before the edit). After-edit-range headings
  may have different dedup suffixes if headings with same base slug were added/removed in the edit.
