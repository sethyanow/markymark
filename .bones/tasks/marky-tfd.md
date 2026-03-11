---
id: marky-tfd
title: Store MarkdownTree per document for incremental tree-sitter reuse
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9b, marky-8gp]
---




## What
Store the MarkdownTree alongside document text in ServerState so it can be passed to the next parse call. Currently the Ast owns the MarkdownTree and it's dropped when DocumentIndex is built.

## Design Options
Option A: Store MarkdownTree in a separate HashMap<DocumentUri, MarkdownTree> in ServerState
Option B: Return MarkdownTree from Ast (Ast::into_parts() -> (DocumentArena, MarkdownTree))
Option C: Store MarkdownTree inside DocumentIndex alongside the arena

Recommend Option A — cleanest separation, MarkdownTree is only needed by the parser layer.

## Acceptance Criteria
- [ ] ServerState stores MarkdownTree per open document
- [ ] MarkdownTree is updated on each parse
- [ ] MarkdownTree is removed on document close
- [ ] Ast exposes the MarkdownTree for extraction (pub method or into_parts)
- [ ] All existing tests pass

## Risk
Medium — Ast currently owns MarkdownTree with #[allow(dead_code)]. Need to expose it without breaking the self-referential arena pattern. MarkdownTree must NOT reference arena memory.

## Files
- markymark-parser/src/ast.rs (expose MarkdownTree extraction)
- markymark-lsp/src/state.rs (store MarkdownTree per doc)
