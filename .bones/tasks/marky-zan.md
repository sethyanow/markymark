---
id: marky-zan
title: 'Implement real incremental parsing: edit() + parse(old_tree)'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9b, marky-8gp, marky-tfd, marky-tzq]
---





## What
Wire the actual tree-sitter incremental parsing: convert LSP edit ranges to InputEdit, call md_tree.edit(), then parser.parse(new_bytes, Some(&old_tree)). This is the core optimization that makes reparse O(edit_size) instead of O(document).

## Acceptance Criteria
- [ ] Parser::parse_incremental properly converts edit ranges to InputEdit
- [ ] MarkdownTree.edit() called with correct byte offsets and Point positions
- [ ] parser.parse() receives old_tree reference for incremental update
- [ ] New Ast built from incrementally-parsed MarkdownTree
- [ ] DocumentIndex still rebuilt from scratch (full correctness, Phase 1)
- [ ] Benchmark: single-char edit in 50KB doc is ≥10x faster than full reparse
- [ ] Correctness test: incremental parse of edit produces same AST as full reparse
- [ ] Stress test: 100 sequential single-char edits produce correct final AST

## Risk
MEDIUM — byte_to_point conversion must be exact. Off-by-one in edit coordinates causes tree-sitter to produce incorrect trees silently (no error, just wrong nodes). Need thorough test coverage comparing incremental vs full parse results.

## Files
- markymark-parser/src/lib.rs (Parser::parse_incremental)
- markymark-lsp/src/state.rs (change_document wiring)
- markymark-parser/tests/ (new incremental parsing tests)
- markymark-index/benches/memory.rs (new incremental benchmark)
