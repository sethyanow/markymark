---
id: marky-a02
title: Expose content hash via FFI and add DocumentEngine::content_hash() method
status: closed
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

1. **Zig C export** (`zig/src/engine/exports.zig` — same file as `marky_engine_create` etc.):
   Add `export fn marky_engine_get_content_hash(handle: ?*anyopaque) u64` that uses
   `castHandle(handle)` to resolve the `*DocumentEngine`, then returns `engine.content_hash`.
   Return 0 if handle is null (via `orelse return 0`). Follow the existing pattern:
   doc comment, null-guard via `castHandle`, direct field access.

2. **Rust extern declaration** (`markymark-kernels/src/engine.rs:16-24`):
   Add `fn marky_engine_get_content_hash(handle: *mut std::ffi::c_void) -> u64;` to
   the `extern "C"` block.

3. **Rust method** (`markymark-kernels/src/engine.rs`, on `impl DocumentEngine`):
   Add `pub fn content_hash(&self) -> u64` that calls the FFI function with
   `self.handle`. SAFETY comment: "handle is valid (created by marky_engine_create,
   not yet destroyed). content_hash is a pure field read — no mutation, no allocation."
   Follow the `nosemgrep` comment pattern from `get_blob()`.

4. **Tests** (`markymark-kernels/src/engine.rs`, `tests` module):
   - `test_engine_content_hash_stable`: create engine with `"# Hello\n"`, get hash,
     update with same text, assert hash unchanged. Catches: non-deterministic hashing
     or update-path divergence from create-path.
   - `test_engine_content_hash_changes`: create engine with `"# Hello\n"`, get hash,
     update with `"# Hello\n## World\n"`, assert hash differs. Catches: hash that
     doesn't reflect content changes.
   - `test_engine_content_hash_after_create`: create engine with `"# Heading\n"`,
     assert hash != 0. Catches: FFI returning zero/garbage for valid input.
   - `test_engine_content_hash_empty`: create engine with `""`, assert hash == 0.
     Matches Zig behavior: `content_hash_mod.content_hash()` skipped when text.len == 0,
     field stays at default 0.

## Success Criteria
- [x] `marky_engine_get_content_hash` exported from Zig, callable from Rust
- [x] `DocumentEngine::content_hash()` returns u64 on Rust side
- [x] Test: same-content stability (hash unchanged after update with same text)
- [x] Test: different-content detection (hash changes when headings change)
- [x] Test: non-zero hash for non-empty text
- [x] Test: zero hash for empty text
- [x] `cargo test -p markymark-kernels` passes
- [x] `cargo clippy -p markymark-kernels` clean

## Anti-Patterns
- NO changing the content_hash computation in Zig (the hash algorithm is correct, just not exposed)
- NO adding hash to the blob format (the hash is metadata about the parse, not serialized content)
- NO touching `build_markdown_index_via_engine` or `ServerState` (that's seam 1b, next task)
- NO adding the export to `c_adapter.zig` — engine C exports live in `zig/src/engine/exports.zig`
- NO calling the standalone `marky_content_hash` from `c_adapter.zig` (that takes raw text; we need the engine's stored field value which reflects the post-parse hash)
- NO returning an error code (i32) — this is a simple field read that returns u64 directly, matching the pattern of `marky_estimate_tokens` not `marky_engine_get_blob`

## Key Considerations

### Null handle returns 0 (ambiguous with empty-text hash)
The `content_hash` field defaults to 0, and empty text produces hash 0. Returning 0 for
null handle is therefore ambiguous. This is acceptable because the Rust `DocumentEngine`
struct guarantees a non-null handle (constructor fails if `create` returns null, `Drop`
nulls it only on destroy). The Rust wrapper `content_hash(&self)` can only be called on
a live engine — the null path is defense-in-depth, never reached in normal operation.

### FFI u64 alignment
Zig u64 and Rust u64 have identical C ABI representation on all supported platforms.
No special alignment or conversion needed.

### Thread safety
`DocumentEngine` is `Send` but NOT `Sync`. `content_hash()` is a read of a field that
is only written during `update()` (which takes `&mut self`). The borrow checker prevents
concurrent read + write. `ServerState` wraps engines in `Mutex<DocumentEngine>`,
serializing all access. No concurrent-access risk.

### Zig build integration
No `build.zig` changes needed. `export fn` in `exports.zig` produces a C-linkage symbol
automatically, just like the existing 4 engine exports. The Zig static library already
links into `markymark-kernels` via the `cc` crate build script.

## Log

- [2026-03-23T15:03:08Z] [Seth] Completed: FFI hash exposure (Phase 1a). Added marky_engine_get_content_hash to exports.zig + Rust extern + content_hash() method. 12 tests (4 core, 8 adversarial). All 64 workspace test suites pass. Clippy clean. SRE review sharpened skeleton: fixed file path ambiguity (exports.zig not c_adapter.zig), added 3 new anti-patterns, added Key Considerations. No surprises — pure plumbing task matched skeleton exactly.
