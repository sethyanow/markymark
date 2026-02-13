# markymark

High-performance Markdown LSP + MCP server in Rust.

## Overview

markymark is a Rust-based Language Server Protocol (LSP) and Model Context Protocol (MCP) server for Markdown, featuring:

- **Multi-tenant realm isolation** (shared vs isolated workspaces)
- **Full Obsidian and Logseq flavor support** (wiki links, callouts, block IDs, page properties)
- **Arena allocation** for O(1) cleanup
- **Anchor link rename support** (updates heading references across workspace)
- **Dual-transport architecture** (LSP + MCP over stdio)

## Installation

### Cargo Install

```bash
cargo install markymark-cli
```

### GitHub Releases

Download pre-built binaries from [Releases](https://github.com/sethyanow/markymark/releases):

- macOS ARM64 (Apple Silicon)
- macOS x86_64 (Intel)
- Linux x86_64 / ARM64
- Windows x86_64

### Claude Code Plugin

```bash
# From Claude Code (when published to marketplace)
/plugin install markymark
```

Or install manually from a release archive — see [markymark-plugin/README.md](markymark-plugin/README.md).

## Usage

**As LSP server** (stdio):
```bash
markymark --lsp
```

**As MCP server** (stdio):
```bash
markymark --mcp /path/to/workspace
```

## Crates

| Crate | Description |
|-------|-------------|
| `markymark-core` | Core types and abstractions |
| `markymark-parser` | Tree-sitter based markdown parser |
| `markymark-index` | Document indexing and symbol resolution |
| `markymark-lsp` | LSP server (tower-lsp) |
| `markymark-mcp` | MCP server (rmcp) |
| `markymark-cli` | CLI entry point |

## Development

### Requirements

- Rust 1.85+ (Edition 2024)

### Building

```bash
cargo build --release
# Binary at target/release/markymark
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p markymark-core

# Run with insta snapshot review
cargo insta test --review
```

### Linting

```bash
cargo clippy --workspace --all-targets
```

## Supported Markdown Flavors

- **CommonMark**: Standard markdown with heading anchors
- **Obsidian**: Wiki links `[[page]]`, callouts, block IDs `^id`, embeds `![[file]]`
- **Logseq**: Nested lists, block UUIDs, page properties

## License

MIT OR Apache-2.0
