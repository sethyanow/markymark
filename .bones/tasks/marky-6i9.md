---
id: marky-6i9
title: 'markdown-check: implement get_diagnostics MCP or markymark check CLI'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

Recommendations from /markdown-check skill run:

1. **Implement get_diagnostics in markymark MCP** — The skill expects a markymark MCP tool get_diagnostics(workspace_path?, file_path?) that returns diagnostics (broken links, duplicate slugs, unclosed XML). Documented in docs/tools/get_diagnostics.md but not implemented in markymark-mcp. Adding it would let the markdown-check skill call MCP and report for the whole workspace.

2. **Or add a markymark check CLI** — A subcommand (e.g. `markymark check [path]`) that indexes the workspace and prints diagnostics so scripts/CI can run full-workspace checks without the editor.

3. **Interim: scripts/markdown_check_lsp.py** — Python script that drives markymark LSP (spawn --lsp, initialize, didOpen all .md files, collect publishDiagnostics) and prints a markdown-check style report. Implemented so /markdown-check can run it until MCP or CLI is available.
