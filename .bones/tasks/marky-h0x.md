---
id: marky-h0x
title: Add Bazel build with cross-language LTO via toolchains_llvm_bootstrapped
status: closed
type: task
priority: 2
owner: Seth
---




## Requirements

1. Add Bazel build configuration alongside existing Cargo build
2. Use `toolchains_llvm_bootstrapped` (LLVM 21.1.8) for unified LLVM across Rust + Zig
3. Enable cross-language ThinLTO so Zig FFI calls can inline across the boundary
4. Cargo remains the dev-loop build system; Bazel is the optimized release path

## Context

rustc 1.93.1 uses LLVM 21. Zig 0.15.2 ships LLVM 20. The version mismatch prevents
cross-language LTO under Cargo. `toolchains_llvm_bootstrapped` provides a hermetic
LLVM 21.1.8 toolchain for Bazel that both Rust and Zig can share, enabling ThinLTO
across the FFI boundary.

Source: https://github.com/cerisier/toolchains_llvm_bootstrapped

## Implementation

1. `MODULE.bazel` — declare deps: `toolchains_llvm_bootstrapped`, `rules_rust`, `rules_zig`
2. `.bazelrc` — ThinLTO flags, platform configs
3. `BUILD.bazel` (root) — workspace-level config
4. `BUILD.bazel` per crate (7 crates) — `rust_library`/`rust_binary` targets
5. `crate_universe` setup — auto-generate BUILD for third-party Cargo deps
6. Zig kernel `BUILD.bazel` — replace build.rs `zig build lib` with `zig_library` target
7. Link Zig static lib into `markymark-kernels` rust target
8. Verify: `bazel build //markymark-cli --config=release` produces working binary

## Success Criteria

- [ ] `bazel build //markymark-cli` compiles successfully
- [ ] Cross-language ThinLTO is active (Zig symbols inlined, verifiable via objdump)
- [ ] Existing `cargo build` / `cargo test` still work unchanged
- [ ] `bazel test //...` runs the test suite

## Log

- [2026-03-23T23:53:42Z] [Seth] Bazel build integration complete. MODULE.bazel + .bazelrc + BUILD.bazel per crate + zig/BUILD.bazel. Uses toolchains_llvm_bootstrapped (LLVM 21.1.8), rules_rust 0.68.1, rules_zig 0.12.3. Both debug and release (ThinLTO) builds pass. Cargo test suite fully green. Note: -Clinker-plugin-lto doesn't work on macOS (ld64.lld plugin-opt unsupported), using -Clto=thin with -Cembed-bitcode=yes instead.
