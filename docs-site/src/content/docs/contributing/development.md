---
title: Development Setup
description: How to build, test, and run markymark locally for contribution
---

## Prerequisites

| Tool | Minimum version | Purpose |
|------|----------------|---------|
| Rust | 1.80 | Compiler and cargo |
| Zig | 0.15.2 | Zig FFI layer (md4c parser and SIMD acceleration) |
| Bun | latest | Docs site tooling |
| cargo-nextest | latest | Test runner |
| lefthook | latest | Pre-commit hooks |

Install cargo-nextest and lefthook if you don't have them:

```bash
cargo install cargo-nextest --locked
brew install lefthook   # macOS; see lefthook.run for others
lefthook install
```

## Clone and build

```bash
git clone https://github.com/sethyanow/markymark.git
cd markymark
cargo build
```

For a release-optimized build (slower compile, faster binary):

```bash
cargo build --release
```

The Zig FFI layer compiles automatically via `build.rs`. Zig 0.15.2+ is required for all
builds — the library is statically linked into the binary. The build will fail if Zig is
not installed or is below the minimum version.

## Run tests

```bash
cargo nextest run                      # all tests
cargo nextest run -p markymark-core    # single crate
cargo nextest run -p markymark-index   # integration tests live here
```

The project uses cargo-nextest instead of `cargo test` for parallel execution and
better output. Integration tests live in each crate's `tests/` directory alongside unit tests
in source files (`#[cfg(test)]` modules).

Zig tests run separately:

```bash
cd zig && zig build test
```

## Lint

```bash
cargo fmt --all -- --check    # formatting
cargo clippy --workspace --all-targets -- -D warnings   # lints
cargo audit                   # dependency vulnerabilities
```

All three must pass before committing. The pre-commit hooks run these automatically.

## Run locally

Start the LSP server (editors connect over stdin/stdout):

```bash
cargo run -- --lsp
```

Start the MCP server (AI agents connect over stdin/stdout):

```bash
cargo run -- --mcp /path/to/your/workspace
```

## Pre-commit hooks

The project uses [lefthook](https://github.com/evilmartians/lefthook) to run
checks before each commit. After cloning, install the hooks:

```bash
lefthook install
```

The hook sequence runs in order:

1. `cargo fmt` — formatting check
2. `cargo clippy` — lint with `-D warnings`
3. `cargo-audit` — dependency vulnerability scan
4. `gitleaks` — secret detection on staged files
5. `zig build` — Zig FFI layer compilation check

If a hook fails, the commit is blocked. Fix the issue and try again.

## Build the docs site

```bash
cd docs-site
bun install
bun run dev     # local preview at localhost:4321
bun run build   # production build
```
