---
id: marky-m8b
title: Refactor markymark-lsp state.rs into submodules
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---

## Context

markymark-lsp/src/state.rs has grown well beyond the 500-line threshold and now mixes multiple responsibilities (incremental edit application, indexing orchestration, rename/reference/symbol operations, and helper utilities).

## Goal

Split markymark-lsp/src/state.rs into cohesive submodules while preserving behavior and test coverage.

## Proposed module split

- markymark-lsp/src/state/mod.rs (public API + ServerState struct)
- markymark-lsp/src/state/incremental.rs (DocumentChange handling, byte bounds/clamp logic)
- markymark-lsp/src/state/navigation.rs (symbol lookup/goto/references helpers)
- markymark-lsp/src/state/rename.rs (prepare_rename/rename logic)
- markymark-lsp/src/state/completion.rs (completion context and candidate generation)
- markymark-lsp/src/state/diagnostics.rs (diagnostic computation helpers)

## Acceptance criteria

- state.rs responsibilities are split into submodules with clear boundaries
- Existing public behavior remains unchanged
- All existing markymark-lsp tests remain green
- No significant net increase in complexity or duplication
- New internal module docs explain ownership/responsibility boundaries

## Notes

This is a refactor task, not a feature change. Prioritize small, reviewable commits and maintain test green status throughout.

## Design

## Refined Plan (2026-02-18)

Supersedes marky-ulw (test-shuffling). This is the full modular decomposition.

### Module split (from actual code analysis)

| New file | What moves there | ~Lines (prod+test) |
|----------|------------------|--------------------|
| `state/mod.rs` | ServerState struct, Default, new(), document lifecycle (open/change/close/get*), document_kind_from_uri, build_markdown_index wrappers, types (DocumentChange, CompletionContext, etc.) | ~310 |
| `state/completion.rs` | detect_completion_context, completion_at, CompletionCandidate types | ~230 |
| `state/rename.rs` | prepare_rename_at, rename_at, find_wiki_link_heading_range, find_markdown_link_anchor_range, PrepareRenameResult, RenameEdit types | ~280 |
| `state/navigation.rs` | symbol_at_position, SymbolAtPosition, StructuredKeyInfo | ~90 |
| `incremental.rs` (existing) | Gets apply_document_changes orchestration method + all 334 lines of incremental tests from old state.rs | ~940 |

### Key architectural decisions:
- apply_document_changes (190 lines, the big orchestrator) moves to incremental.rs where its logic belongs. It becomes a free function taking &mut ServerState, or stays as impl ServerState in incremental.rs
- Each submodule uses `impl ServerState` blocks (idiomatic Rust pattern)
- Types that are only used by one module move with that module
- Tests co-locate with the code they test (no more test-in-wrong-file)

### Execution order (change -> test -> commit each):
1. Create state/ dir, move state.rs -> state/mod.rs (rename only)
2. Extract completion.rs (types + 2 methods + tests)
3. Extract rename.rs (types + 2 methods + 2 helpers + tests)
4. Extract navigation.rs (1 method + types)
5. Move apply_document_changes + owned-data extraction to incremental.rs
6. Move remaining incremental tests from state/ to incremental.rs
7. Final verification: all tests green, clippy clean

### Success criteria:
- [ ] No file exceeds 500 lines (production code)
- [ ] All existing tests pass
- [ ] cargo clippy clean
- [ ] cargo fmt clean
- [ ] Each commit is independently green
