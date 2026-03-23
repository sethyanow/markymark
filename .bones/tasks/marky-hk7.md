---
id: marky-hk7
title: 'Audit DocumentArena::reset() safety: pub fn with dangling-reference hazard'
status: closed
type: bug
priority: 3
owner: Seth
---





## Context

`DocumentArena::reset()` (markymark-core/src/arena.rs:84) is a `pub fn` that invalidates all
arena-backed references. The doc comment warns about dangling references but the method is not
`unsafe`. The `&mut self` requirement provides partial protection (self_cell prevents mutable
access post-construction), but external crate consumers could misuse it.

**Current callers:** Only one — a unit test (arena.rs:207). Zero production callers.
The dec-041 investigation found arena reset saves 0.07% of reparse cost, so it was never
wired into any hot path.

## Decision

**Remove `reset()` entirely.** Dead code with zero production callers, no performance
justification (0.07% reparse savings per dec-041), and a public API hazard. Restriction
(pub(crate) or unsafe) adds ceremony to code nobody uses.

## Diagnosis

- **Root cause:** `pub fn reset(&mut self)` on `DocumentArena` invalidates all arena-backed
  references without `unsafe` marker. Exposed via `pub mod arena` to any downstream crate.
- **Evidence:** LSP `findReferences` — 2 refs total (definition + 1 unit test). 28 refs to
  `DocumentArena` across 6 files in 4 crates, none call `reset()`. Miri suite (14 tests)
  exercises DocumentArena extensively, never uses `reset()`.
- **Confidence:** HIGH

## Implementation

### Step 1: Remove the `reset()` method
File: `markymark-core/src/arena.rs`
Delete doc comment + method (lines 77-86):
```
    /// Reset the arena, deallocating all objects but keeping the first memory
    /// chunk for reuse. Excess chunks are returned to the global allocator.
    ///
    /// # Safety note
    ///
    /// All references into this arena become dangling after `reset()`. The
    /// caller must ensure no live references exist before calling this method.
    pub fn reset(&mut self) {
        self.0.reset();
    }
```

### Step 2: Remove the test for `reset()`
File: `markymark-core/src/arena.rs`
Delete test `document_arena_reset_clears_allocations` (lines 194-213)

### Step 3: Run crate tests
Command: `cargo nextest -p markymark-core`
Expected: All remaining tests pass, no compilation errors

### Step 4: Run workspace-wide check
Command: `cargo check --workspace`
Expected: Clean — confirms no other crate referenced `reset()`

### Step 5: Commit and push
Message: `fix(core): remove dead DocumentArena::reset() — pub fn with dangling-reference hazard`

## Success Criteria

- [x] `reset()` method removed from `DocumentArena`
- [x] Test `document_arena_reset_clears_allocations` removed
- [x] `cargo nextest -p markymark-core` passes
- [x] `cargo check --workspace` clean

## Log

- [2026-03-23T14:06:15Z] [Seth] Diagnosis (debugging-with-tools): reset() is dead code — LSP confirms exactly 1 caller (unit test at arena.rs:207), zero production callers across 4 crates that use DocumentArena (28 refs). Miri test suite (14 tests) never uses it. dec-041 found 0.07% reparse savings — no perf justification. Fix: remove method (lines 77-86) and test (lines 194-213). Confidence: HIGH.
- [2026-03-23T14:13:00Z] [Seth] Fix complete: removed reset() method (lines 77-86) and test document_arena_reset_clears_allocations (lines 194-213) from arena.rs. 132 markymark-core tests pass, workspace check clean. Committed ac4510f, pushed to dev.
