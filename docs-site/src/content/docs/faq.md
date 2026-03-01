---
title: FAQ
description: Frequently asked questions about markymark
---

## What formats does markymark support?

Markdown (`.md`, `.markdown`) with full feature support: diagnostics,
go-to-definition, find-references, rename, hover, completion, and symbol
search. Wiki-link syntax (`[[page]]`, `[[page#heading]]`) is supported
natively.

Structured formats — JSON, JSONC, JSON5, JSONL, YAML, TOML, `.env`, and
INI — get document outlines, hover, and workspace symbol search. See the
[supported formats reference](/features/supported-formats/) for details.

## Does markymark work with Obsidian or Logseq?

Yes. markymark supports wiki links (`[[page]]`, `[[page#heading]]`,
`[[page|alias]]`), callouts, block IDs (`^block-id`), and Logseq page
properties (`key:: value`). Your vault or graph folder works as a markymark
workspace — just point the server at the root directory.

markymark does not replace Obsidian or Logseq. It adds IDE-level navigation
(go-to-definition, find-references, rename) on top of the files those tools
create.

## What's the difference between LSP and MCP mode?

**LSP mode** (`markymark --lsp`) integrates with text editors. Your editor
starts the server, sends file changes, and receives diagnostics, completions,
and navigation results. This is what VS Code and Neovim use.

**MCP mode** (`markymark --mcp /path`) exposes tools for AI agents and scripts.
Agents call tools like `search-symbols`, `get-outline`, and `find-references`
to read and navigate documents programmatically. Claude Code uses this mode.

Both modes access the same indexing engine — the difference is the interface
(editor protocol vs tool protocol).

## How does markymark differ from marksman and markdown-oxide?

All three are Markdown language servers. Key differences:

- **Multi-format support.** markymark indexes JSON, YAML, TOML, and other
  structured formats alongside Markdown. The others are Markdown-only.
- **MCP server mode.** markymark can serve as an AI agent tool server,
  not just an editor backend.
- **Dual parser architecture.** markymark uses a Zig-based md4c parser for
  real-time LSP operations and tree-sitter for batch indexing, optimized for
  different workloads.

## Is markymark open source? What license?

Yes. markymark is licensed under
[AGPL-3.0](https://github.com/sethyanow/markymark/blob/main/LICENSE.txt).
Source code is available at
[github.com/sethyanow/markymark](https://github.com/sethyanow/markymark).

## How do I contribute?

See the [contributing guide](/contributing/development/) for build setup,
testing, and code style. The project uses Rust with a Zig FFI layer for parsing and extraction.
You'll need Rust 1.80+, Zig 0.15.2+, and cargo-nextest.

Bug reports and feature requests go to
[GitHub Issues](https://github.com/sethyanow/markymark/issues).
