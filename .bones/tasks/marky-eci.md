---
id: marky-eci
title: Explore eliminating Zig FFI layer — direct Zig integration under unified Bazel build
status: open
type: task
priority: 2
---

## Requirements

Explore whether the Zig FFI boundary can be eliminated now that Bazel unifies the build.
The `extern "C"` funnel (markymark-kernels/src/*.rs) and all the `from_` conversion methods
that marshal data across the C ABI boundary may no longer be needed — Zig code could be
called more directly without the C adapter layer and `#[repr(C)]` struct conversions.

## Context

With Bazel + toolchains_llvm_bootstrapped, Rust and Zig share the same LLVM. The C ABI
FFI layer exists because Cargo couldn't do cross-language linking any other way. Under
Bazel, there may be a path to tighter integration that eliminates the conversion overhead
entirely.
