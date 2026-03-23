---
id: marky-wvqy
title: Scaffold Starlight docs site with navigation structure
status: open
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-y1gm
---


Set up the Starlight (Astro) docs site in docs-site/ with all navigation configured and placeholder structure. This is the foundation — all subsequent content tasks build on this.

## Design

## Goal
Scaffold a working Starlight (Astro) docs site in docs-site/ with sidebar navigation matching the epic's 22-page structure. Site builds and serves locally. Only the index landing page has real prose content — all other pages have frontmatter-only so navigation renders correctly. This is structural scaffolding, not stub content.

## Effort Estimate
2-4 hours

## Implementation

### Step 1: Initialize Starlight project
```bash
cd /Volumes/code/markymark_worktrees/next
bunx create-astro docs-site --template starlight --no-install
cd docs-site
bun install
```
Pin Astro/Starlight to latest stable. Use bun exclusively (NEVER npm/yarn/pnpm per project conventions).

### Step 2: Configure astro.config.mjs
- Site title: "markymark"
- Tagline: "High-performance Markdown LSP and MCP server"
- Social links: GitHub repo
- Sidebar groups matching this exact structure:

```
About (about.md)
Getting Started
  ├── Installation (getting-started/installation.md)
  └── Quick Start (getting-started/quick-start.md)
Usage
  ├── Workspace Management (usage/workspace-management.md)
  ├── Navigation (usage/navigation.md)
  ├── Diagnostics (usage/diagnostics.md)
  ├── Refactoring (usage/refactoring.md)
  └── Search (usage/search.md)
Guides
  └── Using with AI Agents (guides/agents.md)
Editors
  ├── VS Code (editors/vscode.md)
  ├── Neovim (editors/neovim.md)
  └── Claude Code (editors/claude-code.md)
Features
  ├── LSP Capabilities (features/lsp.md)
  ├── MCP Tools Reference (features/mcp-tools.md)
  └── Supported Formats (features/supported-formats.md)
Architecture
  ├── Overview (architecture/overview.md)
  ├── Parser Pipeline (architecture/parser-pipeline.md)
  └── Indexing (architecture/indexing.md)
Contributing
  ├── Development Setup (contributing/development.md)
  ├── Project Structure (contributing/project-structure.md)
  └── Guidelines (contributing/guidelines.md)
Troubleshooting (troubleshooting.md)
FAQ (faq.md)
```

### Step 3: Create all 22 content files
Each file gets frontmatter only (title + description). Example:
```md
---
title: Installation
description: How to install markymark on all platforms
---
```
No body text, no "Coming soon", no TODOs. Starlight renders the title from frontmatter, which is sufficient for navigation verification.

Full file list (all under src/content/docs/):
1. about.md
2. getting-started/installation.md
3. getting-started/quick-start.md
4. usage/workspace-management.md
5. usage/navigation.md
6. usage/diagnostics.md
7. usage/refactoring.md
8. usage/search.md
9. guides/agents.md
10. editors/vscode.md
11. editors/neovim.md
12. editors/claude-code.md
13. features/lsp.md
14. features/mcp-tools.md
15. features/supported-formats.md
16. architecture/overview.md
17. architecture/parser-pipeline.md
18. architecture/indexing.md
19. contributing/development.md
20. contributing/project-structure.md
21. contributing/guidelines.md
22. troubleshooting.md
23. faq.md

(23 pages total including about.md — the 22-page count in the epic excluded about since it was added later.)

### Step 4: Write index.mdx landing page
Real content covering:
- What markymark is (1 paragraph, layperson-friendly)
- Key feature highlights (4-6 bullets)
- Quick install snippet (cargo install one-liner)
- Links to Getting Started, Editor Setup, and MCP Tools Reference
- Use Starlight hero component if available

### Step 5: Update root .gitignore
Add these patterns:
```
# Docs site
docs-site/node_modules/
docs-site/dist/
docs-site/.astro/
```

### Step 6: Verify
```bash
cd docs-site && bun run build    # Must exit 0
bun run dev                       # Serves at localhost, sidebar shows all sections
# Click through every sidebar link — no 404s
```

## Success Criteria
- [ ] docs-site/ exists with astro.config.mjs configured
- [ ] bun run build in docs-site/ exits 0 with no errors
- [ ] bun run dev serves locally and sidebar shows all 8 section groups
- [ ] All 23 content files exist with correct title/description frontmatter
- [ ] index.mdx has real landing page prose (not placeholder)
- [ ] Root .gitignore updated with docs-site/node_modules, dist, .astro patterns
- [ ] No broken sidebar links (all pages render)
- [ ] bun.lockb committed (not package-lock.json)

## Anti-Patterns (FORBIDDEN)
- NO npm/yarn/pnpm — use bun exclusively
- NO "Coming soon" or "TODO" text in any file body
- NO committed node_modules directory
- NO package-lock.json (bun uses bun.lockb)
- NO customizing Starlight theme/CSS (defaults are fine, defer to later)
- NO real content in the 22 structural pages (only frontmatter — content comes in subsequent tasks)

## Key Considerations (SRE Review)

**Bun + Astro Compatibility:**
Astro officially supports bun. If bunx create-astro fails, fall back to: npx create-astro (for scaffold only), then delete package-lock.json and run bun install.

**Existing docs/ Directory:**
docs-site/ is a NEW directory at repo root. It does NOT conflict with the existing docs/ directory which contains agent reference material. These are separate concerns.

**Starlight Version Pinning:**
Use whatever version bunx create-astro installs. Don't manually pin — let the template provide compatible versions. Record the installed version in the commit message.

**Frontmatter-Only Pages vs Epic Anti-Pattern:**
The epic forbids "placeholder/stub pages with TODO content." Frontmatter-only pages are structural scaffolding (title renders as page heading), NOT stubs. They contain zero misleading content. This is the standard Starlight workflow — scaffold navigation first, fill content iteratively.
