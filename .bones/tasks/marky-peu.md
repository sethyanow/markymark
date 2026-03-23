---
id: marky-peu
title: 'Release markymark v0.1.0-alpha.1: plugin hooks, crates.io, GitHub release'
status: closed
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
---












## Goal
Ship markymark's first alpha release across three distribution channels: Claude Code plugin (with integrated hooks), crates.io (all 6 crates), and GitHub Releases (multi-platform binaries).

## Context
markymark-plugin already has 95% of marketplace infrastructure (plugin.json, scripts, tests, CI/CD). Missing pieces:
- Plugin hooks not integrated (only exist as examples)
- Crates.io metadata incomplete (5/6 crates lack descriptions, keywords, categories)
- No first release has been tagged yet

## Requirements (IMMUTABLE)
1. Plugin hooks integrated into markymark-plugin (PreToolUse suggest-lsp, SessionStart context)
2. All 6 crates publishable to crates.io with proper metadata
3. First alpha release tagged and built via existing CI/CD
4. Release workflow documented for future releases

## Success Criteria
- [ ] markymark-plugin/hooks/ contains working hooks.json with PreToolUse hook
- [ ] All 6 Cargo.toml files have description, keywords, categories
- [ ] cargo publish --dry-run succeeds for all crates in dependency order
- [ ] git tag v0.1.0-alpha.1 triggers successful CI build
- [ ] GitHub Release contains plugin archive + platform binaries
- [ ] RELEASING.md documents the release process

## Anti-Patterns
- FORBIDDEN: Publishing to crates.io without dry-run verification
- FORBIDDEN: Tagging release before all metadata is correct
- FORBIDDEN: Skipping plugin hook tests

## Approaches Considered
1. CHOSEN: Alpha release (v0.1.0-alpha.1) to validate the pipeline before stable
2. REJECTED: Going straight to v0.1.0 stable — too risky for first release, no way to test pipeline
