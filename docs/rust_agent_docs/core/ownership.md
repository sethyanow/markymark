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

#### RefCell in Practice

`RefCell<T>` enforces the same borrow rules as the compiler, but at **runtime**.
If you violate the rules, the program **panics** instead of getting a compiler error.

```rust
use std::cell::RefCell;

let data = RefCell::new(vec![1, 2, 3]);

// borrow() → shared reference (like &T)
let r = data.borrow();
println!("len: {}", r.len());
drop(r); // Must drop before borrowing mutably

// borrow_mut() → exclusive reference (like &mut T)
data.borrow_mut().push(4);

// ❌ PANICS: two mutable borrows at once
// let a = data.borrow_mut();
// let b = data.borrow_mut(); // panic: already mutably borrowed
```

**Common pattern — `Rc<RefCell<T>>` for shared mutable state:**

```rust
use std::cell::RefCell;
use std::rc::Rc;

let shared = Rc::new(RefCell::new(0));
let clone1 = Rc::clone(&shared);

*clone1.borrow_mut() += 1;
assert_eq!(*shared.borrow(), 1);
```

**Multi-threaded equivalent:** Replace `Rc<RefCell<T>>` with `Arc<Mutex<T>>`.

> ⚠️ **Agent pitfall:** `borrow()` and `borrow_mut()` are **not** compile-time checked.
> If you hold a `Ref` or `RefMut` guard across a code path that calls `borrow_mut()` again,
> the program panics at runtime. Always drop guards before re-borrowing.

### Advanced Lifetimes

#### Lifetime Subtyping (`'a: 'b`)

`'a: 'b` means `'a` outlives `'b`. Use when a reference must live at least as long as another:

```rust
fn longest_with_announcement<'a, 'b>(x: &'a str, y: &'a str, ann: &'b str) -> &'a str
where
    'a: 'b,  // 'a lives at least as long as 'b
{
    println!("Announcement: {ann}");
    if x.len() > y.len() { x } else { y }
}
```

#### Higher-Ranked Trait Bounds (HRTB)

`for<'a>` means "for any lifetime." Use when accepting callbacks that must work with
any borrow lifetime:

```rust
// ❌ DON'T: Can't name the lifetime of the closure's argument
// fn apply(f: impl Fn(&str) -> &str) { ... }

// ✅ DO: for<'a> means the closure works for any lifetime
fn apply(f: impl for<'a> Fn(&'a str) -> &'a str) {
    let owned = String::from("hello");
    let result = f(&owned);
    println!("{result}");
}
```

Most of the time, the compiler inserts `for<'a>` automatically. You only need it explicitly
in trait bounds on struct fields or type aliases.

#### Self-Referential Structs

Rust does **not** natively support structs that borrow from their own fields:

```rust
// ❌ IMPOSSIBLE: Can't borrow from yourself
// struct SelfRef {
//     data: String,
//     slice: &str,  // can't reference data
// }
```

**Solutions (from most to least preferred):**
1. **Compute on access** — store indices/offsets instead of references
2. **Use `Pin` + unsafe** — only if you really need it
3. **Use `ouroboros` or `self_cell` crate** — safe wrappers for self-referential patterns
4. **Restructure** — split into two types with explicit lifetime relationship

### Borrow Splitting

The borrow checker understands **disjoint struct field borrows** — you can mutably borrow
different fields simultaneously:

```rust
struct State {
    buffer: Vec<u8>,
    position: usize,
}

fn process(state: &mut State) {
    // ✅ OK: borrowing different fields
    let buf = &mut state.buffer;
    let pos = &state.position;
    buf.resize(*pos, 0);
}
```

However, the borrow checker does **NOT** understand array/slice index disjointness:

```rust
let mut arr = [1, 2, 3];
// ❌ FAILS: compiler can't prove arr[0] and arr[1] don't overlap
// let a = &mut arr[0];
// let b = &mut arr[1];

// ✅ DO: Use split_at_mut for disjoint slice borrows
let (left, right) = arr.split_at_mut(1);
let a = &mut left[0];
let b = &mut right[0];
```

### Ownership Manipulation: mem::take, mem::replace, mem::swap

These functions are essential tools for working around borrow checker limitations
without cloning:

| Function | What It Does | Use When |
|----------|-------------|----------|
| `mem::take(&mut val)` | Replaces val with `Default::default()`, returns old val | Moving out of `&mut` reference |
| `mem::replace(&mut val, new)` | Replaces val with `new`, returns old val | Swapping in a sentinel/placeholder |
| `mem::swap(&mut a, &mut b)` | Swaps values in place | Rearranging without temp variable |

```rust
use std::mem;

struct Node {
    value: String,
    children: Vec<Node>,
}

fn take_children(node: &mut Node) -> Vec<Node> {
    // Can't move `node.children` out of &mut Node...
    // but mem::take replaces it with empty Vec and returns the old one
    mem::take(&mut node.children)
}

fn replace_value(node: &mut Node, new_val: String) -> String {
    // Replace and get old value in one step
    mem::replace(&mut node.value, new_val)
}
```

**When to use instead of `.clone()`:**
- `mem::take` when you need the old value AND the field has a sensible default
- `mem::replace` when you need the old value AND want to insert a specific new value
- These are O(1) operations vs `.clone()` which may be O(n)

### References

- The Rust Book: [Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- The Rust Book: [Smart Pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html)
- Nomicon: [Ownership](https://doc.rust-lang.org/nomicon/ownership.html), [Splitting Borrows](https://doc.rust-lang.org/nomicon/borrow-splitting.html)
- Guidelines: [ai.md](../../docs/rust_guidelines/ai.md)
- Related: [core/traits.md](traits.md) (Deref, Drop), [core/closures.md](closures.md) (move semantics), [advanced/concurrency.md](../advanced/concurrency.md) (Arc, Mutex)
