---
id: marky-y1gm
title: '[EPIC] Documentation overhaul: Starlight docs site + README refresh'
status: open
type: epic
priority: 1
owner: sethyanow@users.noreply.github.com
---


Full documentation overhaul for markymark v0.4.x release. Separate Starlight (Astro) docs site with comprehensive user and contributor documentation. README rewritten as concise landing page.

## Design

## Requirements (IMMUTABLE)

- Starlight (Astro) docs site scaffolded in docs-site/ directory
- README.md rewritten as concise landing page linking to docs site
- About page explains what markymark is for laypeople (no LSP/MCP jargon upfront)
- Installation guide covers all platforms and methods (cargo, binaries, VS Code marketplace, Claude Code plugin)
- Quick-start guide gets users productive in 5 minutes
- Usage guide covers: workspace management, navigation, diagnostics, refactoring, search
- Agent tutorial walks through markymark as MCP server for AI agents with real examples
- Editor setup guides for VS Code, Neovim, and Claude Code
- LSP features reference documents all capabilities with examples
- MCP tools reference documents ALL tools with parameters and examples (not just a subset)
- Architecture overview covers crate map, parser pipeline (tree-sitter + md4c), and indexing
- Contributing guide covers build/test/lint, project structure, and code style
- Troubleshooting page covers common issues and fixes
- FAQ page addresses common questions
- All 7 workspace crates documented (including markymark-kernels)

## Success Criteria (MUST ALL BE TRUE)

- [ ] Starlight site builds without errors (astro build)
- [ ] README.md is under 100 lines and links to docs site
- [ ] All 22 doc pages listed in structure exist with substantive content
- [ ] MCP tools reference covers all tools (verify against actual tool list in code)
- [ ] LSP features reference covers all capabilities
- [ ] markymark-kernels crate is documented in architecture
- [ ] Agent tutorial includes working examples with Claude Code
- [ ] No broken internal links in docs site
- [ ] Pre-commit hooks passing

## Anti-Patterns (FORBIDDEN)

