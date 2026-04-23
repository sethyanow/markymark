# Releasing markymark

This document describes the complete release workflow for markymark.

## Pre-Release Checklist

Before starting a release, verify all quality gates pass:

```bash
# Format check
cargo fmt --all -- --check

# Lint (warnings are errors in CI)
cargo clippy --workspace --all-targets -- -D warnings

# All tests
cargo test --workspace

# Smoke tests (LSP + MCP protocol handshake)
cargo test -p markymark-cli --test smoke_lsp --test smoke_mcp

# E2E protocol tests
cargo test -p markymark-cli --test lsp_methods --test mcp_methods -- --nocapture

# Plugin tests (bash)
bash markymark-plugin/tests/test_hooks.sh
```

## Version Bumping

The workspace version is replicated across **five** source files. Each
ecosystem (Bazel, Cargo, plugin manifest, VSCode extension) needs its own
literal — none can load `version.bzl`, and `MODULE.bazel` is evaluated in
a restricted context that can't read `Cargo.toml`.

| # | File | Lines to edit | Purpose |
|---|------|---------------|---------|
| 1 | `version.bzl` (root) | 1 | `VERSION` constant loaded by `BUILD.bazel` files; drives `CARGO_PKG_VERSION` in Bazel-built binaries so `markymark --version` reports correctly |
| 2 | `Cargo.toml` (root) | 7 | `[workspace.package].version` + six `[workspace.dependencies]` entries for internal crates (`markymark-core`, `-kernels`, `-parser`, `-index`, `-lsp`, `-mcp`). All six must match the package version for Cargo/crates.io publish. |
| 3 | `MODULE.bazel` | 1 | Bazel module declaration |
| 4 | `markymark-plugin/.claude-plugin/plugin.json` | 1 | Claude Code plugin manifest (not auto-derived) |
| 5 | `markymark-vscode/package.json` | 1 | VSCode extension manifest |

Member crates use `version.workspace = true` and internal deps use
`{ workspace = true }`, so no per-crate `Cargo.toml` edits are needed.

To bump:

1. Edit `VERSION` in `version.bzl`
2. Edit `Cargo.toml`: update `[workspace.package].version` AND all 6
   `[workspace.dependencies]` entries for `markymark-*` (seven version
   strings total, all in root `Cargo.toml`)
