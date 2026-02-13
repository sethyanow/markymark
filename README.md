# markymark

High-performance Markdown LSP + MCP server in Rust.

> **Alpha Software**: markymark is in active development and not yet ready for production use. APIs, configuration, and behavior may change without notice.

## Overview

markymark is a Rust-based Language Server Protocol (LSP) and Model Context Protocol (MCP) server for Markdown, featuring:

- **Multi-tenant realm isolation** (shared vs isolated workspaces)
- **Full Obsidian and Logseq flavor support** (wiki links, callouts, block IDs, page properties)
- **Anchor link rename support** (updates heading references across workspace)
- **Dual-transport architecture** (LSP + MCP over stdio)

## Installation

### From Source

```bash
cargo install --git https://github.com/sethyanow/markymark markymark-cli
```

### Cargo Install (after crates.io publish)

```bash
cargo install markymark-cli
```

### GitHub Releases (after first release)

Pre-built binaries for macOS (ARM64/x86_64), Linux (x86_64/ARM64), and Windows (x86_64) will be available from [Releases](https://github.com/sethyanow/markymark/releases).

### Claude Code Plugin

See [markymark-plugin/README.md](markymark-plugin/README.md) for plugin installation.

## Usage

**As LSP server** (stdio):
```bash
markymark --lsp
```

**As MCP server** (stdio):
```bash
markymark --mcp /path/to/workspace
```

## Claude Code Integration

When using markymark with Claude Code, AI agents can save significant tokens by querying the LSP for structure and diagnostics **before** reading full markdown files.

### LSP-First Workflow

For a 260-line markdown file, an LSP `documentSymbol` query uses ~100 tokens vs ~2000+ for a full `Read` — roughly **95% savings**. Use this workflow:

1. **Get structure first** — `LSP documentSymbol` returns the heading/XML tag hierarchy
2. **Check diagnostics** — Broken links, duplicate slugs, and unclosed tags are reported automatically
3. **Hover for details** — `LSP hover` on headings shows backlink counts; on XML tags shows workspace usage stats
4. **Read only when needed** — Use `Read` only if you need the full prose content

### Example LSP Queries

**Document outline** (heading + XML tag hierarchy):
```
LSP documentSymbol file.md
```

**Hover on a heading** (backlinks, level info):
```
LSP hover file.md <line> <col>
```

**Find all references** to a heading or wiki link:
```
LSP findReferences file.md <line> <col>
```

**Jump to definition** of a wiki link target:
```
LSP goToDefinition file.md <line> <col>
```

**Search symbols across workspace**:
```
LSP workspaceSymbol "query"
```

### CLAUDE.md Rule (Copy-Paste)

> Note: Claude loves to hype itself up - this is not ideal until future features are implemented. YMMV.

Add this to your project's `CLAUDE.md` to encourage LSP-first markdown reading:

```markdown
## Markdown Intelligence

This project uses markymark for markdown LSP. ALWAYS prefer LSP over reading raw files:
- `LSP documentSymbol <file>` for structure/outline before Read
- `LSP hover <file> <line> <col>` for heading backlinks and XML tag stats
- Diagnostics (broken links, duplicate headings) are reported automatically
- Only use the Read tool when you need full prose content
```

## Crates

| Crate | Description |
|-------|-------------|
| `markymark-core` | Core types and abstractions |
| `markymark-parser` | Tree-sitter based markdown parser |
| `markymark-index` | Document indexing and symbol resolution |
| `markymark-lsp` | LSP server (tower-lsp-server) |
| `markymark-mcp` | MCP server (rmcp) |
| `markymark-cli` | CLI entry point |

## Development

### Requirements

- Rust stable (Edition 2021)

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

