---
id: marky-lgt
title: Phase 4 cleanup + cross-cutting verification
status: closed
type: task
priority: 2
parent: marky-nxc
---






## Context

Final task in epic marky-nxc. All structural decomposition (Phases 1-3) is complete.
Remaining work is low-severity cleanup (bare `.unwrap()` on Mutex/pop → `.expect()`)
and cross-cutting verification (full test suite under both feature sets + clippy).

Also: Phase 3's `did_change()` criterion needs verification — confirm it was
intentionally left unchanged (it should be, as no extraction tasks touched it).

**Blocked by:** marky-zr6 (closed)
**Unlocks:** Closing epic marky-nxc (all remaining criteria)

## Requirements

1. Verify `did_change()` in server.rs is unchanged (Phase 3 criterion).
2. Replace 9x `self.debounce.lock().unwrap()` / `debounce.lock().unwrap()` in server.rs
   with `.expect("debounce lock poisoned")`.
3. Replace 1x `stack.pop().unwrap()` in symbols.rs:203 with
   `.expect("stack non-empty after while-let guard")`.
4. Full test suite passes under default features.
5. Full test suite passes under all features.
6. Clippy clean across workspace.

## Design

### did_change() verification

`did_change()` spans L181-299 in the current server.rs. The epic Design section explicitly
notes: "did_change() (117 lines) is intentionally left as-is: its complexity is interlocking
state (debounce handles, generation counters, async spawns), not switch-on-type dispatch."
Verify it was not modified by any extraction tasks and check the criterion.

### Mutex unwrap → expect (9 occurrences)

Current locations (verified):
- L172: `did_open` — `self.debounce.lock().unwrap()`
- L208: `did_change` — `self.debounce.lock().unwrap()`
- L233: `did_change` spawned task — `debounce.lock().unwrap()`
- L266: `did_change` spawned task — `debounce.lock().unwrap()`
- L294: `did_change` — `self.debounce.lock().unwrap()`
- L306: `did_close` — `self.debounce.lock().unwrap()`
- L737: `drain_pending` (cfg-gated) — `self.debounce.lock().unwrap()`
- L757: `try_apply_drained` (cfg-gated) — `self.debounce.lock().unwrap()`
- L771: `document_generations_count` (cfg-gated) — `self.debounce.lock().unwrap()`

Note: Epic originally said "8 occurrences" but listed 9 line positions. Actual count is 9.

All are `Mutex::lock()` on the debounce state — same expect message for all:
`.expect("debounce lock poisoned")`.

### stack.pop().unwrap() (1 occurrence)

symbols.rs:203 — inside `outline_children_to_symbols`. The `while let Some(last) =
stack.last()` guard ensures the stack is non-empty when `.pop()` is called.
Replace with `.expect("stack non-empty after while-let guard")`.

## Implementation

### Step 1: Baseline — run all markymark-lsp tests, confirm GREEN
### Step 2: Verify did_change() is unchanged
- Read `did_change()` method (L181-299) — confirm it matches the pre-epic state
  (debounce with generation counters, spawned async task, multiple mutex acquisitions)
- Check off Phase 3 did_change criterion in epic skeleton
### Step 3: Replace 9x Mutex lock unwrap with expect in server.rs
- Find-and-replace `.lock().unwrap()` with `.lock().expect("debounce lock poisoned")` at all 9 locations
- Cargo check
### Step 4: Replace stack.pop().unwrap() in symbols.rs
- Replace `.pop().unwrap()` at symbols.rs:203 with `.pop().expect("stack non-empty after while-let guard")`
- Cargo check
### Step 5: Full verification — all feature sets + clippy
- `cargo nextest` (workspace-wide, default features — matches epic criterion)
- `cargo nextest --all-features` (workspace-wide, all features — matches epic criterion)
- `cargo clippy --workspace --all-targets`
- Check off Cross-cutting criteria in epic skeleton

## Success Criteria

- [x] `did_change()` verified intentionally unchanged — Phase 3 criterion checked in epic
- [x] 9x `.lock().unwrap()` in server.rs replaced with `.expect("debounce lock poisoned")`
- [x] 1x `.pop().unwrap()` in symbols.rs replaced with `.expect("stack non-empty after while-let guard")`
- [x] All tests pass under default features
- [x] All tests pass under all features
- [x] Clippy clean across workspace
- [x] All Phase 4 and Cross-cutting criteria checked in epic skeleton

## Anti-Patterns

- Do NOT change any logic — this is purely replacing `.unwrap()` with `.expect()`.
- Do NOT touch `did_change()` implementation — only verify it's unchanged.
- Do NOT skip the `--all-features` test run — the embeddings feature flag changes behavior.

## Log

- [2026-03-23T12:24:31Z] [Seth] Debrief: Pure mechanical replacement, no surprises. SRE caught Step 5 scope narrowing (fixed pre-execution). All 9+1 unwrap locations verified at exact claimed lines. Epic count corrected 8→9. Reflections: Skeleton accuracy excellent — only SRE-caught gap was verification scope. No user corrections. No cross-pollination. docs/MEMORY.md Active Work section stale (2026-03-05). All marky-nxc criteria now satisfied — Step 4 review-implementation needed to close epic.
