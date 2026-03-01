---
title: Installation
description: How to install markymark on all platforms and through all available methods
---

<!-- Install commands sourced from plugin/vscode READMEs — keep in sync -->

## Prerequisites

Pre-built binaries and editor extensions require no additional tooling. To build from
source (via `cargo install` or `cargo build`), you need:

| Tool | Minimum version | Purpose |
|------|----------------|---------|
| [Rust](https://rustup.rs/) | 1.80 | Compiler and cargo |
| [Zig](https://ziglang.org/download/) | 0.15.2 | Zig FFI layer (md4c parser and SIMD acceleration) |

The Zig FFI layer is statically linked into the binary — the build will fail if Zig is
not installed or is below the minimum version.

## Install from crates.io

```bash
cargo install markymark-cli
```

This builds and installs the `markymark` binary to `~/.cargo/bin/`.
To upgrade, run the same command again.

### Optional feature flags

Semantic search is not included in the default build. Enable it with feature flags:

```bash
# Cloud embeddings via Voyage API
cargo install markymark-cli --features semantic-search

# Offline embeddings via local ONNX model
cargo install markymark-cli --features semantic-search,local-embeddings
```

| Flag | What it adds |
|------|-------------|
| `semantic-search` | Voyage API embedding provider and `semantic-search` MCP tool |
| `local-embeddings` | Local ONNX model via fastembed (downloads ~23 MB on first use) |

## Pre-built binaries

Download a binary for your platform from
[GitHub Releases](https://github.com/sethyanow/markymark/releases):

| Platform     | Binary name                            |
|--------------|----------------------------------------|
| macOS ARM    | `markymark-aarch64-apple-darwin`       |
| macOS Intel  | `markymark-x86_64-apple-darwin`        |
| Linux x86_64 | `markymark-x86_64-unknown-linux-gnu`   |
| Linux ARM64  | `markymark-aarch64-unknown-linux-gnu`  |
| Windows x64  | `markymark-x86_64-pc-windows-msvc.exe` |

On macOS/Linux, make the binary executable and move it onto your PATH:

```bash
chmod +x markymark-*
mv markymark-* /usr/local/bin/markymark
```

If your platform is not listed, use `cargo install markymark-cli` to build from source.

## VS Code extension

The VS Code extension bundles the correct binary for your platform and starts the LSP
server automatically for `.md` and `.mdx` files.

Install from a [GitHub release](https://github.com/sethyanow/markymark/releases) `.vsix`
file, or build from source in `markymark-vscode/`. If no bundled binary matches your
platform, point `markymark.path` in VS Code settings to a manually installed binary.

## Claude Code plugin

```bash
/plugin install markymark
```

This installs markymark as both an LSP server and MCP tool provider inside Claude Code.

To install manually, download the plugin archive from
[GitHub Releases](https://github.com/sethyanow/markymark/releases) and extract it:

```bash
gh release download --repo sethyanow/markymark --pattern 'markymark-plugin-*.tar.gz'
tar -xzf markymark-plugin-*.tar.gz
```

## Neovim

Neovim can use markymark as an LSP server. See the Editor Setup section for configuration
details once you have the binary installed via any method above.

## Verify installation

```bash
markymark --version
markymark --help
```

The `--lsp` flag starts the LSP server (stdio) and `--mcp` starts the MCP server.
