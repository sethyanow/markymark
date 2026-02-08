# markymark

High-Performance Markdown LSP in Rust.

## Overview

markymark is a Rust-based Language Server Protocol implementation for Markdown,
designed to replace [Marksman](https://github.com/artempyanykh/marksman) with:

- Multi-tenant realm isolation (shared vs isolated workspaces)
- Full Obsidian and Logseq flavor support
- Arena allocation for O(1) cleanup
- Anchor link rename support
- Dual-transport architecture (LSP + MCP)

## Crates

- `markymark-core` - Core types and abstractions
- `markymark-parser` - Tree-sitter based markdown parser
- `markymark-index` - Document indexing and symbol resolution
- `markymark-lsp` - LSP server (tower-lsp-server)
- `markymark-mcp` - MCP server (rmcp)
- `markymark-cli` - CLI entry point

## Development

### Requirements

- Rust 1.85+ (Edition 2024)
- cargo-mcp MCP server (for development)

### Building

```bash
# Set working directory for cargo-mcp
mcp__cargo-mcp__set_working_directory({ path: "./markymark" })

# Check compilation
mcp__cargo-mcp__cargo_check()

# Run tests
mcp__cargo-mcp__cargo_test()

# Build release binary
mcp__cargo-mcp__cargo_build()
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p markymark-core

# Run with insta review
cargo test --review
```

## License

MIT OR Apache-2.0
