---
id: marky-efm
title: 'LSP: Add JSON document support (symbol outline, diagnostics)'
status: open
type: feature
priority: 3
owner: sethyanow@users.noreply.github.com
---

markymark LSP doesn't support JSON files yet. The MCP server can process JSON via markymark-parser. Add JSON support to the LSP server so it can provide document symbols, diagnostics, and navigation for JSON files. Previous attempt failed because the LSP types weren't wired up. Dogfood opportunity: test against PR comment JSON files saved in repo.
