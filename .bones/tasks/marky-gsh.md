---
id: marky-gsh
title: 'P0 Refactor: server.rs exceeds 1000-line hard limit (1068 lines)'
status: closed
type: task
priority: 0
owner: sethyanow@users.noreply.github.com
---

server.rs is at 1068 lines, violating rule-004 hard stop. Needs immediate refactoring into submodules.

## Current state
- markymark-lsp/src/server.rs: 1068 lines
- Contains: LSP handler implementations, capability setup, request routing, response building

## Suggested split
1. server.rs — core LspServer struct, initialize/shutdown, capability negotiation (~200 lines)
2. handlers.rs — textDocument/* handlers (didOpen, didChange, documentSymbol, etc.) (~400 lines)
3. completion.rs — completion provider logic (~200 lines)
4. diagnostics.rs — diagnostic publishing (~100 lines)
5. Remaining helper functions grouped by concern

## Constraints
- Must preserve all existing tests
- Must maintain tower-lsp trait impl on single struct
- state.rs at 996 lines is approaching limit too — consider in refactor plan

## Triggered by
rule-004: Hard stop at 1000 lines
