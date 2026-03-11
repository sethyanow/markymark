---
id: marky-5yt.2
title: Migrate DocumentIndex to self_cell/ouroboros and remove 'static field storage
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-5yt
---


Phase 2 of marky-5yt: remove internal 'static markers from DocumentIndex fields by moving to a self-referential crate-backed model with equivalent lookup APIs.

## Design

## Goal
Migrate DocumentIndex internals to a self-referential crate model (self_cell or ouroboros) removing internal 'static field storage while preserving APIs.

## Failure Modes / Edge Cases
- Must preserve Send/Sync compatibility needed by LSP server state.
- Lookup maps must keep stable semantics for duplicates and insertion order assumptions.
- Incremental indexing/state integration cannot regress.
- Arena ownership transfer must avoid leaks/double-drops.

## Test Plan
1. Add failing regression(s) before refactor.
2. Run markymark-index and markymark-lsp test suites.
3. Run Miri arena safety checks.

## Success Criteria
- [ ] DocumentIndex fields no longer store &'static data.
- [ ] No ptr::read + mem::forget ownership transfer in from_ast.
- [ ] Existing index/lsp tests pass.
- [ ] Miri safety tests pass.
