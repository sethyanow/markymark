---
title: Project Structure
description: Tour of the markymark codebase — what lives where and why
---

## Top-level layout

```
markymark/
├── markymark-cli/         # Binary entry point
├── markymark-core/        # Shared types and traits
├── markymark-parser/      # Tree-sitter markdown parser
├── markymark-index/       # Document and realm indexing
├── markymark-lsp/         # LSP server
├── markymark-mcp/         # MCP server
├── markymark-kernels/     # Zig SIMD kernels + md4c FFI
├── zig/                   # Zig source (compiled by kernels crate)
├── markymark-plugin/      # Claude Code plugin manifest
├── markymark-vscode/      # VS Code extension
├── docs-site/             # This documentation site (Starlight)
├── docs/                  # Agent reference docs (internal)
├── examples/              # Example configurations
├── security-fixtures/     # Test fixtures for security scans
└── docker/                # Container build files
```

## Crate responsibilities

Each crate has a focused role. See the [architecture overview](/architecture/overview/)
for the full dependency graph.

| Crate | Key files | What to look at |
|-------|-----------|-----------------|
| **cli** | `src/main.rs` | Argument parsing, LSP vs MCP mode selection |
| **core** | `src/engine.rs`, `src/scanner.rs` | `CoreEngine` trait, `CoreOperation` enum, `ScanBackend` trait |
| **parser** | `src/lib.rs`, `src/extract/` | Tree-sitter parsing, frontmatter extraction |
| **index** | `src/document/`, `src/realm/` | Per-document index, cross-document realm index, resolution |
| **lsp** | `src/server.rs`, `src/state/` | LSP protocol handlers, server state management |
| **mcp** | `src/lib.rs`, `src/tools/` | MCP tool definitions, engine operation handlers |
| **kernels** | `build.rs`, `src/lib.rs` | Zig compilation, FFI bindings, `repr(C)` structs |

## Zig sources

The `zig/` directory contains the Zig source code compiled by `markymark-kernels`:

| Directory | Purpose |
|-----------|---------|
| `zig/src/` | Production source — md4c parser, document engine, SIMD kernels |
| `zig/test/` | Zig test files |
| `zig/bench/` | Zig benchmarks |

The `markymark-kernels/build.rs` script invokes `zig build` during `cargo build`
and links the resulting static library.

## Where tests live

- **Unit tests** — inline in source files as `#[cfg(test)]` modules
- **Integration tests** — in each crate's `tests/` directory (e.g., `markymark-index/tests/realm_index.rs`)
- **Zig tests** — in `zig/test/` and inline `test` blocks in source files
- **Snapshot tests** — some crates use `insta` for snapshot testing

The `markymark-index` crate has the most integration tests since it exercises
the full indexing pipeline (parsing, extraction, resolution, diagnostics).

## Configuration files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Workspace configuration, shared dependencies |
| `lefthook.yml` | Pre-commit hook definitions |
| `cliff.toml` | Changelog generation (git-cliff) |
| `deny.toml` | Dependency license and advisory checks (cargo-deny) |
| `Cross.toml` | Cross-compilation settings |