3. Edit `version` in `MODULE.bazel`
4. Edit `version` in `markymark-plugin/.claude-plugin/plugin.json`
5. Edit `version` in `markymark-vscode/package.json`
6. Run `cargo update -p markymark-core -p markymark-parser -p markymark-index -p markymark-kernels -p markymark-lsp -p markymark-mcp -p markymark-cli` to refresh `Cargo.lock` (faster than a full build; doesn't require Zig on PATH)
7. Run `bazel build //markymark-cli:markymark` and verify `bazel-bin/markymark-cli/markymark --version` reports the new version (not `0.0.0` — that indicates a missed `rust_binary(version = VERSION)` wiring in some `BUILD.bazel`)
8. Commit together: `version.bzl`, `Cargo.toml`, `MODULE.bazel`, `plugin.json`, `package.json`, `Cargo.lock`

**Why multiple places?** Cargo, Bazel, and the plugin manifest each have
their own version literal because none can read the others:

- `MODULE.bazel` can't `load()` arbitrary `.bzl` files (restricted evaluation context)
- `version.bzl` can't be read by Cargo or `plugin.json`
- `rust_binary` defaults `CARGO_PKG_VERSION` to `"0.0.0"` when the `version` attr is unset (rules_rust `rustc.bzl`); `version.bzl` consolidates all `BUILD.bazel` targets so they don't each need a literal

Within each system we've minimized duplication: a single `VERSION` in
`version.bzl` for all Bazel targets, `version.workspace = true` for all
member crates, and `{ workspace = true }` for all internal deps.

## crates.io Publishing

Crates must be published in dependency order (regular deps only; dev/build deps don't affect publish order).

**Publish order** (derived from `cargo metadata`):

```bash
# 1. Kernels (no regular internal deps)
cargo publish -p markymark-kernels

# 2. Core (depends on kernels)
cargo publish -p markymark-core

# 3. Parser (depends on core)
cargo publish -p markymark-parser

# 4. Index (depends on core, kernels, parser)
cargo publish -p markymark-index

# 5. LSP (depends on core, index, kernels, parser) — parallel with MCP
cargo publish -p markymark-lsp

# 6. MCP (depends on core, index, kernels, parser) — parallel with LSP
cargo publish -p markymark-mcp

# 7. CLI (depends on core, lsp, mcp)
cargo publish -p markymark-cli
```

**Important**: Wait for each crate to appear on crates.io before publishing the next. The crates.io index can lag by a few seconds — retry if you get a "not found" error for a dependency. LSP and MCP can be published in parallel since neither depends on the other.

**Re-derive publish order** before each release (crate deps may change):

```bash
cargo metadata --format-version 1 --no-deps | python3 -c "
import json, sys
meta = json.load(sys.stdin)
for p in sorted(meta['packages'], key=lambda x: x['name']):
    if not p['name'].startswith('markymark'): continue
    deps = [d['name'] for d in p['dependencies']
            if d['name'].startswith('markymark') and d.get('kind') is None]
    print(f\"{p['name']}: {deps if deps else '(none)'}\")"
```

**Dry run** (validates metadata only — inter-crate deps will fail until published):

```bash
cargo publish -p markymark-kernels --dry-run
```

Only `markymark-kernels` will pass a dry-run locally because other crates reference path dependencies that aren't on crates.io yet. This is expected.

## Git Tagging and GitHub Release

The release CI is triggered by pushing a tag matching `v*`.

```bash
# Tag the release
git tag v0.1.0

# Push the tag (triggers .github/workflows/release.yml)
git push origin v0.1.0
```

The release workflow:
1. **Builds** binaries for 5 targets (macOS ARM/Intel, Linux x64/ARM64, Windows x64)
2. **Packages** the Claude Code plugin archive with all binaries
3. **Creates** a GitHub Release with auto-generated release notes and all artifacts

### Release Artifacts

The GitHub Release will contain:
- `markymark-plugin-v0.1.0.tar.gz` — full plugin archive (all platforms)
- `markymark-aarch64-apple-darwin` — macOS Apple Silicon binary
- `markymark-x86_64-apple-darwin` — macOS Intel binary
- `markymark-x86_64-unknown-linux-gnu` — Linux x86_64 binary
- `markymark-aarch64-unknown-linux-gnu` — Linux ARM64 binary
- `markymark-x86_64-pc-windows-msvc.exe` — Windows binary
- Per-target `.tar.gz` archives for standalone installs

## Claude Code Marketplace

After the GitHub Release is created:

1. Download the plugin archive: `gh release download v0.1.0 --pattern 'markymark-plugin-*.tar.gz'`
2. Extract and verify: `tar -xzf markymark-plugin-*.tar.gz && ls markymark-plugin/`
3. Submit via Claude Code marketplace (process TBD — marketplace is not yet public)

## Post-Release Verification

After publishing, verify everything landed correctly:

```bash
# crates.io — check all 7 crates
open https://crates.io/crates/markymark-core
open https://crates.io/crates/markymark-cli

# docs.rs — auto-generated from crates.io publish
open https://docs.rs/markymark-core
open https://docs.rs/markymark-cli

# GitHub Release
gh release view v0.1.0

# Install from crates.io (end-to-end test)
cargo install markymark-cli
markymark --version
```

## Conventions

- **Tag format**: `vMAJOR.MINOR.PATCH` (e.g., `v0.1.0`, `v0.1.0-alpha.1`)
- **Pre-release tags**: `v0.1.0-alpha.1`, `v0.1.0-beta.1`
- **Branch**: Releases are always tagged from `main`
- **Changelog**: Auto-generated by GitHub Release from commit history
