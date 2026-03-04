---
title: Claude Code
description: Using the markymark Claude Code plugin for AI-assisted markdown workflows
---

The Claude Code plugin gives you both editor-level LSP features (diagnostics,
navigation, rename) **and** MCP tools for AI agent workflows — from a single
plugin install.

## Installation

**Manual installation** (download a release):

```bash
# Download latest release
gh release download --repo sethyanow/markymark \
  --pattern 'markymark-plugin-*.tar.gz'

# Extract and install
tar -xzf markymark-plugin-*.tar.gz
claude-code --plugin-dir ./markymark-plugin
```

Pre-built binaries are included for macOS (ARM and Intel), Linux (x86_64 and
ARM64), and Windows (x86_64). The plugin auto-detects your platform.

## What the plugin provides

The plugin registers two servers simultaneously:

**LSP server** — activates on `.md` and `.mdx` files. Provides diagnostics, go-to-definition,
find-references, rename, hover, completion, and document symbols — the same features
available in VS Code and Neovim.

**MCP server** — provides 15 tools that AI agents can call programmatically. These tools
let agents read document structure, search across files, check for problems, and
perform refactoring without parsing raw Markdown themselves.

In practice, agents interleave both protocols in a single workflow: LSP for
position-aware navigation (outline, hover, go-to-definition) and MCP for
workspace-wide operations (search, diagnostics, graph analysis). See
[Using with AI Agents](/guides/agents/) for dual-protocol workflow examples
and a comparison table of when to use each protocol.

## Key MCP tools

These are the tools agents use most often:

| Tool | What it does |
|------|--------------|
| `get-outline` | Returns the heading hierarchy for a document |
| `search-symbols` | Fuzzy search for headings and tags across the workspace |
| `find-references` | Lists all documents linking to a heading |
| `get-diagnostics` | Reports broken links, duplicate headings, and other issues |
| `search-workspace` | Full-text search with metadata filters |
| `search-for-pattern` | Regex search with context lines |
| `rename` | Renames a heading and updates all references |
| `graph-analysis` | Analyzes link health: orphans, broken links, clusters |

The full tool list covers workspace management (`create-realm`, `destroy-realm`,
`realm-stats`), document inspection (`export-index`), and semantic search
(`semantic-search`).

## Plugin skill

The plugin includes a `/markdown-check` skill that validates Markdown quality
across your workspace. Run it from Claude Code:

```text
/markdown-check
```

This checks for broken links, duplicate headings, unclosed XML tags, and other
common issues — a quick quality gate before committing documentation changes.

## Tips

- The MCP server operates on a **realm**, which corresponds to a workspace folder.
  If your project has multiple documentation directories, create separate realms
  for each via `create-realm`.
- `get-outline` is the fastest way for an agent to understand document structure
  before making targeted edits.
- Agents can combine `get-diagnostics` + `rename` for automated refactoring:
  find issues, then fix them programmatically.

### Dual-protocol tips

- Use **LSP `documentSymbol`** before editing a file — it returns a semantic,
  position-aware outline that is cheaper than reading the full file.
- Use **MCP `search-symbols`** when you don't know which file to look in — it
  searches headings and tags across the entire workspace.
- Use **MCP `get-diagnostics`** for batch quality checks across a realm; use
  **LSP diagnostics** for real-time feedback as you edit.
