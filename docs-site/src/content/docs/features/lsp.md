---
title: LSP Capabilities
description: Complete reference for all Language Server Protocol features in markymark
---

markymark implements the Language Server Protocol to provide real-time editing
features in any LSP-compatible editor. These capabilities work as you type, with
changes reflected after a 75ms debounce. AI agents access the same features
through their LSP client — see [Using with AI Agents](/guides/agents/) for
dual-protocol workflow examples.

## Go to definition

Jump to the target of a wiki link, markdown anchor, XML tag declaration, or
structured document key. Works across files in the workspace.

**Supported symbols:** wiki links, markdown heading anchors, XML tags, structured keys

## Find references

Find every location that references a symbol. Useful for understanding how widely
a heading or tag is linked before renaming or removing it.

**Supported symbols:** headings, wiki links, XML tags, structured keys

## Hover

Shows contextual information when you hover over a symbol:

- **Headings** — backlink count and list of linking documents
- **Wiki links** — resolved file path of the target
- **XML tags** — workspace-wide usage statistics
- **Code spans** — inline code reference details
- **Structured keys** — value kind and key path

## Document symbols

Provides a hierarchical outline of the current file. For markdown, this includes
headings and XML tags in tree form. For structured documents (JSON, YAML, TOML,
etc.), this shows the key hierarchy.

**Editor shortcut:** Ctrl+Shift+O (Cmd+Shift+O on macOS) opens the symbol outline
in most editors.

## Workspace symbols

Search headings, `#tags`, XML tags, and `` `code spans` `` across every file in
the workspace. Results are fuzzy-matched, so partial names and abbreviations work.

**Editor shortcut:** Ctrl+T (Cmd+T on macOS) opens workspace symbol search.

## Rename

Rename a heading, XML tag, or structured key and update all references across the
workspace in a single operation. The server validates the rename before applying it
(via `prepareRename`), so you'll get an error if the symbol at the cursor isn't
renameable rather than a silent failure.

**Supported symbols:** headings, XML tags, structured keys

## Completion

Context-aware completions triggered by specific characters:

| Trigger | Completes |
|---------|-----------|
| `[` | Page names (wiki links) |
| `#` | Heading anchors and tags |
| `(` | Heading anchors within a link |
| `<` | XML tag names |

Completions draw from the full workspace index, so they stay current as you add
and rename content.

## Diagnostics

Diagnostics are published automatically when you open, edit, or close a file. You
do not need to request them — they appear as warnings and errors in your editor's
problems panel.

**What's checked:**

- Broken wiki links and markdown links (target not found)
- Duplicate heading text within the same file
- Unclosed XML tags
