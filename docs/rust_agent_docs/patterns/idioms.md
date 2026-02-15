## Rust Idioms & Patterns Catalog

> **TL;DR:** Rust has well-established patterns for common problems: Builder for complex
> construction, Newtype for type safety, Typestate for compile-time state machines,
> RAII for resource management, and Cow for flexible borrowing.

### Pattern Selection Decision Tree

```
What problem are you solving?
├─ Complex object construction (>3 params)?
│   └─ Builder pattern
├─ Adding semantic meaning to a primitive?
│   └─ Newtype pattern
├─ Enforcing valid state transitions at compile time?
│   └─ Typestate pattern
├─ Resource cleanup on scope exit?
│   └─ RAII / Drop pattern
├─ Avoiding allocation when data might be borrowed OR owned?
│   └─ Cow (Clone-on-Write)
├─ Smart pointer-like access to an inner type?
│   └─ Deref polymorphism
├─ Adding methods to a foreign type?
│   └─ Extension trait
├─ Preventing external implementations of a trait?
│   └─ Sealed trait
├─ Tagging types without data?
│   └─ Marker trait
├─ Implementing a trait for all types matching a bound?
│   └─ Blanket implementation
└─ Unsure?
    └─ Start with the simplest approach; refactor as needed
```

### Builder Pattern

Use when construction requires >3 parameters or many optional fields.

```rust
#[derive(Debug)]
struct Server {
    host: String,
    port: u16,
    max_connections: usize,
    tls: bool,
}

struct ServerBuilder {
    host: String,
    port: u16,
    max_connections: usize,
    tls: bool,
}

impl ServerBuilder {
    fn new(host: impl Into<String>) -> Self {
        Self { host: host.into(), port: 8080, max_connections: 100, tls: false }
    }

    fn port(mut self, port: u16) -> Self { self.port = port; self }
    fn max_connections(mut self, n: usize) -> Self { self.max_connections = n; self }
    fn tls(mut self, enabled: bool) -> Self { self.tls = enabled; self }

    fn build(self) -> Server {
        Server {
            host: self.host, port: self.port,
            max_connections: self.max_connections, tls: self.tls,
        }
    }
}
```

### Newtype Pattern

Wrap primitives to add domain semantics at zero runtime cost.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UserId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OrderId(u64);

// Can't accidentally pass UserId where OrderId is expected!
fn process_order(order: OrderId, user: UserId) { /* ... */ }
```

Add `#[repr(transparent)]` if you need FFI compatibility. See [advanced/type-layout.md](../advanced/type-layout.md).

### Typestate Pattern

Encode valid state transitions in the type system — invalid transitions become compile errors.

```rust
struct Unvalidated;
struct Validated;

struct Form<State> {
    data: String,
    _state: std::marker::PhantomData<State>,
}

impl Form<Unvalidated> {
    fn new(data: String) -> Self {
        Self { data, _state: std::marker::PhantomData }
    }

    fn validate(self) -> Result<Form<Validated>, &'static str> {
        if self.data.is_empty() { return Err("empty form"); }
        Ok(Form { data: self.data, _state: std::marker::PhantomData })
    }
}

impl Form<Validated> {
    fn submit(&self) -> String {
        format!("Submitted: {}", self.data)
    }
}

// form.submit() only available after validate() succeeds!
```

### RAII / Drop Pattern

Resources are released when the owner goes out of scope.

```rust
struct TempFile { path: std::path::PathBuf }

impl TempFile {
    fn new(path: impl Into<std::path::PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        std::fs::File::create(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
// File is automatically deleted when TempFile goes out of scope
```

#### Drop Order Rules

| Scope | Drop Order |
|-------|-----------|
| Local variables | **Reverse** declaration order (last declared, first dropped) |
| Struct fields | **Declaration** order (first field dropped first) |
| Tuple elements | Index order (`.0` first, `.1` second, ...) |
| Enum variants | The active variant's fields only |

```rust
// Variables: c drops before b, b before a
let a = ResourceA::new();
let b = ResourceB::new();
let c = ResourceC::new();
// scope end: drop(c), drop(b), drop(a)

// Struct fields: x drops before y (declaration order)
struct S { x: ResourceX, y: ResourceY }
```

**Early drop with `std::mem::drop()`:** When you need to release a resource before scope end
(e.g., releasing a lock before doing more work):

```rust
let guard = mutex.lock().unwrap();
let data = guard.clone();
drop(guard); // Release lock early
// ... use data without holding the lock
```

**Recursive drop:** Rust automatically drops all fields. If you implement `Drop`, your
`drop(&mut self)` runs first, then Rust drops each field. Don't manually drop fields
in your `drop()` impl — Rust handles it.

### Cow (Clone-on-Write)

Avoid cloning when you might only need a borrow:

```rust
use std::borrow::Cow;

fn normalize_name(name: &str) -> Cow<'_, str> {
    if name.contains(' ') {
        Cow::Owned(name.replace(' ', "_"))  // Allocates only when needed
    } else {
        Cow::Borrowed(name)  // Zero-cost borrow
    }
}
```

### Deref Polymorphism

Allow transparent access to an inner type:

```rust
use std::ops::Deref;

struct EmailAddress(String);

impl Deref for EmailAddress {
    type Target = str;
    fn deref(&self) -> &str { &self.0 }
}

let email = EmailAddress("user@example.com".into());
println!("Length: {}", email.len());  // Uses str methods directly
```

**Caution:** Only use Deref for smart-pointer-like types, not for general inheritance emulation.

#### Automatic Deref Coercion Rules

Rust automatically inserts deref calls when passing arguments to functions/methods.
The three coercion rules:

| From | To | Requires |
|------|----|----------|
| `&T` | `&U` | `T: Deref<Target=U>` |
| `&mut T` | `&mut U` | `T: DerefMut<Target=U>` |
| `&mut T` | `&U` | `T: Deref<Target=U>` |

Note: `&T` → `&mut U` is **never** allowed (would violate borrowing rules).

**Common auto-coercions agents should know:**

| You have | Coerces to | Because |
|----------|-----------|---------|
| `&String` | `&str` | `String: Deref<Target=str>` |
| `&Vec<T>` | `&[T]` | `Vec<T>: Deref<Target=[T]>` |
| `&Box<T>` | `&T` | `Box<T>: Deref<Target=T>` |
| `&Arc<T>` | `&T` | `Arc<T>: Deref<Target=T>` |
| `&PathBuf` | `&Path` | `PathBuf: Deref<Target=Path>` |
| `&OsString` | `&OsStr` | `OsString: Deref<Target=OsStr>` |

Coercion is **recursive** and resolved at **compile time** (zero runtime cost).
`&Box<String>` → `&String` → `&str` happens automatically.

### Blanket Implementations

Implement a trait for all types matching a bound:

```rust
trait Greet {
    fn greet(&self) -> String;
}

// Blanket impl: anything that implements Display can be greeted
impl<T: std::fmt::Display> Greet for T {
    fn greet(&self) -> String {
        format!("Hello, {self}!")
    }
}
```

### References

- Rust API Guidelines: [Checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
- Guidelines: [M-INIT-BUILDER](../../docs/rust_guidelines/libraries-ux.md)
- Related: [patterns/api-design.md](api-design.md) (using patterns in APIs), [core/traits.md](../core/traits.md) (extension traits, sealed traits)
