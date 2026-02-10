## Type Layout — Memory Representation & Repr Attributes

> **TL;DR:** Rust's default layout (`repr(Rust)`) is unstable and non-deterministic. Use
> `repr(C)` for FFI, `repr(transparent)` for newtypes, and **never take references to
> `repr(packed)` fields** — it's undefined behavior.

### repr(Rust) — The Default

- Compiler may reorder fields, add padding for alignment, optimize size
- Layout is **not guaranteed stable** between compilations
- Cannot be relied upon for FFI or serialization
- Sufficient for pure-Rust code with no cross-boundary needs

### repr(C) — C-Compatible Layout

- Fields ordered as declared, with C-standard padding/alignment rules
- **Required** for any struct passed across FFI boundaries
- Enables `transmute` and pointer casting between compatible types
- Enum variant: equivalent to a C enum (platform-dependent size)

```rust
#[repr(C)]
struct Point {
    x: f64,
    y: f64,
}
```

### repr(transparent) — Zero-Cost Newtypes

- Only on structs with a single non-zero-sized field (plus ZST fields)
- Guarantees identical layout and ABI to the inner field
- Safe to `transmute` between the newtype and its inner type
- Perfect for FFI-safe newtypes

```rust
#[repr(transparent)]
struct Meters(f64);  // Same ABI as f64, can pass to C expecting f64
```

### repr(packed) — Removing Padding

- Forces alignment to 1 byte (or `repr(packed(N))` for alignment ≤ N)
- Reduces memory footprint but **creates alignment hazards**

> ⚠️ **CRITICAL MISTAKE: Taking references to packed struct fields**
> Fields in `#[repr(packed)]` structs may be unaligned. Creating a reference (`&field`)
> to an unaligned field is **undefined behavior** in Rust.

```rust
#[repr(packed)]
struct Packed {
    flag: u8,
    value: u32,  // may be at offset 1 (unaligned for u32)
}

let p = Packed { flag: 1, value: 42 };

// ❌ UB: &p.value creates an unaligned reference
// let v = &p.value;

// ✅ Read by value (compiler emits unaligned read)
let v = p.value;

// ✅ Or use read_unaligned for raw pointers
let v = unsafe { std::ptr::addr_of!(p.value).read_unaligned() };
```

The compiler emits a lint (`unaligned_references`) that will become a hard error.
**Always read packed fields by value, never by reference.**

### repr(align(N)) — Minimum Alignment

- Forces alignment to at least N bytes (N must be a power of 2)
- Useful for cache-line alignment to prevent false sharing

```rust
#[repr(align(64))]
struct CacheAligned {
    counter: std::sync::atomic::AtomicU64,
}
```

### Repr Attribute Selection Guide

| Need | Use | Notes |
|------|-----|-------|
| Default Rust struct | (none) | Compiler optimizes layout |
| Pass to/from C | `repr(C)` | Stable, predictable layout |
| FFI-safe newtype | `repr(transparent)` | Single non-ZST field |
| Minimize memory | `repr(packed)` | ⚠️ Alignment dangers |
| Cache optimization | `repr(align(N))` | False sharing prevention |
| C-like enum | `repr(u8)` / `repr(i32)` etc. | Fixed discriminant size |
| FFI enum | `repr(C)` | Platform C ABI for enums |
| Combined | `repr(C, packed)` | For matching C packed structs |

### Size and Alignment Examples

```rust
use std::mem::{size_of, align_of};

// repr(Rust): compiler may reorder
struct A { x: u8, y: u32, z: u8 }
// Possible size: 8 (fields reordered to: y, x, z + padding)

#[repr(C)]
struct B { x: u8, y: u32, z: u8 }
// Size: 12 (x: 1 + 3 padding + y: 4 + z: 1 + 3 padding)

#[repr(C, packed)]
struct C { x: u8, y: u32, z: u8 }
// Size: 6 (no padding, but y is unaligned!)
```

### References

- Nomicon: [repr(Rust)](https://doc.rust-lang.org/nomicon/repr-rust.html)
- Nomicon: [Alternative Reprs](https://doc.rust-lang.org/nomicon/other-reprs.html)
- Rust Reference: [Type Layout](https://doc.rust-lang.org/reference/type-layout.html)
- Related: [advanced/ffi.md](ffi.md) (FFI types), [advanced/unsafe.md](unsafe.md) (safety invariants)
