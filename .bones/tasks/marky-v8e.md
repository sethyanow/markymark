---
id: marky-v8e
title: '[EPIC] markymark v1.0 Product Launch'
status: open
type: epic
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-peu]
---


## Goal
Ship markymark as a polished, discoverable product across all distribution channels with a clear forward-looking roadmap.

## Context
Alpha release (marky-peu) validates the CI/CD pipeline and crates.io metadata. This epic takes markymark from 'tagged alpha' to 'installable product' — the plugin installs cleanly from marketplace, binaries work on all 5 platforms via GitHub Releases download, and a researched roadmap guides future development.

## Requirements (IMMUTABLE)
1. Plugin installs from Claude marketplace with zero friction on all 5 platforms
2. CI pre-packages per-platform plugin archives (no binaries in git, no runtime download)
3. GitHub Releases contains per-platform plugin archives + standalone binaries
4. Plugin hooks (PreToolUse LSP-first) and skills (/markdown-check) work after install
5. README covers all distribution methods with copy-paste install instructions
6. Roadmap research complete for 3 tracks with prioritized proposals
7. At least one follow-up implementation epic created from highest-priority research

## Success Criteria
- [ ] `/plugin install markymark` works on macOS ARM, macOS Intel, Linux x86, Linux ARM, Windows x86
- [ ] Plugin archive on GitHub Releases contains correct binary per platform
- [ ] No binaries committed to git (download from releases only)
- [ ] LSP + MCP features work after marketplace install
- [ ] README has installation instructions for: marketplace, GitHub Releases, cargo install
- [ ] Roadmap research docs exist for all 3 tracks
- [ ] Marksman feature gap analysis complete
- [ ] At least 1 follow-up implementation epic created

## Anti-Patterns
- FORBIDDEN: Committing binaries to git (use CI pre-packaging)
- FORBIDDEN: Requiring Rust toolchain to install plugin
- FORBIDDEN: Shipping without testing on multiple platforms
- FORBIDDEN: Creating implementation epics without research backing
- FORBIDDEN: Publishing to marketplace before all platforms validated
