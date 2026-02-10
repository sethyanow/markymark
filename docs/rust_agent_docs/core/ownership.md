## Ownership, Borrowing & Lifetimes

> **TL;DR:** Every value has one owner; values are moved by default; borrowing grants
> temporary access via `&T` (shared) or `&mut T` (exclusive); lifetimes ensure references
> don't outlive their data; smart pointers extend ownership patterns.

### The Three Ownership Rules

1. **Each value has exactly one owner** — a variable binding.
2. **When the owner goes out of scope, the value is dropped** (destructor runs, memory freed).
3. **Ownership can be transferred (moved)** — the old binding becomes invalid.

```rust
let s1 = String::from("hello");
let s2 = s1;           // s1 is MOVED to s2; s1 is now invalid
// println!("{s1}");    // ❌ compile error: value used after move
println!("{s2}");       // ✅ s2 owns the string
```

### Move vs Copy Decision Tree

```
Does the type implement Copy?
├─ YES (integers, floats, bool, char, tuples/arrays of Copy types)
│   └─ Assignment copies bitwise — both bindings valid
└─ NO (String, Vec, Box, any type with Drop, or non-Copy fields)
    └─ Assignment moves — old binding invalidated
```

**Rule of thumb:** If a type manages a heap resource or has a `Drop` impl, it is `!Copy`.
You can derive `Copy` only if all fields are `Copy` and the type has no `Drop` impl.

### Borrowing Rules

- You may have **either** one `&mut T` (exclusive/mutable) **or** any number of `&T` (shared/immutable) — **never both simultaneously**.
- References must always be **valid** (no dangling refs).
- The borrow checker enforces these at compile time.

```rust
let mut data = vec![1, 2, 3];
let first = &data[0];      // shared borrow
// data.push(4);            // ❌ can't mutate while shared borrow active
println!("{first}");        // last use of shared borrow
data.push(4);               // ✅ shared borrow no longer active (NLL)
```

### Lifetime Elision Rules

The compiler infers lifetimes automatically using these rules (in order):

1. **Each input reference gets its own lifetime parameter:** `fn f(x: &T, y: &U)` → `fn f<'a, 'b>(x: &'a T, y: &'b U)`
2. **If there is exactly one input lifetime, it is assigned to all outputs:** `fn f(x: &T) -> &U` → `fn f<'a>(x: &'a T) -> &'a U`
3. **If one input is `&self` or `&mut self`, its lifetime is assigned to all outputs.**

If these rules don't resolve all output lifetimes, you must annotate explicitly.

### Smart Pointer Selection Decision Tree

```
Do you need shared ownership?
├─ NO
│   Need heap allocation?
│   ├─ YES → Box<T>
│   └─ NO → just use T (stack)
└─ YES
    Is it single-threaded?
    ├─ YES → Rc<T>
    │   Need interior mutability?
    │   ├─ YES → Rc<RefCell<T>>
    │   └─ NO → Rc<T>
    └─ NO (multi-threaded) → Arc<T>
        Need interior mutability?
        ├─ YES, simple writes → Arc<Mutex<T>>
        ├─ YES, read-heavy → Arc<RwLock<T>>
        └─ NO → Arc<T>

Need interior mutability WITHOUT shared ownership?
├─ Single-threaded, Copy types → Cell<T>
├─ Single-threaded, non-Copy → RefCell<T> (runtime borrow checks)
├─ Multi-threaded → Mutex<T> or RwLock<T>
└─ Lock-free atomic → AtomicT (for primitives only)
```

### Common Ownership Errors

| Error Message | Cause | Fix |
|--------------|-------|-----|
| "value moved here" | Used after move | Clone, borrow, or restructure |
| "cannot borrow as mutable" | Shared borrow active | Reduce shared borrow scope |
| "does not live long enough" | Reference outlives data | Extend data lifetime or clone |
| "cannot move out of borrowed" | Moving from behind `&` | Clone or use `std::mem::take` |

> ⚠️ **COMMON MISTAKE: Fighting the borrow checker with `.clone()`**
> Excessive cloning is a code smell. If you find yourself cloning to appease the
> borrow checker, restructure the algorithm: split structs, reduce borrow scopes,
> or use interior mutability patterns like `RefCell` or `Mutex`.

### Interior Mutability Patterns

| Type | Thread-Safe | Checked | Use When |
|------|-------------|---------|----------|
| `Cell<T>` | No | No (Copy only) | Simple single-thread mutation of Copy types |
| `RefCell<T>` | No | Runtime | Single-thread mutation with borrow tracking |
| `Mutex<T>` | Yes | Runtime (lock) | Multi-thread exclusive access |
| `RwLock<T>` | Yes | Runtime (lock) | Multi-thread read-heavy access |
| `AtomicT` | Yes | Lock-free | Primitive counters, flags |

### References

- The Rust Book: [Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- The Rust Book: [Smart Pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html)
- Nomicon: [Ownership](https://doc.rust-lang.org/nomicon/ownership.html)
- Guidelines: [ai.md](../../docs/rust_guidelines/ai.md)
- Related: [core/traits.md](traits.md) (Deref, Drop), [advanced/concurrency.md](../advanced/concurrency.md) (Arc, Mutex)
