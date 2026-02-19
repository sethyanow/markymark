# markymark

High-performance Markdown & structured document LSP + MCP server in Rust and Zig.

> **Pre-release Software**: markymark is in active development. APIs, configuration, and behavior may change between minor versions.

## Overview

markymark is a Language Server Protocol (LSP) and Model Context Protocol (MCP) server for Markdown and structured data files, built in Rust with Zig SIMD kernels for performance-critical operations:

- **Multi-format support** — Markdown, JSON, JSONC, JSON5, JSONL, YAML, TOML, .env, INI/CFG
- **Zig SIMD kernels** — Vectorized heading/link/tag/block scanning, similarity search, entity hashing, and token estimation via `@Vector` intrinsics with scalar reference fallbacks
- **Multi-tenant realm isolation** (shared vs isolated workspaces)
- **Full Obsidian and Logseq flavor support** (wiki links, callouts, block IDs, page properties)
- **Anchor link rename support** (updates heading references across workspace)
- **Cross-format references** — wiki links resolve to structured document key paths
- **Incremental indexing** — Byte-range selective merge for fast re-indexing on edit
- **Dual-transport architecture** (LSP + MCP over stdio)

## Installation

### From Source

```bash
cargo install --git https://github.com/sethyanow/markymark markymark-cli
```

### Cargo Install

```bash
cargo install markymark-cli
```

### GitHub Releases

Pre-built binaries for macOS (ARM64/x86_64), Linux (x86_64/ARM64), and Windows (x86_64) are available from [Releases](https://github.com/sethyanow/markymark/releases).

### VSCode Extension

Install `markymark` from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=sethyanow.markymark) or search "markymark" in the Extensions panel. The extension discovers the `markymark` binary on `$PATH` and starts the LSP client automatically.

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

## MCP Tools

When running as an MCP server, markymark exposes these tools:

| Tool | Description |
|------|-------------|
| `create-realm` | Create an isolated workspace realm |
| `add-root` | Add a directory root to a realm for indexing |
| `search-symbols` | Fuzzy symbol search across indexed documents |
| `get-outline` | Heading/key outline for a single document |
| `export-index` | Full index export (headings, tags, links) for a document |
| `find-references` | All references to a heading or XML tag |
| `rename` | Rename a heading or tag, updating all references |
| `search-workspace` | Full-text search with frontmatter/property/tag filtering |
| `search-for-pattern` | Regex search with glob filtering and context lines |
| `graph-analysis` | Link graph intelligence (orphans, hubs, broken links, clusters, DOT export) |

## Claude Code Integration

When using markymark with Claude Code, AI agents can save significant tokens by querying the LSP for structure and diagnostics **before** reading files.

### LSP-First Workflow

For a 260-line file, an LSP `documentSymbol` query uses ~100 tokens vs ~2000+ for a full `Read` — roughly **95% savings**. This works for Markdown, JSON, YAML, TOML, and all supported formats:

1. **Get structure first** — `LSP documentSymbol` returns the heading/key hierarchy
2. **Check diagnostics** — Broken links, duplicate slugs, and unclosed tags are reported automatically
3. **Hover for details** — `LSP hover` on headings shows backlinks; on structured keys shows path/type info
4. **Read only when needed** — Use `Read` only if you need full content

### Example LSP Queries

**Document outline** (works for Markdown headings AND JSON/YAML/TOML keys):
```bash
LSP documentSymbol file.md
LSP documentSymbol config.json
```

**Hover on a heading or structured key**:
```bash
LSP hover file.md <line> <col>
LSP hover config.yaml <line> <col>
```

**Find all references** (cross-format: wiki links resolve to structured keys):
```bash
LSP findReferences file.md <line> <col>
```

**Jump to definition** of a wiki link target:
```bash
LSP goToDefinition file.md <line> <col>
```

**Search symbols across workspace**:
```bash
LSP workspaceSymbol "query"
```

### CLAUDE.md Rule (Copy-Paste)

Add this to your project's `CLAUDE.md` to encourage LSP-first reading:

```markdown
## Document Intelligence

This project uses markymark LSP. ALWAYS prefer LSP over reading raw files:
- `LSP documentSymbol <file>` for structure/outline before Read
- `LSP hover <file> <line> <col>` for heading backlinks and key path info
- Diagnostics (broken links, duplicate headings) are reported automatically
- Works for Markdown, JSON, YAML, TOML, .env, INI, and more
- Only use the Read tool when you need full prose content
```

## Crates

| Crate | Description |
|-------|-------------|
| `markymark-core` | Core types and abstractions |
| `markymark-parser` | Tree-sitter based parser (Markdown, JSON, YAML, TOML, JSON5, JSONL, flat) |
| `markymark-index` | Document indexing and symbol resolution |
| `markymark-kernels` | Zig SIMD kernel FFI wrappers (scan, embed, similarity, hash, tokens) |
| `markymark-lsp` | LSP server (tower-lsp-server) |
| `markymark-mcp` | MCP server (rmcp) |
| `markymark-cli` | CLI entry point |

## Development

### Requirements

- Rust stable (Edition 2021)
- Zig 0.15.2+ (for SIMD kernels in `markymark-kernels`)

### Building

```bash
cargo build --release
# Binary at target/release/markymark
```

The `markymark-kernels` crate's `build.rs` invokes `zig build lib` automatically — Zig just needs to be on `$PATH`.

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

### Pre-Commit Security Hooks

Install lefthook and local security tools:

```bash
# macOS (Homebrew)
brew install lefthook gitleaks

# Linux (install lefthook binary from upstream release if needed)
# https://github.com/evilmartians/lefthook#install

# Rust tooling
cargo install --locked cargo-audit
```

Install git hooks for this repository:

```bash
lefthook install
```

Run the full pre-commit pipeline manually:

```bash
lefthook run pre-commit
```

## Supported Formats

### Markdown Flavors

- **CommonMark**: Standard markdown with heading anchors
- **Obsidian**: Wiki links `[[page]]`, callouts, block IDs `^id`, embeds `![[file]]`
- **Logseq**: Nested lists, block UUIDs, page properties

### Structured Data Formats

All structured formats get byte-accurate source positions, LSP document symbols, hover info, and cross-format reference resolution.

| Format | Extensions | Parser |
|--------|-----------|--------|
| JSON | `.json` | tree-sitter-json |
| JSONC | `.jsonc` | tree-sitter-json (comment-tolerant) |
| JSON5 | `.json5` | json5 crate + custom scanner |
| JSONL | `.jsonl` | per-line JSON via tree-sitter |
| YAML | `.yaml`, `.yml` | tree-sitter-yaml |
| TOML | `.toml` | tree-sitter-toml-ng |
| .env | `.env` | line-based key=value |
| INI/CFG | `.ini`, `.cfg` | section + key=value/key:value |
