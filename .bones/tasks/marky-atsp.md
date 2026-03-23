---
id: marky-atsp
title: 'Task 2: FFI exports (Zig C ABI) + Rust DocumentEngine wrapper'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-6jzs]
parent: marky-io3h
---




## Goal

Expose the Zig DocumentEngine (from marky-6jzs) across the FFI boundary and provide a safe Rust wrapper. After this task, Rust code can create/update/get_blob/destroy a DocumentEngine via the markymark-kernels crate.

## Context

Task 1 (marky-6jzs) implemented the Zig-side DocumentEngine struct with create/update/getBlob/destroy methods and flat binary blob serialization. This task adds the FFI layer so Rust can call those methods.

## Implementation

### 1. Zig C ABI exports: zig/src/engine/exports.zig

Following the proven pattern from exports_embed.zig:

- marky_engine_create(text: ?[*]const u8, text_len: u32) -> ?*anyopaque
  - Null text with len=0: create empty engine, return handle
  - Null text with len>0: return null
  - Allocates with page_allocator (matches exports_embed.zig pattern)
  - Returns opaque handle on success, null on failure

- marky_engine_update(handle: ?*anyopaque, text: ?[*]const u8, text_len: u32) -> i32
  - 0: success, -1: invalid input, -3: allocation failure, -4: parse failure
  - Null text with len=0: update to empty document
  - Null text with len>0: return -1
  - On error: old state preserved (DocumentEngine.update() guarantees this)

- marky_engine_get_blob(handle: ?*anyopaque, blob_ptr: ?*[*]const u8, blob_len: ?*u32) -> i32
  - 0: success (blob_ptr and blob_len set), -1: invalid input, -3: allocation failure
  - Blob memory owned by engine, valid until next update() or destroy()
  - Caller must NOT free the returned blob pointer

- marky_engine_destroy(handle: ?*anyopaque) -> void
  - Null handle is a no-op (safe double-free pattern)
  - Frees engine and all owned memory

### 2. Wire into build: zig/src/c_adapter.zig

Add comptime import in the comptime block:
  _ = @import("engine/exports.zig");

### 3. Rust FFI declarations + safe wrapper: markymark-kernels/src/engine.rs

Following the proven pattern from embed.rs:

extern "C" declarations for all 4 functions.

pub struct DocumentEngine {
    handle: *mut c_void,
}

- DocumentEngine::new(text: &str) -> Result<Self, KernelError>
- DocumentEngine::update(&mut self, text: &str) -> Result<(), KernelError>
- DocumentEngine::get_blob(&self) -> Result<ScanBlob<'_>, KernelError>
- Drop impl: null-checks handle before destroy, sets to null after
- unsafe impl Send + Sync with safety comments (matches embed.rs pattern)
- Debug impl showing handle state

pub struct ScanBlob<'a> {
    data: &'a [u8],
}

ScanBlob is a thin view over the raw blob bytes returned by get_blob().
Provides:
- data() -> &[u8]: raw blob bytes
- len() -> usize: blob size
- header validation delegated to from_blob() in Task 3

### 4. Wire into crate: markymark-kernels/src/lib.rs

Add: pub mod engine;

### 5. Zig-side tests (in exports.zig)

- test_engine_create_and_destroy: create("# Hello\n") -> non-null handle -> destroy
- test_engine_create_null_text_zero_len: create(null, 0) -> non-null (empty engine)
- test_engine_create_null_text_nonzero_len: create(null, 10) -> null
- test_engine_destroy_null: destroy(null) -> no crash
- test_engine_update_basic: create -> update("# New\n") -> 0
- test_engine_update_null_handle: update(null, ...) -> -1
- test_engine_update_null_text_nonzero_len: update(handle, null, 10) -> -1
- test_engine_get_blob_basic: create -> get_blob -> validates header magic/version
- test_engine_get_blob_null_handle: get_blob(null, ...) -> -1
- test_engine_lifecycle: create -> update x10 -> get_blob -> destroy (no crash)

### 6. Rust-side tests (in engine.rs)

