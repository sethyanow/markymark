---
title: Workspace Management
description: How to set up and manage markymark workspaces across your projects
---

markymark indexes **folders** of files, not individual documents. Cross-file features
like navigation, find-references, and rename only work when markymark has a folder
(or set of folders) to index. Opening a single file gives you diagnostics and
an outline, but nothing that spans documents.

## Realms

A **realm** is markymark's name for an indexed workspace scope — a named collection of
folder roots whose files are tracked together for navigation, search, and diagnostics.

**In your editor,** realm setup is automatic. When you open a folder in VS Code, markymark
creates a realm for it and begins indexing. Each workspace folder becomes a root in
the realm. Multi-root workspaces (multiple folders open at once) are supported — all
roots share the same realm.

**With MCP tools,** you manage realms explicitly:

Create a realm, then add folder roots to it:

```text
create-realm: { "name": "my-docs" }
add-root: { "realm": "my-docs", "root": "/path/to/docs" }
```

You can add multiple roots to a single realm. Remove a root when you no longer need it indexed:

```text
remove-root: { "realm": "my-docs", "root": "/path/to/docs" }
```

Check what's indexed with `realm-stats`:

```text
realm-stats: { "realm": "my-docs" }
```

This returns counts of documents, headings, links, tags, and more. Use it as a quick
health check after adding or removing roots.

When finished, destroy the realm to free resources:

```text
destroy-realm: { "name": "my-docs" }
```

## Supported file types

markymark indexes Markdown (`.md`) as its primary format. It also understands
JSON, JSONC, JSON5, JSONL, YAML, TOML, `.env`, and INI files — providing
outlines, key-path navigation, and diagnostics for structured data alongside
your prose.

## CLI flags

Start markymark in LSP mode (for editors):

```bash
markymark --lsp
```

Start in MCP mode (for AI agents), passing a workspace path:

```bash
markymark --mcp /path/to/workspace
```

The `--mcp` flag creates a default realm with the given path as its root.
