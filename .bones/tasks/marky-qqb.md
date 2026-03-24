---
id: marky-qqb
title: Bazel primary build system — cross-language ThinLTO, CI, release, install
status: active
type: task
priority: 1
owner: Seth
parent: marky-o8e
---



## Context

Promoted Bazel from "alongside Cargo" to the primary build system for markymark.
Work done on `optimize` branch (2026-03-24). Cargo kept as lightweight canary.

## What was done

### Cross-language ThinLTO (Rust ↔ Zig)
- Patched `rules_zig` 0.12.3 with two-step bitcode build (`-femit-llvm-bc` → clang wrap → llvm-ar)
- `clang-lto-wrapper` strips `-plugin-opt` args that macOS ld64.lld rejects
- `-Clinker-plugin-lto` makes rustc emit bitcode for the linker (required for cross-lang)
- LTO canary test (`lto_eliminates_fault_injection`) confirms optimization is active
- `.llvm.NNN` suffixes on Zig internal symbols prove cross-module LTO processing

### Platform config
- `MODULE.bazel`: `apple_support` for macOS CC, `toolchains_llvm_bootstrapped` Linux-only (Apple SDK 403)
- `.bazelrc` split: `build:release` (common LTO), `build:macos-lto` (Homebrew clang/ld64.lld)
- `extra_rustc_flag` (singular, accumulates) for platform-specific additions

### CI migration (ci.yml)
- `bazel-build-and-test`: primary job, hermetic toolchains, no setup-zig, no Zig cache workarounds
- `lint`: cargo fmt + clippy (stays Cargo)
- `cargo-canary`: Cargo compatibility check
- Miri + benchmarks: stay Cargo
- **Untested on actual GitHub runners**

### Release migration (release.yml)
- Bazel for macOS (arm64, x86_64) and Linux x86_64 native builds with LTO
- Cargo for Windows and Linux aarch64 cross-compile
- `brew install llvm` step for macOS runners
- **Untested on actual GitHub runners**

### Local install
- `scripts/install.sh` → builds with Bazel LTO → copies to `~/.local/bin/`
- Auto-detects macOS for `--config=macos-lto`
- Verified locally

### Prereqs
- Homebrew LLVM: `brew install llvm` (macOS)
- `clang-lto-wrapper` installed at `/opt/homebrew/opt/llvm/bin/clang-lto-wrapper`

## Remaining

- [ ] Validate CI workflows on GitHub Actions runners
- [ ] Validate release matrix on GitHub Actions runners
- [ ] Update marky-o8e success criteria based on what landed

## Commits

- `e5cffe36` feat: enable cross-language ThinLTO between Rust and Zig
- `cb74c067` test: flip fault-injection test into LTO canary
- `62596a10` feat: promote Bazel to primary build system
- `2a4b7389` bones: create marky-qqb

## Log

- [2026-03-24T20:40:23Z] [Seth] Session log: LTO confirmed via .llvm.NNN suffixes, config split validated via bazel cquery, install script tested, all 7 Bazel tests + all Cargo canary tests pass locally. CI/release workflows written but untested on runners.