- test_engine_lifecycle: new -> update -> get_blob -> validate header -> drop
- test_engine_empty_input: new("") -> get_blob -> 64 bytes (empty blob)
- test_engine_update_changes_blob: new("# A") -> blob1 -> update("# B") -> blob2 -> different
- test_engine_multiple_updates: new -> update 100x -> drop (no leak/crash)
- test_engine_is_send_and_sync: compile-time Send + Sync assertions
- test_engine_blob_header_valid: new("# Hello\n[link](url) #tag ^id\n") -> get_blob -> validate magic=0x4D4B5343, version=1, counts > 0

## Success Criteria
- [ ] All 4 Zig export functions compile and export (nm -g shows symbols)
- [ ] All 10 Zig export tests pass (zig build test)
- [ ] All 6 Rust tests pass (cargo nextest -p markymark-kernels)
- [ ] Null handle safety: all functions handle null gracefully (no crash)
- [ ] Blob pointer valid: get_blob returns readable blob that validates
- [ ] Zero clippy warnings for engine.rs
- [ ] Existing tests still pass (cargo nextest)

## Anti-Patterns (FORBIDDEN)
- NO exposing DocumentEngine pointer directly (always opaque ?*anyopaque)
- NO Rust code freeing blob memory (blob owned by engine, freed on update/destroy)
- NO skipping null checks on any FFI parameter
- NO panic paths in export functions (return error codes)

## Design

## Goal

Expose the Zig DocumentEngine (from marky-6jzs) across the FFI boundary and provide a safe Rust wrapper. After this task, Rust code can create/update/get_blob/destroy a DocumentEngine via the markymark-kernels crate.

## Effort Estimate

6-8 hours. Zig exports ~100-150 lines, Rust wrapper ~200-250 lines, 2 trivial wiring changes, 20 tests total. Proven patterns exist for both sides (exports_embed.zig, embed.rs).

## Context

Task 1 (marky-6jzs) implemented the Zig-side DocumentEngine struct with create/update/getBlob/destroy methods and flat binary blob serialization (35 tests, GPA leak-free). This task adds the FFI layer so Rust can call those methods.

## Implementation

### 1. Zig C ABI exports: zig/src/engine/exports.zig

Following the proven pattern from exports_embed.zig (opaque handle + castHandle helper):

- marky_engine_create(text: ?[*]const u8, text_len: u32) -> ?*anyopaque
  - Null text with len=0: create empty engine (DocumentEngine.create("", page_allocator)), return handle
  - Null text with len>0: return null
  - Allocates with page_allocator (matches exports_embed.zig pattern)
  - Returns opaque handle on success, null on failure (allocation or parse error)

- marky_engine_update(handle: ?*anyopaque, text: ?[*]const u8, text_len: u32) -> i32
  - 0: success, -1: invalid input (null handle, null text with len>0), -3: OOM, -4: parse failure
  - Null text with len=0: update to empty document
  - Null text with len>0: return -1
  - On error: old state preserved (DocumentEngine.update() guarantees this)
  - Error mapping: error.OutOfMemory -> -3, error.ParseFailed -> -4

- marky_engine_get_blob(handle: ?*anyopaque, blob_ptr: ?*[*]const u8, blob_len: ?*u32) -> i32
  - 0: success (blob_ptr and blob_len written), -1: invalid input (null handle or null output ptrs), -3: OOM
  - Calls engine.getBlob() which returns []const u8 (lazy cached)
  - Sets blob_ptr.* = slice.ptr, blob_len.* = @intCast(slice.len)
  - Blob memory owned by engine, valid until next update() or destroy()
  - Caller must NOT free the returned blob pointer

- marky_engine_destroy(handle: ?*anyopaque) -> void
  - Null handle is a no-op (safe double-free pattern)
  - Calls engine.destroy() which frees all owned memory
  - Frees the engine struct itself

Helper function (private):
  fn castHandle(handle: ?*anyopaque) ?*DocumentEngine
    Matches pattern from exports_embed.zig. Uses @ptrCast(@alignCast(ptr)).

### 2. Wire into build: zig/src/c_adapter.zig

Add in the comptime block (after md4c/exports.zig):
  _ = @import("engine/exports.zig");

This ensures export fn symbols are included in libmarky_kernels.a.

