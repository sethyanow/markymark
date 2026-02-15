## Types — Primitives, Structs, Enums & Generics

> **TL;DR:** Rust's type system prevents entire categories of bugs at compile time.
> Use structs for product types, enums for sum types, generics for abstraction,
> and newtypes for domain semantics.

### Primitive Types

| Type | Size | Notes |
|------|------|-------|
| `bool` | 1 byte | `true`/`false` |
| `char` | 4 bytes | Unicode scalar value (not a byte) |
| `i8`–`i128`, `u8`–`u128` | 1–16 bytes | Fixed-width integers |
| `isize`, `usize` | pointer-width | Indexing, sizes |
| `f32`, `f64` | 4, 8 bytes | IEEE 754 floats |
| `()` | 0 bytes | Unit type (void equivalent) |
| `!` | 0 bytes | Never type (diverging functions) |

### Struct Patterns

```rust
// Named struct — most common
#[derive(Debug, Clone, PartialEq)]
struct Config {
    host: String,
    port: u16,
    tls: bool,
}

// Tuple struct — lightweight wrappers / newtypes
struct Meters(f64);
struct UserId(u64);

// Unit struct — marker types
struct Production;
```

**Common derive combinations:**

| Use Case | Derives |
|----------|---------|
| Data struct | `Debug, Clone, PartialEq` |
| Config / builder | `Debug, Clone, Default` |
| Hash map key | `Debug, Clone, PartialEq, Eq, Hash` |
| FFI struct | None (manual `repr(C)`) |
| Error type | `Debug` + manual `Display` + `Error` |

### Enums as Algebraic Data Types

```rust
#[derive(Debug)]
enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Point,  // unit variant
}

fn area(shape: &Shape) -> f64 {
    match shape {
        Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
        Shape::Rectangle { width, height } => width * height,
        Shape::Point => 0.0,
    }
}
```

Mark enums `#[non_exhaustive]` in libraries to allow adding variants without breaking callers.

### Generics

```rust
// Type parameters
fn largest<T: PartialOrd>(list: &[T]) -> &T {
    let mut max = &list[0];
    for item in &list[1..] {
        if item > max { max = item; }
    }
    max
}

// Where clauses for complex bounds
fn process<T>(item: T) -> String
where
    T: std::fmt::Display + Clone + Send,
{
    format!("Processing: {item}")
}
```

### Associated Types vs Generic Parameters

```
Does the implementor choose the type?
├─ YES (one impl per concrete type choice)
│   └─ Use associated type: type Output;
│       Example: Iterator::Item, Add::Output
└─ NO (generic over many types simultaneously)
    └─ Use generic parameter: Trait<T>
        Example: From<T>, AsRef<T>
```

**Rule:** If a trait should have exactly one implementation per type, use associated types.
If multiple implementations per type make sense, use generics.

### Newtype Pattern

Wrap primitives to add domain semantics and type safety:

```rust
struct EmailAddress(String);

impl EmailAddress {
    fn new(raw: &str) -> Result<Self, &'static str> {
        if raw.contains('@') {
            Ok(Self(raw.to_owned()))
        } else {
            Err("invalid email: missing @")
        }
    }

    fn as_str(&self) -> &str { &self.0 }
}
```

Benefits: prevents mixing up `UserId(42)` with `OrderId(42)`, enables orphan rule workarounds, zero runtime cost with `#[repr(transparent)]`.

### Pattern Matching

```rust
// Destructuring in match
match config {
    Config { port: 443, tls: true, .. } => "HTTPS default",
    Config { port: 80, tls: false, .. } => "HTTP default",
    Config { port, .. } => &format!("custom port {port}"),
};

// if let for single-variant check
if let Some(value) = optional {
    println!("Got {value}");
}

// let-else for early return
let Some(value) = optional else {
    return Err("missing value".into());
};

// matches! macro — returns bool, great for filter/assert
assert!(matches!(status, Status::Active | Status::Pending));
let active_users: Vec<_> = users.iter()
    .filter(|u| matches!(u.status, Status::Active))
    .collect();

// match guard
match value {
    n if n < 0 => "negative",
    0 => "zero",
    n if n > 100 => "large",
    _ => "moderate",
}
```

### References

- The Rust Book: [Structs](https://doc.rust-lang.org/book/ch05-00-structs.html), [Enums](https://doc.rust-lang.org/book/ch06-00-enums.html), [Generics](https://doc.rust-lang.org/book/ch10-00-generics.html)
- Related: [core/traits.md](traits.md) (trait bounds), [patterns/idioms.md](../patterns/idioms.md) (newtype, typestate)
