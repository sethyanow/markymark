---
id: marky-7cl
title: Polish README and installation documentation
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8pt]
---


## Goal
Create comprehensive, copy-paste-ready installation instructions for all distribution channels.

## Requirements
1. README.md covers 3 installation methods:
   - Claude marketplace: `/plugin install markymark`
   - GitHub Releases: download platform-specific archive, extract, point Claude Code to it
   - Cargo: `cargo install markymark-cli` (binary only, no plugin features)
2. Feature overview section showing what LSP and MCP provide
3. Configuration section for plugin hooks and skills
4. Troubleshooting section for common issues
5. Platform-specific notes where needed (Windows PowerShell vs bash)

## Success Criteria
- [ ] README has install instructions for all 3 methods
- [ ] Each method has copy-paste commands
- [ ] Feature overview accurately reflects current capabilities
- [ ] Troubleshooting covers: binary not found, permissions, platform detection failure
