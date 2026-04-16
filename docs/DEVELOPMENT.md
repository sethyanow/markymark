# Development Setup

markymark uses [mise](https://mise.jdx.dev) to pin its developer toolchain (Rust, Zig, Bazel, lefthook, cargo-audit). Versions live in [`mise.toml`](../mise.toml) at the repo root.

## One-shot bootstrap

```bash
# 1. Install mise (Linux/macOS). See https://mise.jdx.dev/installing-mise.html for alternatives.
curl https://mise.run | sh

# 2. Activate in your shell (persist by adding to ~/.bashrc, ~/.zshrc, etc.).
eval "$(mise activate bash)"    # bash
# eval "$(mise activate zsh)"   # zsh
# mise activate fish | source   # fish

# 3. Trust and install the project's pinned toolchain.
cd /path/to/markymark
mise trust
mise install
```

After this, `cd`-ing into the repo auto-selects the pinned Rust, Zig, and Bazel. Verify:

```bash
mise current          # lists resolved versions
rustc --version       # -> 1.93.1
zig version           # -> 0.15.2
bazel --version       # -> 7.4.1
```

## System dependencies (NOT managed by mise)

### Linux
Install via your distro's package manager:
- `clang`
- `llvm-ar`
- `ld.lld`

Bazel's release builds use a hermetic LLVM 21.1.8 (fetched on first build), but the Zig toolchain and Cargo canary still call the system `clang`/linker.

### macOS
```bash
brew install llvm
```
Provides `clang`, `llvm-ar`, and `ld64.lld`. `scripts/install.sh` and `.bazelrc` both reference `/opt/homebrew/bin/ld64.lld`.

## Common tasks

`mise.toml` defines wrappers for the commands documented in `CLAUDE.md`. Run `mise tasks` to list them:

| Command | What it does |
|---|---|
| `mise run build` | `bazel build //markymark-cli:markymark` (debug) |
| `mise run build-release` | Bazel release build with LTO (Linux) |
| `mise run test` | `bazel test //...` |
| `mise run test-cargo` | `cargo test --workspace` — quickest smoke test, proves Rust + Zig FFI without Bazel |
| `mise run lint` | `cargo clippy` with warnings-as-errors + `cargo fmt --check` |
| `mise run audit` | `cargo audit` |
| `mise run install-local` | Runs `scripts/install.sh` to install to `~/.local/bin` |
| `mise run lsp` | Launches the LSP server on stdio |

Raw `bazel`/`cargo` commands still work — tasks are convenience wrappers.

## First build expectations

- First `bazel build` downloads hermetic LLVM 21.1.8 (~several minutes, network required).
- First `cargo test --workspace` compiles the Zig kernels via `build.rs`.
- The `cargo:cargo-audit` tool compiles on first `mise install` (one-time, ~1 minute).

## Troubleshooting

- **`mise install` fails on a specific tool**: `mise install <tool>` isolates the failure. Check `mise doctor`.
- **Cargo and Bazel disagree on rustc version**: ensure `mise current` shows `rust 1.93.1` inside the repo; if not, run `mise trust` and restart the shell.
- **Bazel build cannot find `clang`**: install system LLVM (see above). Bazel's hermetic toolchain covers the final link but the Zig build step uses the system compiler.