- NO placeholder/stub pages with TODO content (every page must have real content or don't create it yet)
- NO copying internal MEMORY.md verbatim into public docs (extract and rewrite for external audience)
- NO Node.js lockfile committed without .gitignore for node_modules (keep repo clean)
- NO documenting features that don't exist yet (document current v0.4.x capabilities only)
- NO jargon-heavy About page (must be understandable by someone who doesn't know what LSP or MCP means)
- NO incomplete MCP tools list (the current README lists a subset — docs must list ALL tools)

## Approach

Two-phase approach:
1. Scaffold Starlight site + rewrite README as landing page
2. Write content section-by-section, starting with user-facing pages (about, getting-started, usage, editors) then contributor-facing (architecture, contributing)

Content sourced from: current README, plugin/extension READMEs, MEMORY.md architectural decisions, actual code (MCP tools, LSP capabilities), and existing docs/plans/.

## Architecture

docs-site/ directory at repo root using Starlight (Astro). 8 content sections:

1. about.md — What is markymark? Layperson-friendly intro
2. getting-started/ — Installation + quick-start
3. usage/ — Day-to-day workflows (5 pages)
4. guides/ — Agent tutorial
5. editors/ — VS Code, Neovim, Claude Code setup (3 pages)
6. features/ — LSP reference, MCP tools reference, supported formats (3 pages)
7. architecture/ — Crate overview, parser pipeline, indexing (3 pages)
8. contributing/ — Development, project structure, guidelines (3 pages)

Plus: troubleshooting.md and faq.md at top level.

Total: ~22 content pages + index landing page.

README.md: Rewritten to ~80 lines — description, feature highlights, quick install, link to docs.

## Design Rationale

### Problem
markymark has mature code (7 crates, LSP+MCP dual server, Zig SIMD kernels) but documentation is scattered across internal agent docs, a stale README, and thin plugin/extension READMEs. New users and contributors have no clear entry point.

### Research Findings

**Codebase:**
- README.md lists 6 of 7 crates (missing markymark-kernels)
- MCP tools list in README is incomplete vs actual tool implementations
- docs/ has 58+ files but almost all are agent/developer reference, not user-facing
- MEMORY.md has excellent architectural decisions but is internal-only
- Plugin README and VS Code extension README exist but are thin

**External:**
- Comparable LSP servers (marksman, markdown-oxide) use concise README + separate docs
- MCP server best practices require documenting all tools with params for agent consumption
- Starlight (Astro) is modern, fast, has good content collections for organizing docs

### Approaches Considered

#### 1. Starlight docs site + lean README ✓

**What it is:** Separate Astro/Starlight docs site in docs-site/. README becomes concise landing page.

**Pros:**
- Clean separation of quick-start vs detailed docs
- Starlight handles navigation, search, dark mode out of box
- Matches industry standard for mature projects
- Can deploy to GitHub Pages

**Cons:**
- Adds Node.js dependency for docs build
- More initial setup than plain markdown

**Chosen because:** Project maturity warrants dedicated docs site. Starlight is lightweight and user chose it.

#### 2. Top-level markdown files ✗

**What it is:** ARCHITECTURE.md, CONTRIBUTING.md, etc. at repo root.

**Why explored:** Simplest approach, no build step.

**Pros:** Zero tooling, GitHub renders natively.
**Cons:** No navigation, no search, doesn't scale past 5-6 files, poor discoverability.

**REJECTED BECAUSE:** 22 pages of docs don't work as flat files at repo root.
**DO NOT REVISIT UNLESS:** Scope shrinks to <5 documents.

#### 3. docs/ subdirectory (no site generator) ✗

**What it is:** Markdown files in docs/ alongside existing agent docs.

**Why explored:** Avoids new tooling.

**Pros:** Simple, no build step.
**Cons:** Mixes user docs with agent reference material, no navigation/search, confusing directory.

**REJECTED BECAUSE:** docs/ already has 58+ agent files — adding user docs there creates confusion.
**DO NOT REVISIT UNLESS:** Agent docs are moved elsewhere first.

### Scope Boundaries

**In scope:**
- Starlight site scaffold and configuration
- All 22 content pages listed in structure
- README rewrite
- Content derived from existing sources (README, MEMORY.md, code, plugin READMEs)

**Out of scope (deferred):**
- GitHub Pages deployment CI (separate task after content exists)
- API reference / rustdoc hosting (separate concern)
- Internationalization (not needed at this stage)
- Blog/changelog section (changelog handled by release workflow)
- Custom Starlight theme/branding (defaults are fine for now)

### Open Questions
- Exact Starlight version/config — resolve during scaffold task
- Whether to include screenshots in editor setup guides — decide per-page

## Design Discovery (Reference Context)

### Key Decisions Made

| Question | User Answer | Implication |
|----------|-------------|-------------|
| Primary audience? | Both users and contributors | Need both user-facing and contributor sections |
| Scope? | Full docs overhaul | 22 pages across 8 sections |
| Changelog? | Handled by release workflow | Skip changelog in docs site |
| Doc location? | Separate docs site | Starlight (Astro) in docs-site/ |
| Site generator? | Starlight (Astro) | Node.js dependency, modern DX |
| Editor integrations? | VS Code + Neovim + Claude Code | 3 editor setup pages |
| About page? | Yes, layperson-friendly | No jargon upfront, explain value proposition plainly |
| FAQ? | Yes | Separate page for common questions |
| Usage guide? | Yes | 5 pages covering day-to-day workflows |
| Agent tutorial? | Yes | guides/agents.md with working Claude Code examples |

### Research Deep-Dives

#### Current Documentation State
**Question:** What docs exist and what is stale?
**Sources:** README.md, docs/ directory, plugin README, extension README, MEMORY.md
**Findings:**
- README lists 6/7 crates, incomplete MCP tools
- docs/ has 58+ agent files, zero user-facing docs
- MEMORY.md has excellent arch decisions (internal only)
- Plugin/extension READMEs are thin but functional
**Conclusion:** Need complete docs overhaul, not just README patch

#### Comparable Project Docs
**Question:** What do well-documented LSP/MCP projects look like?
**Sources:** marksman, markdown-oxide, MCP official docs, Claude Code plugin docs
**Findings:**
- Concise README linking to full docs is standard
- MCP servers must document all tools with params
- Per-editor setup guides are expected
**Conclusion:** Starlight site with comprehensive tool reference matches best practices

### Dead-End Paths
None — documentation is a well-understood problem space.

### Open Concerns Raised
- None raised during brainstorming
