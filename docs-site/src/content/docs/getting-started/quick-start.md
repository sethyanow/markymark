---
title: Quick Start
description: Get markymark working in your editor in 5 minutes
---

This guide covers the minimum steps to start using markymark. Pick the path
that matches your setup — editor integration or AI agent tooling.

## Before you start

markymark works on a **folder of files**, not individual documents. Open a folder
(or workspace) containing `.md` files to enable cross-file features like navigation
and reference detection.

If you don't have Markdown files handy, create two test files to follow along:

```bash
mkdir ~/markymark-test && cd ~/markymark-test
echo -e '# Welcome\nSee [[notes]] for details.' > index.md
echo -e '# Notes\nBack to [[index]].' > notes.md
```

## For editor users

If you installed the **VS Code extension**, it starts the language server automatically
when you open a `.md` file — no additional configuration needed.

Open your workspace and try these features:

**See diagnostics.** Open a file with a broken wiki link (e.g., `[[nonexistent]]`).
The link is underlined with a warning. Duplicate headings in the same file are
also flagged.

**Navigate.** Hold Ctrl (Cmd on macOS) and click a wiki link or heading reference
to jump to its definition. This works across files in your workspace.

**Search symbols.** Press Ctrl+T (Cmd+T on macOS) to open workspace symbol search.
Type a heading name to find it across all files.

**Find references.** Right-click a heading and select "Find All References" to see
every document that links to it.

**Rename.** Right-click a heading and select "Rename Symbol." All wiki links and
Markdown anchors pointing to that heading update automatically across the workspace.

**Browse outline.** Open the document outline panel (Ctrl+Shift+O / Cmd+Shift+O)
to see a hierarchical tree of headings and tags in the current file.

## For AI agents

If you installed the **Claude Code plugin**, MCP tools are available immediately —
no manual server startup needed.

To run markymark as a standalone MCP server:

```bash
markymark --mcp /path/to/your/workspace
```

Try these tools to get oriented:

**Get a document outline.** Call `get-outline` with a document URI to see its
heading structure:

```text
get-outline: { "uri": "file:///path/to/README.md" }
```

**Search across files.** Call `search-symbols` to find headings and tags by name:

```text
search-symbols: { "query": "installation" }
```

**Find cross-references.** Call `find-references` to see which documents link to
a specific heading:

```text
find-references: { "uri": "file:///path/to/doc.md", "line": 0, "character": 0 }
```

**Check for problems.** Call `get-diagnostics` to detect broken links and
duplicate headings:

```text
get-diagnostics: { "uri": "file:///path/to/doc.md" }
```

The full tool list includes 15 tools covering workspace management, search,
graph analysis, and more.

## Next steps

The editor setup guides cover detailed configuration for VS Code, Neovim, and
Claude Code. The usage section walks through advanced workflows like workspace
search, regex pattern matching, and refactoring across files.