Engine tests are already wired via: _ = @import("engine/document.zig") in the test block.
Add engine exports tests: _ = @import("engine/exports.zig"); in the test block.

### 3. Rust FFI declarations + safe wrapper: markymark-kernels/src/engine.rs

Following the proven pattern from embed.rs:

extern "C" declarations for marky_engine_create, marky_engine_update, marky_engine_get_blob, marky_engine_destroy. All handles are *mut std::ffi::c_void.

pub struct DocumentEngine with handle: *mut std::ffi::c_void.

DocumentEngine::new(text: &str) -> Result<Self, KernelError>:
- MUST check text.len() <= u32::MAX, return InvalidInput if exceeds
- Empty text: pass (std::ptr::null(), 0) -- Zig side handles this
- Non-empty: pass (text.as_ptr(), text.len() as u32)
- Null return: Err(KernelError::InternalError(-3))

DocumentEngine::update(&mut self, text: &str) -> Result<(), KernelError>:
- MUST check text.len() <= u32::MAX
- Empty text: pass (std::ptr::null(), 0)
- Map rc: 0 -> Ok(()), -1 -> InvalidInput, -3 -> InternalError(-3), -4 -> InternalError(-4)

DocumentEngine::get_blob(&self) -> Result<ScanBlob, KernelError>:
- Stack-local blob_ptr: *const u8 = null, blob_len: u32 = 0
- Call FFI, map rc: 0 -> construct ScanBlob from raw parts, -1 -> InvalidInput, -3 -> InternalError
- SAFETY: blob_ptr/blob_len valid for duration of engine (until update/destroy)
- ScanBlob lifetime tied to &self -- borrow checker prevents update() while ScanBlob exists

Drop impl:
- if !self.handle.is_null() { destroy(self.handle); self.handle = null_mut(); }

unsafe impl Send for DocumentEngine {} with safety comment (matches embed.rs)
unsafe impl Sync for DocumentEngine {} with safety comment (matches embed.rs)

Debug impl: show handle_null status

pub struct ScanBlob with data: &[u8] (lifetime tied to DocumentEngine borrow).

ScanBlob methods:
- pub fn data(&self) -> &[u8]
- pub fn len(&self) -> usize
- pub fn is_empty(&self) -> bool

### 4. Wire into crate: markymark-kernels/src/lib.rs

Add: pub mod engine;

### 5. Zig-side tests (in exports.zig) -- 12 tests

- test_engine_create_and_destroy: create("# Hello\n") -> non-null handle -> destroy. Catches: FFI creation path works.
- test_engine_create_null_text_zero_len: create(null, 0) -> non-null (empty engine). Catches: empty input handled.
- test_engine_create_null_text_nonzero_len: create(null, 10) -> null. Catches: null+len mismatch rejected.
- test_engine_destroy_null: destroy(null) -> no crash. Catches: double-free safety.
- test_engine_update_basic: create -> update("# New\n") -> rc==0. Catches: update through FFI.
- test_engine_update_null_handle: update(null, "text", 4) -> -1. Catches: null handle rejected.
- test_engine_update_null_text_nonzero_len: update(handle, null, 10) -> -1. Catches: null text rejected.
- test_engine_get_blob_basic: create("# Hello\n") -> get_blob -> rc==0 -> validate magic/version from blob_ptr. Catches: blob serialization through FFI.
- test_engine_get_blob_null_handle: get_blob(null, &ptr, &len) -> -1. Catches: null handle rejected.
- test_engine_get_blob_null_output_ptrs: get_blob(handle, null, null) -> -1. Catches: null output params rejected.
- test_engine_get_blob_caching: create -> get_blob twice -> same ptr. Catches: blob caching works through FFI.
- test_engine_lifecycle: create -> update x10 with varied markdown -> get_blob (validate) -> destroy. Catches: full lifecycle without crash/leak.

### 6. Rust-side tests (in engine.rs) -- 8 tests

