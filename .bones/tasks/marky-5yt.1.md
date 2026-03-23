---
id: marky-5yt.1
title: Migrate Ast to self_cell-backed owner/dependent model
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-5yt
---


Phase 1 of marky-5yt: replace Ast self-referential 'static marker pattern with a self_cell owner/dependent structure while preserving current public API behavior.

## Design

## Goal
Migrate Ast to a self_cell owner/dependent model so arena-borrowed elements are tied to Ast lifetime without internal 'static marker storage.

## Failure Modes / Edge Cases
- MarkdownTree take/reuse path must still work (take_md_tree behavior unchanged).
- Extractor methods must not allocate regressively or change ordering.
- Empty document parse path must remain valid.
- Drop order must keep arena alive until dependent elements are dropped.

## Test Plan
1. Add failing regression test first for Ast construction/extractor behavior parity.
2. Run parser targeted tests.
3. Run cross-crate checks that exercise Ast consumers.

## Success Criteria
- [ ] Ast internal root storage no longer uses Vec<Element<'static>>.
- [ ] No new unsafe lifetime transmute in Ast build path.
- [ ] Existing parser behavior tests pass.
- [ ] New regression test added and passing.
