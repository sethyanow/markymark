---
id: marky-g9b
title: Incremental tree-sitter parsing for LSP didChange
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
---






## Problem

Every keystroke triggers a full document re-parse: Parser::new() + parse(full_text) + DocumentIndex::from_ast(). For a 50KB markdown file, this costs ~3.4ms per keystroke. Tree-sitter's incremental parsing can reduce this to O(edit_size) by reusing the previous parse tree.

## Current Architecture (3 layers of waste)

1. **LSP layer**: TextDocumentSyncKind::FULL — editor sends entire document text on every change (server.rs:91)
2. **Parser layer**: Parser::parse() creates a fresh MarkdownParser and parses from scratch every time. parse_incremental() is a stub that just calls parse() (lib.rs:55-67)
3. **Index layer**: DocumentIndex::from_ast() rebuilds all headings, slugs, TOC, outline, links, tags, blocks from scratch. ServerState::change_document() drops the old index entirely (state.rs:208-228)

## Proposed Solution (phased)

### Phase 1: Wire incremental tree-sitter parsing
- Change TextDocumentSyncKind::FULL → INCREMENTAL in server capabilities
- Store the MarkdownTree (old parse tree) alongside document text in ServerState
- Convert LSP TextDocumentContentChangeEvent ranges to tree-sitter InputEdit
- Call md_tree.edit() + parser.parse(new_bytes, Some(&old_tree))
- Still rebuild full DocumentIndex from the new AST (simplest correct approach)

### Phase 2: Retain Parser per ServerState (avoid re-init)
- Move Parser into ServerState instead of creating one per build_markdown_index() call
- Parser holds the tree-sitter MarkdownParser which has internal state for incremental mode

### Phase 3 (future): Incremental indexing
- Diff old/new ASTs to identify changed sections
- Only re-extract headings/links/tags in changed regions
- Patch DocumentIndex instead of rebuilding
- Significantly harder — defer unless Phase 1 benchmarks demand it

## Key References

- docs/rust_crates/tree-sitter.md "Incremental Parsing with MarkdownTree" section
- tree_sitter::InputEdit struct: {start_byte, old_end_byte, new_end_byte, start_position, old_end_position, new_end_position}
- MarkdownTree::edit(&mut self, edits: &[InputEdit])
- MarkdownParser::parse(&mut self, bytes: &[u8], old_tree: Option<&MarkdownTree>)

## Files to Change

- markymark-lsp/src/server.rs — TextDocumentSyncKind, did_change handler
- markymark-lsp/src/state.rs — ServerState (store Parser + MarkdownTree per doc)
- markymark-parser/src/lib.rs — Parser::parse_incremental (real implementation)
- markymark-parser/src/ast.rs — Ast needs to expose/accept MarkdownTree for reuse

## Success Criteria

1. TextDocumentSyncKind::INCREMENTAL in server capabilities
2. Parser reuses old MarkdownTree on didChange via tree-sitter edit() API
3. Benchmark: reparse of single-char edit in 50KB doc is ≥10x faster than full reparse
4. All existing LSP tests pass (backward compatible)
5. New test: incremental parse produces identical AST to full reparse for same input
6. No regressions in document_symbol, hover, goto_definition, references, completion
