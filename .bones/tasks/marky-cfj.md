---
id: marky-cfj
title: Fix and submit Claude marketplace listing
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8pt]
---


## Goal
Get markymark listed in the Claude Code plugin marketplace so users can install via `/plugin install markymark`.

## Context
Previous attempt (markdown-mcp-6ia) partially worked but had issues: binary not included, install flow broken. With CI pre-packaging in place, marketplace install should deliver a working plugin.

## Requirements
1. Research current Claude marketplace submission process (may have changed since Feb 2026)
2. Ensure plugin.json has all required marketplace metadata
3. Create marketplace.json if required by submission process
4. Submit plugin for marketplace listing
5. Test `/plugin install markymark` on at least 2 platforms after listing

## Tasks
- [ ] Research marketplace submission process
- [ ] Update plugin manifest with marketplace metadata
- [ ] Submit to marketplace
- [ ] Verify installation on macOS ARM
- [ ] Verify installation on Linux x86_64
- [ ] Verify LSP features post-install
- [ ] Verify MCP tools post-install
- [ ] Verify /markdown-check skill works

## Notes
The marketplace process may require the plugin to download its binary from GitHub Releases rather than bundling it. If so, adjust select-binary.sh to download on first run as fallback. The CI pre-packaging task provides the GitHub Releases assets this depends on.

## Success Criteria
- [ ] markymark listed in Claude marketplace
- [ ] `/plugin install markymark` works on macOS and Linux
- [ ] All plugin features (LSP, MCP, hooks, skills) work after marketplace install
