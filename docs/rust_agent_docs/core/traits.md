## Traits — Polymorphism & Standard Library Traits

> **TL;DR:** Traits define shared behavior. Prefer static dispatch (`impl Trait` / generics)
> for performance; use dynamic dispatch (`dyn Trait`) for heterogeneous collections.
> Know the standard traits—they're the vocabulary of idiomatic Rust.

### Trait Basics

```rust
trait Summary {
    fn summarize(&self) -> String;

    // Default method — can be overridden
    fn preview(&self) -> String {
        format!("{}...", &self.summarize()[..20.min(self.summarize().len())])
    }
}

impl Summary for Article {
    fn summarize(&self) -> String {
        format!("{}: {}", self.author, self.title)
    }
}
```

### Static vs Dynamic Dispatch

| Aspect | Static (`impl Trait` / generics) | Dynamic (`dyn Trait`) |
|--------|----------------------------------|-----------------------|
| Speed | Monomorphized, inlined | vtable indirection |
| Binary size | Larger (code per type) | Smaller |
| Heterogeneous collections | ❌ No | ✅ Yes |
| Object safety required | No | Yes |
| Use when | Performance matters, types known | Type erasure needed |

```rust
// Static dispatch — monomorphized per type
fn notify(item: &impl Summary) { println!("{}", item.summarize()); }

// Dynamic dispatch — single code path, vtable lookup
fn notify_dyn(item: &dyn Summary) { println!("{}", item.summarize()); }
```

### Object Safety Rules

A trait is object-safe (can be used as `dyn Trait`) only if:

1. **No `Self: Sized` bound** on the trait itself
2. **All methods** must either:
   - Have a receiver (`self`, `&self`, `&mut self`, `Box<Self>`, etc.)
   - OR be explicitly excluded with `where Self: Sized`
3. **No generic type parameters** on methods (lifetime params OK)
4. **No associated functions** without a receiver (no `fn new() -> Self`)
5. **Return type is not `Self`** (unless bounded with `where Self: Sized`)

### Standard Library Traits Catalog

#### Formatting & Display
| Trait | Purpose | How to Get |
|-------|---------|-----------|
| `Debug` | Developer-facing formatting (`{:?}`) | `#[derive(Debug)]` |
| `Display` | User-facing formatting (`{}`) | Manual `impl` |

#### Comparison & Hashing
| Trait | Purpose | Notes |
|-------|---------|-------|
| `PartialEq` / `Eq` | Equality; `Eq` adds reflexivity | `Eq` required for `HashMap` keys |
| `PartialOrd` / `Ord` | Ordering; `Ord` is total | `Ord` required for `BTreeMap` keys |
| `Hash` | Hash computation | Must agree with `Eq` |

#### Cloning & Copying
| Trait | Purpose | Notes |
|-------|---------|-------|
| `Clone` | Explicit deep copy | `.clone()` |
| `Copy` | Implicit bitwise copy | Marker trait; requires `Clone`; no `Drop` |

#### Conversion Traits Decision Tree
```
Converting between types?
├─ Cheap reference conversion (no allocation)?
│   ├─ Immutable → AsRef<T>
│   └─ Mutable → AsMut<T>
├─ Owned type conversion?
│   ├─ Infallible → From<T> / Into<T>
│   │   └─ Implement From<T> (Into is auto-derived)
│   └─ Fallible → TryFrom<T> / TryInto<T>
├─ Borrowing with possible ownership? → Borrow<T> / ToOwned
│   └─ Example: str → String, [T] → Vec<T>
└─ Smart pointer-like coercion? → Deref / DerefMut
```

#### Iteration
| Trait | Method | Yields |
|-------|--------|--------|
| `Iterator` | `next(&mut self) -> Option<Item>` | Owned items |
| `IntoIterator` | `into_iter(self) -> Iterator` | Enables `for x in collection` |

#### Resource Management
| Trait | Purpose | Notes |
|-------|---------|-------|
| `Default` | Provide default values | `#[derive(Default)]` or manual |
| `Drop` | Custom cleanup | Cannot call explicitly; use `std::mem::drop()` |

#### Thread Safety (marker traits)
| Trait | Meaning |
|-------|---------|
| `Send` | Safe to transfer to another thread |
| `Sync` | Safe to share references between threads (`&T` is `Send`) |

### Orphan Rules

You can only implement a trait for a type if **at least one of** (trait, type) is defined in your crate.

**Workaround:** Use the newtype pattern to wrap external types:

```rust
struct MyWrapper(external_crate::TheirType);

impl MyTrait for MyWrapper {
    // Now legal — MyWrapper is yours
}
```

### Extension Traits

Add methods to foreign types via a new trait:

```rust
trait StringExt {
    fn truncate_to(&self, max_len: usize) -> &str;
}

impl StringExt for str {
    fn truncate_to(&self, max_len: usize) -> &str {
        if self.len() <= max_len { self }
        else { &self[..self.floor_char_boundary(max_len)] }
    }
}
```

### Sealed Traits

Prevent external implementations:

```rust
mod private { pub trait Sealed {} }

pub trait MyTrait: private::Sealed {
    fn method(&self);
}

// Only types you impl Sealed for can implement MyTrait
impl private::Sealed for MyPublicType {}
impl MyTrait for MyPublicType { fn method(&self) {} }
```

### References

- The Rust Book: [Traits](https://doc.rust-lang.org/book/ch10-02-traits.html)
- Rust API Guidelines: [Traits](https://rust-lang.github.io/api-guidelines/interoperability.html)
- Related: [core/types.md](types.md) (generics), [patterns/api-design.md](../patterns/api-design.md) (public API traits)
