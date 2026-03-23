---
id: marky-hk7
title: 'Audit DocumentArena::reset() safety: pub fn with dangling-reference hazard'
status: open
type: bug
priority: 3
---

## Context

`DocumentArena::reset()` (markymark-core/src/arena.rs:84) is a `pub fn` that invalidates all
arena-backed references. The doc comment warns about dangling references but the method is not
`unsafe`. The `&mut self` requirement provides partial protection (self_cell prevents mutable
access post-construction), but external crate consumers could misuse it.

**Current callers:** Only one — a unit test (arena.rs:207). Zero production callers.
The dec-041 investigation found arena reset saves 0.07% of reparse cost, so it was never
wired into any hot path.

## Decision needed

Two options:
1. **Remove `reset()`** — dead code, no callers, avoids the hazard entirely
2. **Keep but restrict** — make it `pub(crate)` or add `# Safety` and mark `unsafe`

## Success Criteria

- [ ] `reset()` either removed or restricted to prevent external misuse
