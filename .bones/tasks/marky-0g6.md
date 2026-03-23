---
id: marky-0g6
title: 'resources.rs: realm hardcoded to None — MCP resources can''t target non-default realms'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

CodeRabbit MAJOR finding (PR #28).

resources.rs hardcodes realm: None in GetOutline and SearchSymbols CoreOperation calls (~lines 94-97, 127-130). Documents indexed under non-default realms are unreachable via MCP resources.

Fix: Parse optional 'realm' query param from the resource_uri and thread it through. Also update resource templates to expose realm={realm} so clients can discover scoped endpoints.

CodeRabbit suggested diff in PR #28 thread.
