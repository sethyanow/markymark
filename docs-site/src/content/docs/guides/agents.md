---
title: Using with AI Agents
description: How to use markymark as an MCP server with Claude Code and other AI agents
---

markymark's MCP server gives AI agents structured access to document
intelligence — headings, links, diagnostics, and refactoring — without
parsing raw Markdown. See [Claude Code setup](/editors/claude-code/) for
installation and [MCP Tools Reference](/features/mcp-tools/) for the full
parameter list.

## Set up a workspace

Create a realm and add your documentation root:

```json
// create-realm
{ "name": "my-docs" }

// add-root
{ "realm": "my-docs", "root": "/path/to/docs" }
```

A realm is an isolated index. You can create multiple realms for different
directories and tear them down with `destroy-realm` when finished.

## Understand structure

Get the heading hierarchy for a single document, then search for symbols
across the workspace:

```json
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

```json
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

```json
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
