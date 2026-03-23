---
id: marky-agk
title: Polish plugin hooks, skills, and configuration
status: open
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-peu, marky-9dx]
---



## Goal
Ensure the markymark plugin provides a complete, polished Claude Code experience out of the box.

## Requirements
1. Review and refine PreToolUse hook (suggest-lsp.sh) for reliability
2. Verify hook fires correctly for .md and .mdx file reads
3. Review /markdown-check skill output quality and formatting
4. Ensure .lsp.json and .mcp.json configs are correct for all installation methods
5. Test plugin in fresh Claude Code environment (no prior state)
6. Fix marky-9dx (test_select_binary.sh failures) as part of this refinement

## Tasks
- [ ] Review suggest-lsp.sh hook behavior with real Claude Code sessions
- [ ] Validate .lsp.json language server config triggers for markdown files
- [ ] Validate .mcp.json MCP server starts correctly with workspace roots
- [ ] Test /markdown-check skill produces useful diagnostics output
- [ ] Fix test_select_binary.sh failures (marky-9dx)
- [ ] Ensure all configs use ${CLAUDE_PLUGIN_ROOT} correctly (no hardcoded paths)
- [ ] Add any missing hook events (e.g., PostToolUse for write validation?)

## Success Criteria
- [ ] Plugin hooks fire correctly during Claude Code sessions
- [ ] /markdown-check produces actionable diagnostics
- [ ] All plugin tests pass (test_hooks.sh, test_select_binary.sh)
- [ ] Plugin works in fresh install with no prior configuration
