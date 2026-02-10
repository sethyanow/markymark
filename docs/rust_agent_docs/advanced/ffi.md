## FFI — Foreign Function Interface

> **TL;DR:** Use `extern "C"` with `repr(C)` types. Never pass `String`, `Vec`, `Box`, or
> any Rust allocator-backed type across FFI/DLL boundaries — each DLL has its own allocator.
> Use opaque pointer patterns with create/destroy function pairs.

### extern "C" Basics

```rust
// Calling C from Rust
extern "C" {
    fn strlen(s: *const std::ffi::c_char) -> usize;
    fn abs(input: std::ffi::c_int) -> std::ffi::c_int;
}

// Exposing Rust function to C
#[no_mangle]
pub extern "C" fn rust_add(a: i32, b: i32) -> i32 {
    a + b
}
```

### C-Compatible Types

| Rust Type | C Equivalent | Notes |
|-----------|-------------|-------|
| `i8`/`u8` | `int8_t`/`uint8_t` | Fixed-width |
| `i32`/`u32` | `int32_t`/`uint32_t` | Fixed-width |
| `std::ffi::c_int` | `int` | Platform-dependent |
| `std::ffi::c_char` | `char` | Platform-dependent signedness |
| `*const T` / `*mut T` | `const T*` / `T*` | Raw pointers |
| `bool` | `bool` / `_Bool` | C99+ |
| `()` | `void` (return only) | |
| `Option<&T>` | Nullable `const T*` | Null pointer optimization |
| `extern "C" fn` | Function pointer | |

**Non-portable types (NEVER pass across FFI):** `String`, `Vec`, `Box`, `HashMap`, `Rc`, `Arc`,
any `#[repr(Rust)]` struct, anything using Rust's allocator.

### DLL Isolation — Why String/Vec Crashes

> ⚠️ **CRITICAL MISTAKE: Passing Rust allocator-backed types across FFI/DLL boundaries**

Each Rust DLL gets its own copy of the Rust runtime, including its own **separate allocator**.
When you pass a `String` from DLL A to DLL B, and DLL B drops it, DLL B frees the memory
using *its* allocator — but the memory was allocated by DLL A's allocator. This is UB.

```rust
// ❌ CRASHES: String allocated in DLL A, freed in DLL B
// DLL A:
#[no_mangle]
pub extern "C" fn get_greeting() -> String {
    "Hello from A".to_string()  // Allocated with A's allocator
}
// DLL B drops it → frees with B's allocator → BOOM 💥

// ✅ CORRECT: Opaque handle pattern
// DLL A:
pub struct Greeting { text: String }

#[no_mangle]
pub extern "C" fn greeting_create() -> *mut Greeting {
    Box::into_raw(Box::new(Greeting {
        text: "Hello from A".to_string(),
    }))
}

#[no_mangle]
pub extern "C" fn greeting_text(g: *const Greeting) -> *const u8 {
    // SAFETY: caller guarantees g is valid and from greeting_create
    let g = unsafe { &*g };
    g.text.as_ptr()
}

#[no_mangle]
pub extern "C" fn greeting_text_len(g: *const Greeting) -> usize {
    let g = unsafe { &*g };
    g.text.len()
}

#[no_mangle]
pub extern "C" fn greeting_destroy(g: *mut Greeting) {
    if !g.is_null() {
        // SAFETY: g was created by greeting_create, freed with SAME allocator
        unsafe { drop(Box::from_raw(g)); }
    }
}
```

**Rule:** Objects are always created AND destroyed by the same DLL. Pass opaque pointers + accessor functions.

### CStr / CString String Handling

```rust
use std::ffi::{CStr, CString};

// Rust → C: create CString (owned, null-terminated)
fn pass_to_c(name: &str) {
    let c_name = CString::new(name).expect("string contains null byte");
    unsafe { c_function(c_name.as_ptr()); }
    // c_name lives until end of scope — pointer stays valid
}

// C → Rust: borrow as CStr, then convert
unsafe fn receive_from_c(ptr: *const std::ffi::c_char) -> String {
    // SAFETY: ptr is a valid null-terminated C string
    let c_str = CStr::from_ptr(ptr);
    c_str.to_string_lossy().into_owned()
}
```

### Panic Safety at FFI Boundaries

Unwinding (panic) across `extern "C"` boundaries is **undefined behavior**.
Always use `catch_unwind` at FFI entry points:

```rust
use std::panic;

#[no_mangle]
pub extern "C" fn safe_entry_point(input: i32) -> i32 {
    let result = panic::catch_unwind(|| {
        do_work(input)
    });
    match result {
        Ok(value) => value,
        Err(_) => -1,  // Return error code on panic
    }
}
```

### Opaque Pointer Pattern Summary

```
1. Define Rust struct with internal state
2. Provide extern "C" create() → *mut Type
3. Provide extern "C" accessor functions taking *const/*mut Type
4. Provide extern "C" destroy(*mut Type) — frees with originating allocator
5. Never expose internal Rust types across the boundary
```

### bindgen / cbindgen

| Tool | Direction | Usage |
|------|-----------|-------|
| `bindgen` | C → Rust | Generates Rust bindings from C headers |
| `cbindgen` | Rust → C | Generates C headers from Rust code |

```toml
# build.rs for bindgen
# [build-dependencies]
# bindgen = "0.69"
```

### References

- Nomicon: [FFI](https://doc.rust-lang.org/nomicon/ffi.html)
- Guidelines: [M-ISOLATE-DLL-STATE](../../docs/rust_guidelines/ffi.md)
- Guidelines: [M-UNSAFE](../../docs/rust_guidelines/safety.md)
- Related: [advanced/type-layout.md](type-layout.md) (repr(C)), [advanced/unsafe.md](unsafe.md), [checklists/ffi-audit.md](../checklists/ffi-audit.md)
