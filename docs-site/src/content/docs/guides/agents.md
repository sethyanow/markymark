---
title: Using with AI Agents
description: How to use markymark with AI agents through LSP and MCP protocols
---

markymark gives AI agents access to document intelligence through two
complementary protocols: **LSP** for real-time, position-aware navigation
(hover, go-to-definition, find-references, document symbols) and **MCP** for
workspace-level operations (search, diagnostics, realm management, graph
analysis). Agents that have both protocols available — such as
[Claude Code](/editors/claude-code/) — get the best results by interleaving
them in a single workflow.

See [Claude Code setup](/editors/claude-code/) for installation,
[LSP Capabilities](/features/lsp/) for the editor-and-agent features, and
[MCP Tools Reference](/features/mcp-tools/) for the full tool parameter list.

## Set up a workspace

Create a realm and add your documentation root:

```jsonc
// create-realm
{ "name": "my-docs" }

// add-root
{ "realm": "my-docs", "root": "/path/to/docs" }
```

A realm is an isolated index. You can create multiple realms for different
directories and tear them down with `destroy-realm` when finished.

## Dual-protocol workflows

Agents with access to both LSP and MCP can pick the best tool for each step.
The table below summarizes when each protocol is the better choice:

| Operation | Recommended | Why |
|-----------|-------------|-----|
| Heading outline for a file | LSP `documentSymbol` | Hierarchical tree with line-precise positions |
| Hover info (backlinks, types) | LSP `hover` | Contextual, position-aware |
| Go to definition | LSP `goToDefinition` | Cross-file navigation with position context |
| Find all references | Either | LSP is position-based; MCP `find-references` works the same way |
| Search symbols by name | MCP `search-symbols` | Workspace-wide fuzzy search, no file context needed |
| Regex content search | MCP `search-for-pattern` | Content search with context lines and glob filtering |
| Get diagnostics | MCP `get-diagnostics` | Can check an entire realm at once |
| Rename across workspace | MCP `rename` | Programmatic, no editor UI needed |
| Link graph analysis | MCP `graph-analysis` | No LSP equivalent |
| Workspace management | MCP `create-realm` / `add-root` | No LSP equivalent |

### Example: explore and fix documentation

1. **LSP `documentSymbol`** — get the heading outline of a target file.
2. **LSP `hover`** on a heading — check how many documents link to it.
3. **MCP `get-diagnostics`** — find broken links and duplicate headings across the realm.
4. **MCP `rename`** — fix each heading issue programmatically.

### Example: research and navigate

1. **MCP `search-symbols`** — find headings matching a topic across the workspace.
2. **LSP `goToDefinition`** — jump from a wiki link to the target document.
3. **LSP `hover`** — read backlink context at the destination.
4. **MCP `graph-analysis`** — audit link health for the whole realm.

:::note
Dual-protocol usage is recommended but not required. Agents with only MCP access
(headless scripts, non-editor integrations) can accomplish most tasks using MCP
tools alone — the sections below cover MCP-only workflows.
:::

## Understand structure

Get the heading hierarchy for a single document, then search for symbols
across the workspace:

```jsonc
// get-outline
{ "uri": "file:///path/to/docs/guide.md", "realm": "my-docs" }

// search-symbols
{ "query": "installation", "realm": "my-docs" }
```

`get-outline` returns the heading tree — useful before making targeted edits.
`search-symbols` fuzzy-matches headings and tags across every indexed file.

## Find and fix issues

Run diagnostics to surface broken links, duplicate headings, and unclosed
XML tags. Then fix a heading with `rename`, which updates all references
automatically:

```jsonc
// get-diagnostics
{ "realm": "my-docs" }

// rename
{
  "uri": "file:///path/to/docs/guide.md",
  "line": 5,
  "character": 3,
  "new_name": "Getting Started",
  "realm": "my-docs"
}
```

Agents can loop over diagnostic results and call `rename` for each
heading issue to batch-fix an entire workspace.

## Audit link health

Use `graph-analysis` to find orphaned documents, broken links, and
hub pages with the most inbound links:

```jsonc
// graph-analysis
{ "realm": "my-docs", "include_clusters": true }
```

This returns orphans (no resolved links in or out), hubs (most incoming
links), broken links, and optionally weakly-connected clusters — a quick
health check for large documentation sets.

## Tips

- Call `realm-stats` for a fast overview: document count, heading count,
  link count, and optional token estimation.
- Use `get-outline` before editing a document — it is cheaper than reading
  the full file and gives you the exact heading positions.
- Combine `get-diagnostics` with `rename` for automated refactoring:
  find issues, then fix them programmatically in a loop.
- Call `destroy-realm` to clean up the index when you are done.
