# markymark

A language server and AI agent tool for Markdown and structured data files.
Rust workspace with a Zig SIMD parser core. Provides navigation, refactoring,
search, and diagnostics across your workspace — as an editor extension (LSP)
and AI agent tool (MCP).

> **Pre-release**: APIs and behavior may change before v1.0.

## Features

**Navigation**
- **Go to definition** — jump to any heading, wiki link, or block ID across files
- **Find all references** — see every document linking to a heading or symbol
- **Rename everywhere** — rename headings and update all references in one step

**Search**
- **Symbol search** — fuzzy-match headings, tags, and code references (Ctrl+T / Cmd+T)
- **Semantic search** — rank sections by relevance using embeddings (Voyage API or local ONNX)
- **Full-text search** — free text with frontmatter, tag, and property filters
- **Regex patterns** — search file content with context lines and glob filters

**Diagnostics**
- **Broken link detection** — wiki links, markdown links, and heading anchors validated as you type
- **Link graph analysis** — orphan documents, hub detection, and connected cluster mapping

**Formats**
- Markdown, JSON, JSONC, JSON5, JSONL, YAML, TOML, .env, INI
- Obsidian + Logseq — wiki links, callouts, block IDs, and page properties

## Under the Hood

markymark is a seven-crate Rust workspace with a Zig FFI layer:

- **Zig parser core** — dependency-free md4c implementation with SIMD-accelerated extraction, statically linked into the final binary
- **Cross-document index** — string-interned lookup tables with O(1) wiki link resolution, incremental contribution-diffing, and lazy tag maintenance
- **Dual protocol** — LSP for real-time editing and MCP for AI agent workflows, backed by the same index
- **Embedding search** — optional semantic index using Voyage API or local ONNX models via fastembed

## Install

```bash
cargo install markymark-cli
```

Pre-built binaries: [GitHub Releases](https://github.com/sethyanow/markymark/releases)
- Claude Code: [Plugin README](markymark-plugin/README.md)

## Documentation

Full documentation at **[sethyanow.github.io/markymark](https://sethyanow.github.io/markymark)**:

- [Getting Started](https://sethyanow.github.io/markymark/getting-started/installation/)
- [Editor Setup](https://sethyanow.github.io/markymark/editors/vscode/)
- [MCP Tools Reference](https://sethyanow.github.io/markymark/features/mcp-tools/)
- [Agent Tutorial](https://sethyanow.github.io/markymark/guides/agents/)

## Claude Code

Install the plugin for LSP + MCP document intelligence:

```bash
claude /plugin install markymark
```

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

[AGPL-3.0](LICENSE.txt)
