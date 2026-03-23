---
id: marky-88hp
title: 'Refactor document.zig: split into submodules (1067 lines, over 1000-line limit)'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

## Design

## Problem
document.zig has grown to 1067 lines, exceeding the 1000-line hard stop.

## Goal
Split document.zig into focused submodules while keeping the public API unchanged.

## Suggested split
- engine/document.zig: parseAll, DocumentEngine struct, public API (~300 lines)
- engine/parse_steps.zig: headings/links/tags/block_ids processing loops
- engine/slug.zig: makeSlug, slugifyText
- engine/blob.zig or keep in existing blob module: serializeState
- engine/document_test.zig: all test functions

## Success Criteria
- [ ] No file exceeds 1000 lines
- [ ] All 615+ Zig tests pass
- [ ] All 1009+ Rust tests pass
- [ ] Public API unchanged (no callers broken)
