---
id: marky-v60
title: 'Task 2: Zig slug reuse — skip makeSlug for headings before edit range'
status: closed
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

- [x] `DocumentEngine` has `slug_reuse_count: u32` field, reset to 0 on each update
- [x] `update()` post-processes new headings between parseAll success and freeState, reading old slugs from `self.headings`
- [x] For new headings with `source_offset < edit_offset`: dupe old slug, free parseAll slug, replace. Dupe BEFORE free (OOM safety).
- [x] `slug_reuse_count` incremented for each reused slug
- [x] Zero-value edit range (0/0/0) bypasses reuse logic via explicit check (not just arithmetic coincidence), count stays 0
- [x] `marky_engine_get_slug_reuse_count` C export + Rust `slug_reuse_count()` wrapper
- [x] Zig test: edit at end of document → headings at start reuse slugs (slug_reuse_count > 0)
- [x] Zig test: edit inside heading → that heading's slug recomputed (count reflects partial reuse)
- [x] Zig test: heading exactly at edit_offset → NOT reused (strict less-than boundary)
- [x] Zig test: slug_reuse_count resets between updates (reuse update → zero-range update → count == 0)
- [x] Rust FFI test: edit range after headings → slug_reuse_count > 0
- [x] Rust FFI test: zero-value range → slug_reuse_count == 0
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
- In `update()`, remove the three `_ = edit_*;` discards (lines 87-89)
- After `parseAll` succeeds (line 128), BEFORE `self.freeState()` (line 131), insert reuse pass:
  1. Explicit zero-value check: `if (edit_offset == 0 and edit_old_len == 0 and edit_new_len == 0)` → skip reuse entirely
  2. Iterate `new_headings` with pointer capture (`|*new_h|`):
     - If `new_h.source_offset < edit_offset`: scan `self.headings` (old, still valid) for matching `source_offset`
     - If found: `const duped = allocator.dupe(u8, old_h.slug) catch continue;` (OOM = skip this heading)
     - Then: `allocator.free(new_h.slug);` (free parseAll's fresh slug AFTER dupe succeeds)
     - Then: `new_h.slug = duped; self.slug_reuse_count += 1;`
  3. No match found or source_offset >= edit_offset: keep parseAll-computed slug
- **No temp array needed** — `self.headings` is still valid between parseAll and freeState. Old slug
  bytes are read directly and duped into new allocations. freeState then frees the originals safely.

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

- **No temp array needed (SRE simplification):** `self.headings` (old data) remains valid between
  parseAll success (line 128) and `freeState()` (line 131). The reuse pass reads directly from
  `self.headings` and dupes slug bytes into new allocations. No temp snapshot required.
- **Matching old→new headings:** For headings before `edit_offset`, the `source_offset` is identical
  in old and new text (no byte shift). Linear scan over old headings is fine (O(n*m) where n,m < 100).
- **freeState ordering:** `update()` calls `freeState()` at line 131, AFTER parseAll succeeds at line 128.
  The reuse pass MUST be inserted between these two lines. After freeState, old slug memory is freed —
  reading it is undefined behavior (anti-pattern: NO pointer reuse across freeState boundary).
- **The heading loop lives inside parseAll (lines 307-332).** To skip makeSlug, we'd need to either
  modify parseAll or post-process. Anti-pattern says don't modify parseAll's signature. The post-processing
  approach: parseAll computes all slugs, then update() replaces eligible ones. This computes slugs
  we'll throw away for reused headings — acceptable for v1. If profiling shows this matters, refactor
  the loop later.
- **Dedup safety guarantee:** Headings before `edit_offset` are safe because their preceding heading
  context is identical (all preceding headings are also before the edit). After-edit-range headings
  may have different dedup suffixes if headings with same base slug were added/removed in the edit.
  Verified: `makeSlug` dedup scans `extraction.headings[0..i]` (lines 30-36 in document_helpers.zig).

### Adversarial Failure Catalog (SRE)

**OOM during slug replacement**
- Assumption: `allocator.dupe()` succeeds for every reusable heading
- Betrayal: OOM after duping slugs for headings 0-2 but failing on heading 3
- Consequence: If new slug freed BEFORE dupe attempt: use-after-free. If dupe attempted FIRST: heading 3
  keeps its fresh parseAll slug, headings 0-2 have duped old slugs. Mixed state but all allocator-owned.
- Mitigation: **Dupe old slug first, free new slug second.** On OOM, `catch continue` — heading keeps
  fresh slug. `freeHeadings` (document_free.zig:26) handles both origins uniformly via `allocator.free`.

**Stale edit_offset (Input Hostility)**
- Assumption: `edit_offset` corresponds to the text being parsed
- Betrayal: Caller passes stale offset from a different text version, causing `source_offset < edit_offset`
  to be true for all headings (over-reuse)
- Consequence: Benign — old and new slugs are identical when text hasn't changed in those regions.
  If text DID change, source_offsets won't match between old and new headings, so the matching
  loop finds nothing and no reuse occurs.
- Mitigation: Structural — offset matching is the safety net against stale ranges.

**slug_reuse_count accumulation (Temporal Betrayal)**
- Assumption: Count resets to 0 each update
- Betrayal: If reset forgotten, count accumulates, giving wrong values across updates
- Consequence: Rust FFI `count > 0` test passes even if reuse didn't happen THIS update
- Mitigation: Success criterion requires Zig test: reuse update → zero-range update → count == 0.
  Tests count reset across sequential updates on same engine instance.

**Encoding boundary (edit_offset vs source_offset)**
- Assumption: Both are byte offsets in UTF-8 text
- Betrayal: LSP protocol uses UTF-16 code unit offsets — conversion is Task 3's responsibility
- Consequence: Wrong headings reused if offsets are in different units
- Mitigation: This task uses raw byte offsets. Task 3 (LSP threading) must convert UTF-16 → bytes.
  All tests in this task use explicit byte offsets to avoid ambiguity.

## Log

- [2026-03-24T14:45:41Z] [Seth] Implemented slug reuse in DocumentEngine.update(). Simplified skeleton's temp-array approach: read directly from self.headings between parseAll and freeState. 12/12 criteria met. 11 Zig tests + 2 Rust FFI tests + 7 adversarial tests (incl GPA leak check for 100 reuse cycles). Also fixed 2 pre-existing export test arg count bugs from marky-f1w. Key decisions: dupe-before-free OOM ordering, catch break on inner loop, explicit zero-value check. 728/728 Zig, 101/101 Rust kernel, 1286/1286 workspace.
