---
id: marky-o8e
title: Bazel adoption refinement — CI, release process, and docs
status: open
type: task
priority: 2
depends_on: [marky-qqb]
---



## Requirements

1. CI pipeline: add `bazel build //markymark-cli:markymark --config=release` alongside existing cargo test
2. prepare-release skill: awareness of Bazel artifacts (verify Bazel build passes as a release gate)
3. RELEASING.md updates for dual build system workflow
4. Cross-platform .bazelrc: Linux CI can use `-Clinker-plugin-lto` (full cross-lang LTO), macOS uses `-Clto=thin` (current)
5. Documentation: docs-site coverage of Bazel build option for users/contributors

## Context

Bazel build with `toolchains_llvm_bootstrapped` (LLVM 21.1.8) was added in marky-h0x (2026-03-23).
Currently works locally but is not integrated into CI, release process, or contributor docs.

Key platform difference: macOS ld64.lld rejects `-plugin-opt` args from `-Clinker-plugin-lto`,
so macOS uses `-Clto=thin,-Cembed-bitcode=yes`. Linux LLD supports full `-Clinker-plugin-lto`
which enables deeper cross-boundary inlining. The .bazelrc should be platform-aware.

## Success Criteria

- [ ] GitHub Actions workflow includes Bazel release build as a CI step
- [ ] prepare-release skill verifies Bazel build passes before tagging
- [ ] RELEASING.md documents Bazel release build step
- [ ] .bazelrc has platform-specific LTO config (Linux: linker-plugin-lto, macOS: lto=thin)
- [ ] docs-site or README documents Bazel build option for contributors
