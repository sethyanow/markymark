---
id: marky-lkj.13
title: Verify JSONC parsing with real comment syntax
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


## Context
JSONC (.jsonc) currently falls back to tree-sitter-json with comment "tree-sitter-json tolerates comments." This assumption needs verification — tree-sitter-json may error on // or /* */ comment nodes depending on the grammar version.

## Requirements
- Write integration tests with real .jsonc content: // line comments, /* block comments */, trailing commas
- Verify tree-sitter-json 0.24 actually produces valid AST for commented JSON
- If it errors or drops keys near comments: either fix the json parser to skip error nodes gracefully, or route .jsonc through the json5 parser instead
- Document the finding

## Acceptance Criteria
- Test with // comments, /* */ comments, trailing commas in .jsonc
- Either: tests pass confirming tree-sitter-json handles JSONC, or .jsonc is rerouted to json5 parser
- No silent data loss (keys adjacent to comments must still be indexed)
