---
id: marky-4aa
title: mem::forget in DocumentIndex::from_ast leaks AST heap allocations
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-luy
---


Miri testing revealed that DocumentIndex::from_ast leaks heap allocations from the consumed Ast: the Box<DocumentArena> shell, Vec<Element> backing array, String source, and MarkdownTree. These are not freed because mem::forget(ast) prevents all Drop implementations. In an LSP context this happens on every file open/change. Fix requires ManuallyDrop + selective field drops, or the self_cell migration (marky-5yt) which eliminates the pattern entirely.
