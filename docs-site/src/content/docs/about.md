---
title: About markymark
description: What is markymark and why does it exist?
---

<!-- Keep format list in sync with README.md -->

## The problem

If you work with Markdown files — documentation, notes, knowledge bases — you've probably
experienced the pain of broken links, lost references, and no way to search across files.
Renaming a heading means hunting through every document that links to it. Finding all
references to a topic means grepping and hoping you catch everything.

## What markymark does

markymark gives your text files the same navigation and refactoring tools that
programmers expect for source code. Open a workspace and you get:

- **Go to definition** — jump to any heading, wiki link, or block ID across your files
- **Find all references** — see every document that links to a heading or symbol
- **Rename everywhere** — rename a heading and update all references in one step
- **Broken link detection** — see dead links and duplicate anchors as you type, before they
  become a problem
- **Workspace search** — fuzzy symbol search, full-text search, and regex with context lines
- **Multi-format support** — beyond Markdown, markymark understands JSON, JSONC, JSON5,
  JSONL, YAML, TOML, `.env`, and INI files
- **Obsidian and Logseq support** — wiki links (`[[page]]`), callouts, block IDs, and
  page properties work out of the box

## How it works

markymark runs in two modes:

**Inside your editor.** markymark integrates with editors like VS Code and Neovim through
a protocol called LSP (Language Server Protocol). You install an extension, point it at a
folder, and your editor gains all the navigation and diagnostic features listed above —
with results appearing as you type.

**As an AI agent tool.** AI coding assistants like Claude Code and Cursor can connect to
markymark as a tool server (using a protocol called MCP). This lets agents read, search,
and navigate your documents programmatically — the same capabilities a human gets in their
editor, but accessible to automated workflows.

Under the hood, markymark is written in Rust with a dependency-free Zig parser core
(based on Bun's Zig port of md4c), designed for fast response times even on large
workspaces.

## Who it's for

**Developers maintaining docs-as-code.** If your documentation lives alongside your code
in Markdown files, markymark catches broken links, helps you refactor headings safely, and
gives you workspace-wide search without leaving your editor.

**Knowledge workers with large note collections.** If you use Obsidian, Logseq, or any
tool that stores notes as Markdown files, markymark adds IDE-level navigation: jump between
notes, find everything linking to a page, and spot broken references across hundreds of
files.

**AI agents that need document access.** If you're building workflows where AI agents read
and reason about documentation, markymark provides structured access to document content,
symbols, and cross-references — without the agent needing to parse raw files itself.

## Current status

markymark is in active pre-release development. The core feature set is stable and used
daily, but APIs and behavior may change before v1.0. The project is licensed under
[AGPL-3.0](https://github.com/sethyanow/markymark/blob/main/LICENSE.txt).

Source code and releases:
[github.com/sethyanow/markymark](https://github.com/sethyanow/markymark)
