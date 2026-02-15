## Unsafe Rust — Superpowers, Invariants & PhantomData

> **TL;DR:** `unsafe` unlocks 5 superpowers but shifts responsibility to you. Every `unsafe`
> block needs a `// SAFETY:` comment. Get PhantomData variance right or your abstraction
> is unsound. Test with Miri.

### The 5 Unsafe Superpowers

1. **Dereference raw pointers** (`*const T`, `*mut T`)
2. **Call unsafe functions or methods**
3. **Access or modify mutable statics**
4. **Implement unsafe traits** (`Send`, `Sync`, `GlobalAlloc`)
5. **Access fields of `union`s**

Everything else is still checked by the compiler inside `unsafe` blocks.

### Safety Comments — The `// SAFETY:` Convention

Every `unsafe` block must have a comment explaining why the unsafe operation is sound:

```rust
/// Returns bytes as a UTF-8 string without validation.
///
/// # Safety
/// `bytes` must contain valid UTF-8.
unsafe fn bytes_to_str(bytes: &[u8]) -> &str {
    // SAFETY: caller guarantees bytes are valid UTF-8
    std::str::from_utf8_unchecked(bytes)
}
```

### Valid Reasons for `unsafe`

| Reason | Example | Requirement |
|--------|---------|-------------|
| Novel abstraction | Custom smart pointer, allocator | No safe alternative exists |
| Performance | `.get_unchecked()`, `transmute` | Benchmarked; safe version too slow |
| FFI/platform calls | `extern "C"` functions | No safe wrapper available |

❌ **Invalid reasons:** Bypassing borrow checker, avoiding lifetimes, shortening code.

### PhantomData Variance Table

This table **must match exactly** when designing unsafe abstractions. Getting variance wrong
makes your abstraction **unsound**.

> ⚠️ **CRITICAL MISTAKE: Wrong PhantomData variance**
> Using the wrong PhantomData type gives your generic type the wrong variance, allowing
> the compiler to accept code that is actually unsound.

| PhantomData variant | variance of `'a` | variance of `T` | Send/Sync |
|---------------------|:-----------------:|:----------------:|-----------|
| `PhantomData<T>` | - | **cov**ariant | inherited |
| `PhantomData<&'a T>` | **cov**ariant | **cov**ariant | `Send + Sync` requires `T: Sync` |
| `PhantomData<&'a mut T>` | **cov**ariant | **inv**ariant | inherited |
| `PhantomData<*const T>` | - | **cov**ariant | `!Send + !Sync` |
| `PhantomData<*mut T>` | - | **inv**ariant | `!Send + !Sync` |
| `PhantomData<fn(T)>` | - | **contra**variant | `Send + Sync` |
| `PhantomData<fn() -> T>` | - | **cov**ariant | `Send + Sync` |
| `PhantomData<fn(T) -> T>` | - | **inv**ariant | `Send + Sync` |

**Quick guide:**
- Need **covariance** (most common): `PhantomData<T>` or `PhantomData<fn() -> T>`
- Need **contravariance** (consuming T): `PhantomData<fn(T)>`
- Need **invariance** (mutating T): `PhantomData<fn(T) -> T>` or `PhantomData<*mut T>`

```rust
use std::marker::PhantomData;

// Covariant over T (like Vec<T>)
struct MyVec<T> {
    ptr: *const T,
    len: usize,
    _marker: PhantomData<T>,  // covariant, owns T
}

// Invariant over T (mutable access to T)
struct MutRef<'a, T> {
    ptr: *mut T,
    _marker: PhantomData<&'a mut T>,  // invariant over T, covariant over 'a
}
```

### Variance Explained

Variance determines how subtyping of type parameters relates to subtyping of the
containing type. This matters for unsafe abstractions and generic library design.

| Variance | Rule | Example |
|----------|------|---------|
| **Covariant** | If `'long: 'short`, then `Container<'long>` can be used where `Container<'short>` expected | `&'a T`, `Vec<T>`, `Box<T>` |
| **Contravariant** | Reversed: `Container<'short>` usable where `Container<'long>` expected | `fn(T)` (function arguments) |
| **Invariant** | No subtyping — must match exactly | `&'a mut T` (over T), `Cell<T>`, `UnsafeCell<T>` |

**Why it matters:** Getting variance wrong in unsafe code lets the compiler accept
programs that produce dangling references or data races.

```rust
// Covariant: Vec<&'long str> can become Vec<&'short str> (safe, shorter lifetime is weaker)
fn covariant_demo<'long, 'short>(v: Vec<&'long str>) -> Vec<&'short str>
where 'long: 'short
{
    v  // compiles: covariant over 'long
}

// Invariant: &mut T is invariant over T — can't substitute subtypes
fn invariant_demo(x: &mut &'static str) {
    // Can't pass to fn expecting &mut &'a str for some shorter 'a
    // because caller could write a shorter-lived &str through the &mut
}
```

