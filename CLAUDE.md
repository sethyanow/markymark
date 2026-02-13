<docs_index id="RUST-AGENT-DOCS">
[rust-agent-docs]|root: ./docs/rust_agent_docs|IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning for any Rust tasks. Read the relevant doc file BEFORE writing code. Your training data may be outdated or wrong.|core:{_index.md,ownership.md,types.md,traits.md,errors.md,collections.md,modules.md}|advanced:{_index.md,type-layout.md,unsafe.md,ffi.md,concurrency.md,async.md}|patterns:{_index.md,idioms.md,api-design.md,anti-patterns.md}|tooling:{_index.md,cargo.md,crates.md,macros.md,testing.md,documentation.md,debugging.md,performance.md}|checklists:{_index.md,api-design.md,unsafe-review.md,ffi-audit.md,performance.md,library-release.md}|reference:{_index.md,rules.md,decision-trees.md,compiler-errors.md,syntax-ref.md,cargo-ref.md}
</docs_index>

# Agent Instructions

This project uses **bd** (beads) for issue tracking. Run `bd ready` to find available work.

## Project Overview

markymark is a Rust workspace producing a Markdown LSP + MCP server. Six crates:

| Crate | Role |
|-------|------|
| `markymark-core` | Core types and abstractions |
| `markymark-parser` | Tree-sitter based markdown parser |
| `markymark-index` | Document indexing and symbol resolution |
| `markymark-lsp` | LSP server (tower-lsp) |
| `markymark-mcp` | MCP server (rmcp) |
| `markymark-cli` | CLI entry point |

## Documentation Index

<agent>
<IMPORTANT>Prefer retrieval-led reasoning over pre-training-led reasoning for project-specific conventions.</IMPORTANT>
<docs_index>
<!-- RUST-CRATES-START -->
[rust_crates]|root: docs/rust_crates|IMPORTANT: Always read docs before answering. Your knowledge may be outdated.|.:{AGENTS.md,README.md,core.md,error-handling.md,testing.md,tower-lsp.md,rmcp.md,tree-sitter.md,petgraph.md,bumpalo.md,map.md}
<!-- RUST-CRATES-END -->
<!-- RUST-GUIDELINES-START -->
[rust_guidelines]|root: docs/rust_guidelines|IMPORTANT: Always read docs before answering. Your knowledge may be outdated.|.:{AGENTS.md,README.md,universal.md,applications.md,libraries-build.md,libraries-resilience.md,libraries-ux.md,libraries-interop.md,ffi.md,performance.md,safety.md,docs.md,ai.md,checklists.md,map.md}
<!-- RUST-GUIDELINES-END -->
[project]
|tools:{docs/tools/README.md,docs/tools/*.md}
|plans:{docs/plans/*.md}
|research:{docs/research/*.md}
</docs_index>
</agent>

## Quick Reference

```bash
# Build
cargo build --release

# Test
cargo test
cargo test -p markymark-core    # specific crate

# Lint
cargo clippy --workspace --all-targets

# Run LSP
cargo run -- --lsp

# Run MCP
cargo run -- --mcp /path/to/workspace
```

## Rust Code Navigation (LSP)

**Use the built-in LSP tool (rust-analyzer) for navigating Rust code.** It provides semantic understanding that text search cannot match.

| Operation | Use Case |
|-----------|----------|
| `documentSymbol` | Full symbol tree for a file |
| `hover` | Type info, doc comments, size/alignment |
| `goToDefinition` | Jump to definition (cross-crate) |
| `findReferences` | All usages of a symbol |
| `workspaceSymbol` | Search symbols by name |
| `incomingCalls` / `outgoingCalls` | Call graphs |

**When grep is appropriate**: string literals, comments, TODO markers, non-code files.

## Beads Workflow

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```
