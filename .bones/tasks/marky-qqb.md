---
id: marky-qqb
title: Validate Bazel CI/release workflows against GitHub Actions runners
status: active
type: task
priority: 1
owner: Seth
parent: marky-o8e
---




## Requirements

1. Open PR from `optimize` branch to trigger `ci.yml` on GitHub Actions
2. Validate `bazel-build-and-test` job passes on `ubuntu-latest` (hermetic toolchains)
3. Validate `lint` job passes (cargo fmt/clippy)
4. Validate `cargo-canary` job passes
5. Dry-run release matrix: macOS (`brew install llvm` + Bazel LTO), Linux (hermetic Bazel), Windows (Cargo fallback)

## Context

Bazel was promoted to primary build system (optimize branch, 2026-03-24). CI and release
workflows rewritten but untested on actual GitHub runners. Key risks:
- `bazelbuild/setup-bazelisk@v3` may need version adjustment
- macOS runner Homebrew LLVM install path may differ from local (`/opt/homebrew/`)
- Bazel cache key and `~/.cache/bazel` path may not work on runners
- `toolchains_llvm_bootstrapped` Linux toolchain download may timeout or fail
- `apple_support` SDK resolution on GitHub macOS runners

Parent: marky-o8e (Bazel adoption refinement)

## Success Criteria

- [ ] `bazel-build-and-test` job green on ubuntu-latest
- [ ] `lint` job green (cargo fmt + clippy)
- [ ] `cargo-canary` job green
- [ ] Release matrix: macOS arm64 Bazel build produces binary artifact
- [ ] Release matrix: Linux x86_64 Bazel build produces binary artifact
- [ ] Release matrix: Windows Cargo build still works (no regression)
