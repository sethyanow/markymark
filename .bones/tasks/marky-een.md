---
id: marky-een
title: Scaffold markymark-kernels Rust crate with build.rs
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---




Create markymark-kernels/ crate with Cargo.toml, build.rs, and src/lib.rs. build.rs should: check for zig compiler, run zig build lib in ../zig/, link libmarky_kernels.a. lib.rs exports empty module structure (scan, embed, similarity, tokens, hash). Add to workspace Cargo.toml members. Verify: cargo build -p markymark-kernels. See docs/plans/brza-markymark.md Section 8 and forge's rust/zig-ffi-proof/build.rs for reference.

## Design

## Goal
Create markymark-kernels/ Rust crate with Cargo.toml, build.rs, and src/lib.rs. The build.rs checks for the Zig compiler, runs zig build lib in ../zig/, and links libmarky_kernels.a. lib.rs exports empty module structure (scan, embed, similarity, tokens, hash). This crate is the Rust-side bridge to all Zig kernels.

## Effort Estimate
3-4 hours

## Success Criteria
- [ ] `cargo build -p markymark-kernels` succeeds when Zig is installed
- [ ] build.rs gracefully fails with clear error message when Zig is not installed
- [ ] build.rs gracefully fails when Zig version < 0.15.2
- [ ] Crate is added to workspace Cargo.toml members list
- [ ] lib.rs compiles with empty module stubs (pub mod scan; pub mod embed; etc.)
- [ ] `cargo test -p markymark-kernels` passes (even if no FFI tests yet)
- [ ] cargo clippy -p markymark-kernels -- -D warnings is clean

## Implementation Checklist
- [ ] Create markymark-kernels/Cargo.toml with [package] and [build-dependencies]
- [ ] Create markymark-kernels/build.rs: check zig version, run zig build lib, link static lib
- [ ] Create markymark-kernels/src/lib.rs with pub mod stubs
- [ ] Create markymark-kernels/src/scan.rs (empty)
- [ ] Create markymark-kernels/src/embed.rs (empty)
- [ ] Create markymark-kernels/src/similarity.rs (empty)
- [ ] Create markymark-kernels/src/tokens.rs (empty)
- [ ] Create markymark-kernels/src/hash.rs (empty)
- [ ] Add markymark-kernels to workspace Cargo.toml members
- [ ] Verify full build: cargo build -p markymark-kernels

## Edge Cases
- Zig not installed: build.rs should panic! with human-readable message including install URL
- Zig wrong version: build.rs should parse zig version output and reject < 0.15.2
- zig build lib fails: build.rs should forward Zig's stderr to Rust build output
- Cross-compilation: build.rs should pass --target to zig build when CARGO_CFG_TARGET_ARCH differs
- Stale library: build.rs should set rerun-if-changed for zig/src/ directory

## Anti-patterns
- NO hardcoded paths to zig binary (use which/where or PATH lookup)
- NO silently skipping Zig build on failure (must error clearly)
- NO missing rerun-if-changed directives (will cause stale builds)
- NO linking without cargo:rustc-link-search and cargo:rustc-link-lib directives
- NO unwrap/expect in build.rs without context message

## Error Handling
- Missing Zig: panic! with "Zig compiler not found. Install Zig 0.15.2+ from https://ziglang.org/download/"
- Wrong Zig version: panic! with "Zig 0.15.2+ required, found {version}"
- Zig build failure: panic! with Zig stderr output included
- Missing library artifact: panic! with "zig build lib did not produce libmarky_kernels.a"

## Test Specifications (what bug does each test catch?)
- test_lib_compiles: catches module declaration errors (missing files referenced in lib.rs)
- test_build_rs_version_parse: catches version string parsing regression (e.g., "0.15.2-dev" vs "0.15.2")
- test_link_directives: catches missing cargo:rustc-link-lib in build.rs (would fail at link time, not compile time)
