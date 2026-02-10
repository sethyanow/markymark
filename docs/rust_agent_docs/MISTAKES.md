## Common Agent Mistakes — Quick Reference

> **TL;DR:** These are the mistakes AI agents make most often when writing Rust.
> Check this list before submitting code. Each entry links to detailed guidance.

### Severity Legend

| Icon | Severity | Meaning |
|------|----------|---------|
| 🔴 | CRITICAL | Undefined behavior or unsoundness |
| 🟠 | HIGH | Likely bugs or incorrect semantics |
| 🟡 | MEDIUM | Non-idiomatic, fragile, or hard to maintain |

### Mistake Table

| # | Mistake | Sev | File | One-Line Fix |
|---|---------|-----|------|--------------|
| 1 | Taking references to packed struct fields | 🔴 | [type-layout](advanced/type-layout.md) | Read by value or `read_unaligned()` |
| 2 | Passing String/Vec/Box across FFI/DLL | 🔴 | [ffi](advanced/ffi.md) | Use opaque handles with create/destroy |
| 3 | Wrong PhantomData variance | 🔴 | [unsafe](advanced/unsafe.md) | Match Nomicon variance table exactly |
| 4 | Defaulting to Ordering::Relaxed | 🟠 | [concurrency](advanced/concurrency.md) | Default to SeqCst; downgrade with proof |
| 5 | Ignoring pinning in async code | 🟠 | [async](advanced/async.md) | Use `Box::pin()` or `pin!()` macro |
| 6 | Fighting borrow checker with `.clone()` | 🟡 | [ownership](core/ownership.md) | Restructure algorithm or use Rc/Arc |
| 7 | Using `unwrap()` in library code | 🟡 | [errors](core/errors.md) | Use `?` propagation with proper errors |
| 8 | Leaking external crate types in API | 🟡 | [api-design](patterns/api-design.md) | Wrap in newtypes |
| 9 | Glob imports in libraries | 🟡 | [modules](core/modules.md) | Explicit `pub use` with `#[doc(inline)]` |
| 10 | Non-descriptive error types | 🟡 | [errors](core/errors.md) | Use `thiserror` with descriptive variants |

---

### CRITICAL Mistake Details

> ⚠️ **CRITICAL MISTAKE #1: Taking references to packed struct fields**
> Fields in `#[repr(packed)]` structs may be unaligned. Taking a reference (`&field`)
> creates an unaligned reference, which is **undefined behavior**.

```rust
#[repr(packed)]
struct Packed { x: u8, y: u32 }

// ❌ DON'T: UB — &p.y may be unaligned
// let r = &packed_val.y;

// ✅ DO: Read by value (the compiler inserts unaligned read)
let val = packed_val.y;
```

> ⚠️ **CRITICAL MISTAKE #2: Passing Rust allocator-backed types across FFI**
> Each Rust DLL has its own allocator. Passing `String`, `Vec`, `Box`, or `HashMap`
> across FFI boundaries means freeing with the wrong allocator — this is **UB**.

```rust
// ❌ DON'T: String uses Rust's allocator — crashes in different DLL
// extern "C" fn get_name() -> String { ... }

// ✅ DO: Return opaque handle, free in originating DLL
extern "C" fn create_name() -> *mut Name { ... }
extern "C" fn destroy_name(ptr: *mut Name) { ... }
```

> ⚠️ **CRITICAL MISTAKE #3: Wrong PhantomData variance**
> Using `PhantomData<T>` when you need `PhantomData<fn(T)>` (or vice versa) makes
> your abstraction **unsound**. Always consult the variance table in [advanced/unsafe.md](advanced/unsafe.md).

```rust
// ❌ DON'T: PhantomData<T> is covariant over T
// struct Inv<T> { _marker: PhantomData<T> } // WRONG if invariance needed

// ✅ DO: PhantomData<fn(T) -> T> is invariant over T
struct Inv<T> { _marker: PhantomData<fn(T) -> T> }
```

### References

- Nomicon: [PhantomData](https://doc.rust-lang.org/nomicon/phantom-data.html)
- Nomicon: [Other Reprs](https://doc.rust-lang.org/nomicon/other-reprs.html)
- Nomicon: [FFI](https://doc.rust-lang.org/nomicon/ffi.html)
