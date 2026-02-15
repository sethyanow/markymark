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
| 4 | Wrong Fn trait bound on closure | 🟠 | [closures](core/closures.md) | FnOnce for one-shot, FnMut for repeated, Fn for shared |
| 5 | Type is !Send due to transitive field | 🟠 | [concurrency](advanced/concurrency.md) | Trace field chain; use Arc instead of Rc, owned instead of &T |
| 6 | Defaulting to Ordering::Relaxed | 🟠 | [concurrency](advanced/concurrency.md) | Default to SeqCst; downgrade with proof |
| 7 | Ignoring pinning in async code | 🟠 | [async](advanced/async.md) | Use `Box::pin()` or `pin!()` macro |
| 8 | Ignoring cancellation safety in select! | 🟠 | [async](advanced/async.md) | Scope partial state; use cancellation-safe APIs |
| 9 | Fighting borrow checker with `.clone()` | 🟡 | [ownership](core/ownership.md) | Restructure or use `mem::take`/`mem::replace` |
| 10 | Using `unwrap()` in library code | 🟡 | [errors](core/errors.md) | Use `?` propagation with proper errors |
| 11 | Leaking external crate types in API | 🟡 | [api-design](patterns/api-design.md) | Wrap in newtypes |
| 12 | Glob imports in libraries | 🟡 | [modules](core/modules.md) | Explicit `pub use` with `#[doc(inline)]` |
| 13 | Non-descriptive error types | 🟡 | [errors](core/errors.md) | Use `thiserror` with descriptive variants |
| 14 | Trusting pre-training over crate docs | 🟠 | [anti-patterns](patterns/anti-patterns.md) | Read `cargo doc` output before implementing |
| 15 | Cloning arena-backed types (SIGSEGV) | 🔴 | [anti-patterns](patterns/anti-patterns.md) | Return references; convert to owned types |
| 16 | Returning `&[]` as arena-lifetime slice | 🔴 | [anti-patterns](patterns/anti-patterns.md) | Allocate empty slice in arena |
| 17 | RefCell double borrow panic | 🟠 | [ownership](core/ownership.md) | Drop `Ref`/`RefMut` guard before re-borrowing |
| 18 | Assuming reverse drop order for struct fields | 🟡 | [idioms](patterns/idioms.md) | Fields drop in declaration order (not reverse) |

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
