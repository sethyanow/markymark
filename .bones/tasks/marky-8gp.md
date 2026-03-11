---
id: marky-8gp
title: Store Parser instance in ServerState instead of per-call creation
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9b]
---




## What
Move Parser from being created fresh in build_markdown_index() to being a field on ServerState. This is prerequisite for incremental parsing — the Parser holds the MarkdownParser which needs to persist across parse calls for tree reuse.

## Acceptance Criteria
- [ ] ServerState has a parser: Parser field
- [ ] build_markdown_index uses self.parser instead of Parser::new()
- [ ] Parser::parse takes &mut self (already does)
- [ ] All existing tests pass unchanged
- [ ] No new allocations per parse call (Parser::new() eliminated)

## Risk
Low — structural refactor, no behavior change. Parser is already &mut self.

## Files
- markymark-lsp/src/state.rs (add parser field, update build_markdown_index)
- markymark-lsp/tests/*.rs (may need to update test helpers)