- test_engine_lifecycle: new("# Hello") -> update("# World") -> get_blob() -> validate header magic == 0x4D4B5343 -> drop. Catches: end-to-end FFI roundtrip works.
- test_engine_empty_input: new("") -> get_blob() -> len == 64 (empty blob is header only). Catches: empty input edge case.
- test_engine_update_changes_blob: new("# A") -> blob1.len() -> update("# B\n## C") -> blob2.len() -> assert different. Catches: blob cache invalidation across FFI.
- test_engine_multiple_updates: new("# Init") -> update 100x with varied text -> drop. Catches: memory leaks in repeated updates.
- test_engine_is_send_and_sync: fn assert_send<T: Send>() {} + fn assert_sync<T: Sync>() {} assertions. Catches: threading contract.
- test_engine_blob_header_valid: new("# Hello\n[link](url) #tag ^id\n") -> get_blob -> read first 4 bytes == [0x43, 0x53, 0x4B, 0x4D] (little-endian magic). Catches: blob format correctness across FFI.
- test_engine_blob_caching: new("# Test") -> get_blob() twice -> both return same data. Catches: caching works through FFI boundary.
- test_engine_debug_format: verify Debug impl produces useful output. Catches: Debug impl exists and shows handle state.

## Success Criteria
- [ ] All 4 Zig export functions compile and export (nm -g libmarky_kernels.a | grep marky_engine shows 4 symbols)
- [ ] All 12 Zig export tests pass (zig build test)
- [ ] All 8 Rust tests pass (cargo nextest -p markymark-kernels)
- [ ] Null handle safety: all functions handle null gracefully (no crash) -- verified by 5 null-specific tests
- [ ] Blob pointer valid: get_blob returns readable blob, magic == 0x4D4B5343, version == 1
- [ ] Zero clippy warnings for engine.rs (cargo clippy -p markymark-kernels)
- [ ] Existing tests still pass (cargo nextest --workspace)
- [ ] Blob lifetime safety: ScanBlob borrows &self, preventing use-after-update (borrow checker enforced)

## Anti-Patterns (FORBIDDEN)
- NO exposing DocumentEngine pointer directly (always opaque ?*anyopaque)
- NO Rust code freeing blob memory (blob owned by engine, freed on update/destroy)
- NO skipping null checks on any FFI parameter
- NO panic paths in export functions (return error codes, never @panic or unreachable)
- NO .unwrap() or .expect() in production code -- use Result/? operator
- NO casting text.len() to u32 without overflow check (text > 4GB would truncate)
- NO @ptrCast without @alignCast (alignment safety, per marky-5rq lesson)
- NO unsafe blocks without nosemgrep annotations (codebase convention)

## Key Considerations (SRE Review)

**Edge Case: text_len u32 truncation**
- Rust &str.len() returns usize (64-bit on aarch64). Casting to u32 without check silently truncates.
- MUST guard: if text.len() > u32::MAX as usize { return Err(InvalidInput); }
- Practical risk: very low (markdown files rarely >4GB) but must not silently corrupt.

**Edge Case: get_blob null output parameters**
- Both blob_ptr and blob_len must be non-null before writing through them.
- Test: test_engine_get_blob_null_output_ptrs verifies this returns -1.

**Edge Case: get_blob after failed update**
- If update() fails (parse error), old state preserved. get_blob() returns the previous blob.
- This is correct behavior -- Zig DocumentEngine.update() guarantees old state on error.
- Not tested explicitly (hard to trigger md4c parse failure on valid UTF-8), but documented.

**Safety: ScanBlob lifetime**
- get_blob(&self) returns ScanBlob borrowing &self. The borrow checker prevents calling update(&mut self) while ScanBlob exists. This is the key safety guarantee -- no runtime cost, compile-time enforced.

**Safety: Send + Sync**
- DocumentEngine wraps an opaque FFI handle with no thread-local state.
- All mutation goes through &mut self (Rust borrow checker serializes access).
- In practice, LSP runtime wraps in RwLock. Same pattern as EmbeddingIndex (embed.rs).
- MUST include nosemgrep annotations for unsafe impls.

**Build wiring: c_adapter.zig**
- comptime block: _ = @import("engine/exports.zig"); ensures export symbols are included in .a
- test block: _ = @import("engine/exports.zig"); ensures export tests run with zig build test
- document.zig is already wired for internal engine tests.

**Rust nosemgrep annotations**
- All unsafe blocks MUST have nosemgrep comments matching existing codebase pattern.
