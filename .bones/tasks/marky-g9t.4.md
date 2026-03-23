---
id: marky-g9t.4
title: Migrate markymark-index document types to arena lifetimes
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9t.3, marky-luy]
parent: marky-g9t
---





Thread 'arena lifetime through all 8 index entry types in markymark-index/src/document.rs (~401 lines). HeadingEntry, WikiLinkEntry, etc. get &'arena str fields. DocumentIndex borrows from the document's arena. Update DocumentIndex::from_ast() to borrow from parser arena instead of cloning strings.

Success: cargo test -p markymark-index passes.

## Design

## Completed implementation

- DocumentIndex::from_ast() now borrows from parser arena instead of cloning
- Added Ast::arena(), Ast::into_arena(), Ast::arena_ptr() in markymark-parser
- All 8 index entry types use arena refs; only slugs and url#anchor (when present) are newly allocated
- Uses ptr::read + mem::forget to transfer arena ownership without borrow conflict
- All call sites updated to pass ast by value
- cargo test -p markymark-index: PASS
- cargo test --workspace: PASS
