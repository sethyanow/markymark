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
├── markymark-kernels/     # Zig FFI layer (md4c parser + SIMD kernels)
├── zig/                   # Zig source (compiled by markymark-kernels)
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
| **core** | `src/engine.rs`, `src/scanner/` | `CoreEngine` trait, `CoreOperation` enum, `ScanBackend` trait |
| **parser** | `src/lib.rs`, `src/extract/` | Tree-sitter parsing, element extraction (frontmatter, links, tags, tasks, blocks) |
| **index** | `src/document/`, `src/realm/` | Per-document index (`from_blob/`, `from_scan`), cross-document realm index, resolution |
| **lsp** | `src/server.rs`, `src/state/` | LSP protocol handlers, server state management |
| **mcp** | `src/lib.rs`, `src/tools/` | MCP tool definitions, engine operation handlers |
| **kernels** | `build.rs`, `src/lib.rs` | Zig FFI layer — compilation, bindings, `repr(C)` structs |

## Key module directories

Several crates use module directories rather than single files. These were split to keep
files under the 500-line threshold.

| Module | Files | Contents |
|--------|-------|----------|
| `markymark-core/src/scanner/` | `mod.rs`, `types.rs`, `md4c.rs`, `tests.rs` | Scan-pass types and md4c scan backend |
| `markymark-parser/src/extract/` | `mod.rs`, `frontmatter.rs`, `links.rs`, `tags.rs`, `tasks.rs`, `blocks.rs` | Tree-sitter element extraction by type |
| `markymark-index/src/document/from_blob/` | `mod.rs`, `header.rs`, `decode.rs`, `owned.rs`, `tests/` | Blob deserialization with v1/v2 backward compat |
| `markymark-index/src/realm/` | `mod.rs`, `types.rs`, `helpers.rs`, `tests.rs` | RealmIndex v2 — interner, incremental updates, resolution |

## Zig sources

The `zig/` directory contains the Zig source code compiled by `markymark-kernels`:

| Directory | Purpose |
|-----------|---------|
| `zig/src/engine/` | Document engine — blob serialization, FFI exports, stored types |
| `zig/src/md4c/` | md4c parser bindings — extraction renderer, offset recovery, XML tags |
| `zig/src/shared/` | Shared utilities (similarity, helpers) |
| `zig/test/` | Zig test files |
| `zig/bench/` | Zig benchmarks |

The `markymark-kernels/build.rs` script invokes `zig build` during `cargo build`
and links the resulting static library. Zig 0.15.2+ is required.

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
