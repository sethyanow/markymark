## Closures & Fn Traits

> **TL;DR:** Closures capture their environment. The compiler infers which `Fn` trait a closure
> implements based on how it uses captured values. Use `Fn` for read-only, `FnMut` for mutation,
> `FnOnce` for consuming. Use `move` to transfer ownership into the closure.

### Fn Trait Hierarchy

Every closure implements one or more of these traits. They form a **subtyping hierarchy**:

```
FnOnce     ← All closures implement this (can be called at least once)
  ↑
FnMut      ← Closures that don't consume captures (can be called multiple times)
  ↑
Fn         ← Closures that don't mutate captures (can be called concurrently)
```

`Fn` is a subtrait of `FnMut`, which is a subtrait of `FnOnce`. A closure that implements
`Fn` also implements `FnMut` and `FnOnce`.

| Trait | Captures | Can Call | Use When |
|-------|----------|---------|----------|
| `FnOnce` | May consume (move out) values | Once | Callback used exactly once |
| `FnMut` | May mutate captured values | Multiple times | Iterators, repeated callbacks |
| `Fn` | Read-only or no captures | Multiple times, concurrently | Event handlers, shared callbacks |

### Capture Semantics

The compiler chooses the **least restrictive** capture mode that works:

```
Does the closure body...
├─ Move a captured value OUT of the closure (e.g., return it, push to vec)?
│   └─ Captures by move → implements only FnOnce
├─ Mutate a captured value (e.g., counter += 1)?
│   └─ Captures by &mut → implements FnMut (+ FnOnce)
├─ Only read captured values?
│   └─ Captures by & → implements Fn (+ FnMut + FnOnce)
└─ Capture nothing?
    └─ Implements Fn (+ FnMut + FnOnce)
```

```rust
let name = String::from("Alice");
let greeting = String::from("Hello");

// Fn: only reads `name`
let print_name = || println!("{name}");

// FnMut: mutates `count`
let mut count = 0;
let mut increment = || { count += 1; };

// FnOnce: moves `greeting` out
let consume = || { drop(greeting); };
// greeting is no longer available here
```

### The `move` Keyword

`move` forces all captured variables to be moved into the closure, even if the closure body
only reads them. This is essential for closures that outlive their creating scope:

```rust
use std::thread;

let data = vec![1, 2, 3];

// ❌ DON'T: closure borrows `data`, but thread may outlive current scope
// thread::spawn(|| println!("{data:?}"));

// ✅ DO: `move` transfers ownership to the thread
thread::spawn(move || {
    println!("{data:?}");
});
// `data` is no longer available here
```

**Key insight:** `move` affects HOW values are captured (by value instead of by reference),
not WHICH `Fn` trait the closure implements. A `move` closure that only reads its captures
still implements `Fn`.

### Which Fn Trait to Use as a Bound?

```
How will you call the closure?
├─ Exactly once (consuming callback, one-shot handler)?
│   └─ FnOnce — most flexible, accepts all closures
├─ Multiple times, closure may need to mutate state?
│   └─ FnMut — accepts Fn and FnMut closures
├─ Multiple times, possibly concurrently, no mutation?
│   └─ Fn — most restrictive, guarantees no side effects
└─ Unsure?
    └─ Start with FnOnce (accepts everything); tighten if needed
```

```rust
// Accept any closure (most flexible)
fn run_once<F: FnOnce() -> String>(f: F) -> String { f() }

// Accept closures that can be called repeatedly
fn run_many<F: FnMut() -> i32>(mut f: F) -> i32 { f() + f() }

// Accept closures safe to call concurrently
fn run_shared<F: Fn() -> bool>(f: F) -> bool { f() && f() }
```

### Closure vs Function Pointer

```
Can the value be a plain function (no captures)?
├─ YES → fn(Args) -> Ret (function pointer)
│   └─ Lighter, implements Fn + FnMut + FnOnce
│   └─ Can use named functions: vec.sort_by(i32::cmp)
└─ NO (needs to capture environment)
    └─ impl Fn/FnMut/FnOnce or Box<dyn Fn/FnMut/FnOnce>
```

```rust
// Function pointer — no captures, lightest
fn apply(f: fn(i32) -> i32, x: i32) -> i32 { f(x) }

// Generic closure — zero-cost, monomorphized
fn apply_generic(f: impl Fn(i32) -> i32, x: i32) -> i32 { f(x) }

// Boxed closure — dynamic dispatch, needed for heterogeneous collections
fn apply_boxed(f: Box<dyn Fn(i32) -> i32>, x: i32) -> i32 { f(x) }
```

### Returning Closures

Closures have anonymous types, so you can't name them directly:

```rust
// ✅ Return with impl Trait (single concrete type)
fn make_adder(x: i32) -> impl Fn(i32) -> i32 {
    move |y| x + y
}

// ✅ Return boxed (when you need dynamic dispatch or multiple return paths)
fn make_handler(kind: &str) -> Box<dyn Fn(i32) -> i32> {
    match kind {
        "double" => Box::new(|x| x * 2),
        _ => Box::new(|x| x),
    }
}
```

### Common Closure Errors

| Error | Cause | Fix |
|-------|-------|-----|
| "closure may outlive the current function" | Closure borrows local data, given to thread/async | Add `move` keyword |
| "expected `FnMut`, found closure that implements `FnOnce`" | Closure moves captured value out | Don't consume captures; clone if needed |
| "closure is `FnOnce` because it moves variable" | Value moved out of closure body | Use `&` or `clone()` instead of moving |
| "cannot borrow as mutable in `Fn` closure" | Mutating capture in `Fn` context | Change bound to `FnMut`, or use `Cell`/`Mutex` |
| "borrowed value does not live long enough" | Closure captures ref to local | Use `move` or restructure lifetimes |

### Closures in Common Patterns

```rust
// Iterator adapters — most use FnMut
let squares: Vec<i32> = (1..=5).map(|x| x * x).collect();

// Option/Result combinators — use FnOnce
let value = some_option.unwrap_or_else(|| expensive_default());

// Sorting — uses FnMut (called multiple times)
items.sort_by(|a, b| a.name.cmp(&b.name));

// Thread spawning — requires move + Send + 'static
std::thread::spawn(move || { /* ... */ });

// Async tasks — requires move + Send + 'static
tokio::spawn(async move { /* ... */ });
```

### References

- The Rust Book: [Closures](https://doc.rust-lang.org/book/ch13-01-closures.html)
- Related: [core/traits.md](traits.md) (trait bounds), [core/ownership.md](ownership.md) (move semantics)
- Related: [advanced/concurrency.md](../advanced/concurrency.md) (move closures for threads)
