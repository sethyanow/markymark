---
title: Navigation
description: Jump to definitions, follow links, and navigate across your markdown files
---

markymark lets you move between documents the same way a code editor jumps between
source files — follow a link, find where it's referenced, or search for any heading
in the workspace.

## Go to definition

Hold Ctrl (Cmd on macOS) and click a wiki link like `[[notes]]` to jump to its
target document. This also works for heading references like `[[notes#setup]]` and
block IDs. If the target is in another file, markymark opens it at the right position.

**With MCP:** Use `go-to-definition` in your editor's LSP client to resolve link
targets. For AI agents, combine `get-outline` with `search-symbols` to locate
target documents programmatically.

## Find all references

Right-click a heading and select **Find All References** to see every document that
links to it — including wiki links, markdown links, and heading anchors. Results
appear in the references panel with file names and line numbers.

**With MCP:**

```
find-references: { "uri": "file:///path/to/doc.md", "line": 0, "character": 2 }
```

Returns a list of locations across the workspace where the symbol at that position
is referenced.

## Document outline

Press Ctrl+Shift+O (Cmd+Shift+O on macOS) to open the document outline. This shows
a hierarchical tree of headings in the current file. For structured data files (JSON,
YAML, TOML), the outline shows key paths instead.

**With MCP:**

```
get-outline: { "uri": "file:///path/to/doc.md" }
```

## Workspace symbol search

Press Ctrl+T (Cmd+T on macOS) to search headings, tags, and code references across
all files in the workspace. Type a partial name and markymark fuzzy-matches against
the full index.

**With MCP:**

```
search-symbols: { "query": "installation" }
```

Returns matching symbols with their file locations and ranges.

## Hover information

Hover over a heading to see how many other documents reference it (backlink count).
Hover over a wiki link to see the target heading's text and location. This helps
you gauge the impact of a heading before renaming or removing it.

## Tips

- Go-to-definition follows wiki links across files — open a folder to enable it.
- Workspace symbol search includes headings, tags, and inline code references.
- The outline works for structured data files too, showing key paths for JSON/YAML/TOML.
