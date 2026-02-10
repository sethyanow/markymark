## Rust Syntax Quick Reference

> **TL;DR:** Concise syntax cheatsheet for Rust 2021. Look up specific syntax constructs.

### Variable Bindings

```rust
let x = 5;                    // Immutable binding
let mut y = 10;                // Mutable binding
let (a, b) = (1, 2);          // Destructuring
let _unused = compute();       // Underscore suppresses unused warning
const MAX: u32 = 100;         // Compile-time constant
static COUNTER: AtomicU64 = AtomicU64::new(0);  // Static with 'static lifetime
```

### Functions

```rust
fn add(x: i32, y: i32) -> i32 { x + y }
fn no_return() { /* returns () */ }
fn diverges() -> ! { panic!("never returns") }
async fn fetch() -> Result<Data, Error> { Ok(data) }
const fn compile_time() -> u32 { 42 }
unsafe fn dangerous() { /* caller ensures safety */ }
```

### Closures

```rust
let add = |x, y| x + y;                    // Inferred types
let add: fn(i32, i32) -> i32 = |x, y| x + y;  // Explicit
let print = |s: &str| { println!("{s}"); }; // Block body
let own = move |x| x + captured_value;     // Takes ownership
```

### Control Flow

```rust
// if/else
if condition { a } else if other { b } else { c }

// if let
if let Some(x) = optional { use(x); }

// let-else (Rust 1.65+)
let Some(x) = optional else { return; };

// while let
while let Some(item) = iter.next() { process(item); }

// loop
let result = loop { if done { break value; } };

// for
for item in collection { }
for (i, item) in collection.iter().enumerate() { }
for i in 0..10 { }    // 0 to 9
for i in 0..=10 { }   // 0 to 10

// match
match value {
    0 => "zero",
    1..=9 => "digit",
    n if n < 0 => "negative",
    _ => "other",
}
```

### Pattern Matching Syntax

```rust
// Binding
let Point { x, y } = point;                     // Struct destructure
let (first, .., last) = tuple;                   // Tuple with rest
let [first, ref rest @ ..] = slice;              // Slice pattern
Some(ref x) => { /* borrow, don't move */ }
value @ 1..=5 => { /* bind AND match range */ }
Enum::A | Enum::B => { /* or-pattern */ }
```

### Traits & Generics Syntax

```rust
// Trait definition
trait Name: SuperTrait { fn method(&self) -> Type; }

// Implementations
impl Type { fn method(&self) {} }
impl Trait for Type { fn method(&self) {} }

// Generic bounds
fn f<T: Clone + Debug>(x: T) {}
fn f<T>(x: T) where T: Clone + Debug {}
fn f(x: impl Clone + Debug) {}

// Associated types
trait Container { type Item; fn get(&self) -> &Self::Item; }

// Turbofish
let x = "42".parse::<i32>()?;
let v = Vec::<u8>::new();
```

### Lifetime Syntax

```rust
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str { if x.len() > y.len() { x } else { y } }
struct Ref<'a, T: 'a> { data: &'a T }
impl<'a> Ref<'a, str> { fn len(&self) -> usize { self.data.len() } }
```

### Attribute Syntax

```rust
#[derive(Debug, Clone)]             // Derive traits
#[cfg(test)]                        // Conditional compilation
#[cfg(feature = "json")]            // Feature gate
#[allow(unused)]                    // Suppress warning
#[expect(clippy::needless_pass_by_value, reason = "API contract")]
#[must_use]                         // Warn if return value unused
#[non_exhaustive]                   // Prevent external exhaustive matching
#[repr(C)]                          // C-compatible layout
#[inline]                           // Inline hint
#[doc(hidden)]                      // Hide from docs
#![deny(unsafe_code)]               // Crate-level: forbid unsafe
```

### Smart Pointer Syntax

```rust
Box::new(value)                     // Heap allocation
Rc::new(value)                      // Reference counted (single-thread)
Arc::new(value)                     // Atomic reference counted (multi-thread)
Rc::clone(&rc)                      // Explicit clone (convention)
Cell::new(value)                    // Interior mutability (Copy)
RefCell::new(value)                 // Interior mutability (runtime checks)
Mutex::new(value)                   // Thread-safe interior mutability
```

### References

- Rust Reference: [Language Reference](https://doc.rust-lang.org/reference/)
- Rust by Example: [rust-by-example.com](https://doc.rust-lang.org/rust-by-example/)
