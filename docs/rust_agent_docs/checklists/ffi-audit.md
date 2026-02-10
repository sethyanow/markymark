## FFI Boundary Audit Checklist

> **TL;DR:** Use this checklist when adding or reviewing FFI code.

### Type Safety
- [ ] No `String`, `Vec`, `Box`, `HashMap` crossing FFI boundary
- [ ] All structs passed to/from C are `#[repr(C)]`
- [ ] Opaque pointers used with paired create/destroy functions
- [ ] Create and destroy functions are in the SAME DLL/library
- [ ] Non-portable types identified and wrapped
- [ ] No `#[repr(Rust)]` (default) types crossing the boundary

### Pointer Safety
- [ ] Null checks on all incoming pointers
- [ ] Pointer validity documented in function's `# Safety` section
- [ ] Lifetime of borrowed data clearly documented
- [ ] No raw pointers used after corresponding destroy/free call
- [ ] `Option<&T>` used for nullable pointer optimization where applicable

### String Handling
- [ ] `CStr`/`CString` used for C string interop (not `String`)
- [ ] NUL byte handling addressed (`CString::new` checked for interior NULs)
- [ ] UTF-8 validation performed for C→Rust string conversion
- [ ] String encoding expectations documented

### Panic Safety
- [ ] `catch_unwind` used at all `extern "C"` function entry points
- [ ] Panic results mapped to error codes (not silent swallowing)
- [ ] All Rust code paths within FFI functions are `UnwindSafe`

### Build & Bindings
- [ ] `bindgen` or `cbindgen` used for binding generation (not manual)
- [ ] Generated bindings are checked in or reproducibly generated
- [ ] `-sys` crate follows naming convention if wrapping native library
- [ ] `build.rs` handles native compilation with `cc` crate
- [ ] Static and dynamic linking both supported where feasible

### DLL Isolation
- [ ] No statics (`static`, thread-local) shared across DLL boundary
- [ ] No `TypeId` depended upon across DLL boundary
- [ ] Each DLL's allocator only frees its own allocations
- [ ] Libraries relying on statics (`tokio`, `log`) not shared across DLLs

### References
- Detail: [advanced/ffi.md](../advanced/ffi.md), [advanced/type-layout.md](../advanced/type-layout.md)
- Guidelines: [M-ISOLATE-DLL-STATE](../../docs/rust_guidelines/ffi.md)
