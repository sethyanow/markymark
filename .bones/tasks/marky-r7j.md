---
id: marky-r7j
title: 'prompts.rs: realm hardcoded to None — prompts can''t target non-default realms'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

CodeRabbit MAJOR finding (PR #28).

prompts.rs has realm: None hardcoded in three CoreOperation calls (ExportIndex x2, FindReferences). Callers cannot target documents indexed under non-default realms via MCP prompts.

Affected lines: ~123, ~219-223, ~250-253 in markymark-mcp/src/prompts.rs

Fix: Accept optional 'realm' argument in the prompt handler, thread it through to each CoreOperation call.

CodeRabbit suggested diff in PR #28 thread — adds PromptArgument for realm and plumbs it through.
