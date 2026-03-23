---
id: marky-a02
title: Expose content hash via FFI and add DocumentEngine::content_hash() method
status: open
type: task
priority: 2
parent: marky-lpb
---

## Context
Phase 1, Seam 1a of epic marky-zsys. Parent sub-epic marky-lpb.

The Zig `DocumentEngine` already computes a `content_hash: u64` on every parse
(via `content_hash_mod.content_hash(text.ptr, text.len)` at `document.zig:583`).
The hash is stored on the struct (`document.zig:62`) but NOT exposed through the
C FFI boundary. The Rust side (`markymark-kernels/src/engine.rs`) has no way to
read it.

This task adds the FFI function to expose the hash and wraps it on the Rust side.
Pure plumbing — no behavior change in the update path.

## Requirements
- R1 (from parent epic): Zig DocumentEngine exposes content hash via C FFI

## Implementation

1. **Zig C export** (`zig/src/engine/document.zig` or the C API export file):
   Add `marky_engine_get_content_hash(handle: *mut c_void) -> u64` that reads
   `self.content_hash` from the opaque handle. Return 0 if handle is null.

2. **Rust extern declaration** (`markymark-kernels/src/engine.rs:16-24`):
   Add `marky_engine_get_content_hash` to the `extern "C"` block.

3. **Rust method** (`markymark-kernels/src/engine.rs`, on `impl DocumentEngine`):
   Add `pub fn content_hash(&self) -> u64` that calls the FFI function with
   `self.handle`. Include SAFETY comment matching the pattern of `get_blob()`.

4. **Tests** (`markymark-kernels/src/engine.rs`, `tests` module):
   - `test_engine_content_hash_stable`: create engine, get hash, update with same text, hash unchanged
   - `test_engine_content_hash_changes`: create engine, get hash, update with different headings, hash differs
   - `test_engine_content_hash_after_create`: hash is non-zero after create with non-empty text
   - `test_engine_content_hash_empty`: hash is 0 for empty input

## Success Criteria
- [ ] `marky_engine_get_content_hash` exported from Zig, callable from Rust
- [ ] `DocumentEngine::content_hash()` returns u64 on Rust side
- [ ] Test: same-content stability (hash unchanged after update with same text)
- [ ] Test: different-content detection (hash changes when headings change)
- [ ] Test: non-zero hash for non-empty text
- [ ] Test: zero hash for empty text
- [ ] `cargo test -p markymark-kernels` passes
- [ ] `cargo clippy -p markymark-kernels` clean

## Anti-Patterns
- NO changing the content_hash computation in Zig (the hash algorithm is correct, just not exposed)
- NO adding hash to the blob format (the hash is metadata about the parse, not serialized content)
- NO touching `build_markdown_index_via_engine` or `ServerState` (that's seam 1b, next task)
