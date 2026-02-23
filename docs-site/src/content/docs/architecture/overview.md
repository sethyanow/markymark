---
title: Architecture Overview
description: The seven-crate workspace structure and how the pieces fit together
---

markymark is a Rust workspace of seven crates that produce a single binary. That
binary can run as an LSP server (for editors) or an MCP server (for AI agents),
sharing the same core indexing engine.

## Crate map

| Crate | Role |
|-------|------|
| `markymark-kernels` | Zig SIMD kernels and md4c FFI bindings |
| `markymark-core` | Shared types, traits, error handling, scanner interface |
| `markymark-parser` | Tree-sitter-based markdown and structured format parsing |
| `markymark-index` | Document indexing, cross-reference resolution, diagnostics |
| `markymark-lsp` | LSP server (tower-lsp) |
| `markymark-mcp` | MCP server (rmcp) |
| `markymark-cli` | CLI entry point and argument parsing |

## Dependency layers

```
                  markymark-cli
                  /            \
          markymark-lsp    markymark-mcp
                  \            /
               markymark-index
                      |
               markymark-parser
                      |
                markymark-core
                      |
             markymark-kernels (optional)
```

The dependency graph is strictly layered — no circular dependencies. `markymark-kernels`
is feature-gated (`zig-kernels`) so the project can build without a Zig toolchain, falling
back to pure Rust implementations.

## Dual-server architecture

Both the LSP and MCP servers share the same indexing infrastructure. The CLI binary
(`markymark-cli`) selects the server mode at startup:

- **`--lsp`** starts the LSP server on stdin/stdout for editor integration
- **`--mcp`** (with workspace roots) starts the MCP server on stdin/stdout for AI agent access

Both modes use `CoreEngine` — a trait defined in `markymark-core` — as the interface
between protocol handlers and the indexing layer. `CoreOperation` enumerates every
supported operation, and `CoreOperationResult` wraps the response. Adding a new
capability requires changes in one place, and both servers can expose it.

## The Zig layer

`markymark-kernels` contains Zig source code compiled via `build.rs` into a static
library linked at build time. The Zig code provides:

- **md4c parser bindings** — a fast markdown parser (ported from Bun's Zig md4c
  implementation) used as the primary parsing path for LSP operations
- **Document Engine** — a stateful per-document Zig engine that parses markdown,
  extracts symbols, and serializes results into a binary blob consumed by Rust via
  `DocumentIndex::from_blob()`
- **SIMD-accelerated operations** — vectorized search and text processing kernels
- **FFI bindings** — Rust calls into Zig via C ABI exports; the Rust side uses
  `repr(C)` mirror structs at the boundary

## Reading the codebase

Good starting points for new contributors:

- **`markymark-cli/src/main.rs`** — how the binary selects LSP vs MCP mode
- **`markymark-core/src/engine.rs`** — `CoreEngine` trait and `CoreOperation` enum
- **`markymark-core/src/scanner.rs`** — `ScanBackend` trait with `Md4cScanBackend`
  and `ZigScanBackend` implementations
- **`markymark-index/src/document/mod.rs`** — `DocumentIndex` per-document storage
- **`markymark-index/src/realm/mod.rs`** — `RealmIndex` cross-document lookups
- **`markymark-parser/src/lib.rs`** — tree-sitter parser entry point