**Designing unsafe abstractions:**
1. Ask "does my type read T, write T, or both?"
2. Read-only → covariant (`PhantomData<T>`)
3. Write-only → contravariant (`PhantomData<fn(T)>`) — rare
4. Read+write → invariant (`PhantomData<fn(T) -> T>` or `PhantomData<*mut T>`)
5. When in doubt, **choose invariant** — it's always safe, just less flexible

### PhantomData in Practice

The variance table tells you *which* `PhantomData` to pick. These examples show *why* real
data structures need it and how the drop checker interacts.

**Owning raw pointer (Vec-like):**
```rust
use std::marker::PhantomData;

struct RawVec<T> {
    ptr: *mut T,         // raw pointer: no ownership semantics
    cap: usize,
    _marker: PhantomData<T>,  // tells compiler: "I own T values"
    // Effect: enables drop check — compiler knows dropping RawVec
    // may drop T values, so T must outlive RawVec.
}

// Without PhantomData<T>, the compiler wouldn't know RawVec<T>
// logically owns T values, and drop check would be unsound.
unsafe impl<T: Send> Send for RawVec<T> {}
unsafe impl<T: Sync> Sync for RawVec<T> {}
```

**Shared ownership (Arc-like):**
```rust
struct MyArc<T> {
    ptr: *const ArcInner<T>,
    _marker: PhantomData<ArcInner<T>>,  // owns the ArcInner
    // PhantomData<T> would also work here — the key point is
    // establishing ownership for the drop checker.
}

struct ArcInner<T> {
    ref_count: AtomicUsize,
    data: T,
}
```

**Lifetime token (no data):**
```rust
/// Borrow guard — holds no data but represents a logical borrow.
struct BorrowGuard<'a> {
    _marker: PhantomData<&'a ()>,  // covariant over 'a
    // Makes BorrowGuard<'long> usable where BorrowGuard<'short> expected
}

/// Exclusive borrow guard — invariant over the lifetime.
struct MutBorrowGuard<'a> {
    _marker: PhantomData<&'a mut ()>,  // invariant over lifetime
    // Prevents the compiler from shortening or extending the borrow
}
```

**Drop checker interaction (RFC 1238):**

The drop checker ensures that when a generic type is dropped, all type/lifetime
parameters are still valid. `PhantomData<T>` opts into this check:

```rust
// This compiles — Vec<&'a str> needs 'a valid at drop
{
    let v: Vec<&str>;
    let s = String::from("hello");
    v = vec![&s];  // OK: s outlives v (dropped in reverse declaration order)
}

// This would NOT compile:
// {
//     let s = String::from("hello");
//     let v = vec![&s];  // ERROR: s dropped before v
// }
```

Without `PhantomData`, custom types using raw pointers would bypass this check,
potentially accessing freed memory in their `Drop` impl.

### Undefined Behavior Catalog

These are **always UB** in Rust — no exceptions:

- Dereferencing null or dangling pointers
- Reading uninitialized memory
- Breaking aliasing rules (two `&mut` to same data, `&mut` + `&` to same data)
- Creating unaligned references (including to packed struct fields)
- Producing invalid values (e.g., `bool` that isn't 0 or 1)
- Data races (concurrent unsynchronized access where at least one is a write)
- Unwinding into C code (panic across `extern "C"` boundary)
- Violating the preconditions of `unsafe` functions

### Miri — UB Detection Tool

```bash
# Install Miri
rustup +nightly component add miri

# Run tests under Miri
cargo +nightly miri test

# Run a specific binary
cargo +nightly miri run
```

Miri detects: out-of-bounds access, use-after-free, invalid alignment, data races (with `-Zmiri-data-race`), memory leaks (`-Zmiri-leak-check`).

**Limitation:** Miri can only find UB that is actually executed. It does not prove absence of UB.

### Sound Abstraction Design

1. **Minimize unsafe surface** — keep `unsafe` blocks as small as possible
2. **Encapsulate invariants** — unsafe internals behind safe public API
3. **Document preconditions** — `# Safety` section on all `unsafe fn`
4. **Test with Miri** — catch UB in tests
5. **Consider adversarial code** — misbehaving `Drop`, `Clone`, `Deref` impls
6. **Soundness boundary = module boundary** — safe functions in the same module may rely on shared invariants

### References

- Nomicon: [Meet Safe and Unsafe](https://doc.rust-lang.org/nomicon/meet-safe-and-unsafe.html)
- Nomicon: [PhantomData](https://doc.rust-lang.org/nomicon/phantom-data.html)
- Unsafe Code Guidelines: [Repository](https://rust-lang.github.io/unsafe-code-guidelines/)
- Guidelines: [M-UNSAFE](../../docs/rust_guidelines/safety.md), [M-UNSOUND](../../docs/rust_guidelines/safety.md)
- Related: [advanced/type-layout.md](type-layout.md), [advanced/ffi.md](ffi.md), [checklists/unsafe-review.md](../checklists/unsafe-review.md)
