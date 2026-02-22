# markymark

A language server and AI agent tool for Markdown and structured data files,
built in Rust with Zig SIMD kernels. Provides navigation, refactoring, search,
and diagnostics across Markdown, JSON, YAML, TOML, .env, INI, and more.

> **Pre-release**: APIs and behavior may change before v1.0.

## Features

- **Go to definition** — jump to any heading, wiki link, or block ID across files
- **Find all references** — see every document linking to a heading or symbol
- **Rename everywhere** — rename headings and update all references in one step
- **Broken link detection** — diagnostics flag dead links and duplicate anchors as you type
- **Workspace search** — fuzzy symbol search, full-text search, and regex with context lines
- **Multi-format support** — Markdown, JSON, JSONC, JSON5, JSONL, YAML, TOML, .env, INI
- **Obsidian + Logseq support** — wiki links, callouts, block IDs, and page properties

Works as an editor extension (LSP) and AI agent tool (MCP).

## Install

```bash
cargo install markymark-cli
```

Pre-built binaries: [GitHub Releases](https://github.com/sethyanow/markymark/releases)
| VS Code: [Marketplace](https://marketplace.visualstudio.com/items?itemName=sethyanow.markymark)
| Claude Code: [Plugin README](markymark-plugin/README.md)

## Documentation

Full documentation at **[markymark.rs](https://markymark.rs)**:

- [Getting Started](https://markymark.rs/getting-started/installation/)
- [Editor Setup](https://markymark.rs/editors/vscode/)
- [MCP Tools Reference](https://markymark.rs/features/mcp-tools/)
- [Agent Tutorial](https://markymark.rs/guides/agents/)

## Claude Code

Add to your `CLAUDE.md` for LSP-first document reading:

```markdown
## Document Intelligence
This project uses markymark LSP. Prefer LSP over reading raw files:
- `LSP documentSymbol <file>` — structure/outline before Read
- `LSP hover <file> <line> <col>` — heading backlinks, key path info
- Diagnostics (broken links, duplicate headings) reported automatically
- Works for Markdown, JSON, YAML, TOML, .env, INI, and more
```

## License

[MIT OR Apache-2.0](LICENSE.txt)
