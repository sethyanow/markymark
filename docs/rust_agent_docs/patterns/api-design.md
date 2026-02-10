## API Design — Public Interfaces & Naming

> **TL;DR:** Design APIs that are hard to misuse: accept borrowed types, use newtypes for
> semantics, provide builders for complex construction, follow Rust naming conventions,
> and never leak external crate types through your public surface.

### Public API Surface Principles

1. **Minimize surface** — expose only what consumers need
2. **Types over documentation** — make invalid states unrepresentable
3. **Borrow by default** — take `&T` unless you need ownership
4. **Non-exhaustive by default** — `#[non_exhaustive]` on public enums/structs in libraries
5. **Stable compatibility** — adding fields/variants should not break callers

### Method Naming Conventions

| Prefix | Meaning | Returns | Example |
|--------|---------|---------|---------|
| `new` | Constructor | `Self` | `Vec::new()` |
| `with_` | Constructor with param | `Self` | `Vec::with_capacity(10)` |
| `get_` | Getter (usually omitted) | `&T` or `Option<&T>` | `.get(index)` |
| `set_` | Setter | `()` or `&mut Self` | `.set_port(443)` |
| `is_` / `has_` | Boolean query | `bool` | `.is_empty()` |
| `as_` | Cheap reference conversion | `&U` | `.as_str()`, `.as_bytes()` |
| `to_` | Expensive conversion | `U` (owned) | `.to_string()`, `.to_vec()` |
| `into_` | Consuming conversion | `U` (owned, consumes self) | `.into_inner()` |
| `try_` | Fallible operation | `Result<T, E>` | `.try_lock()` |
| `from_` | Static constructor | `Self` | `String::from_utf8()` |
| `_mut` | Mutable variant | `&mut T` | `.iter_mut()` |

### Return Type Design

| Situation | Return | Not |
|-----------|--------|----|
| Might not exist | `Option<T>` | Empty value / sentinel |
| Can fail | `Result<T, E>` | Panic |
| Cheap accessor | `&T` | Clone |
| Collection is empty | Empty collection | `Option<Vec<T>>` |

### Argument Design

```rust
// ✅ Accept borrowed when ownership not needed
fn process(data: &[u8]) -> Result<Output, Error> { /* ... */ }

// ✅ Accept impl Into<String> for flexible construction
fn set_name(&mut self, name: impl Into<String>) { self.name = name.into(); }

// ✅ Accept impl AsRef<Path> for path flexibility
fn read_config(path: impl AsRef<std::path::Path>) -> Result<Config, Error> { /* ... */ }

// ❌ Don't force callers to allocate
// fn process(data: Vec<u8>) → forces caller to own a Vec
// fn process(data: String) → forces caller to own a String
```

### Generics vs Trait Objects at API Boundaries

```
Is the concrete type known at compile time?
├─ YES → Use generics (impl Trait / <T: Trait>)
│   ├─ Better performance (monomorphized)
│   └─ Larger binary size
└─ NO (heterogeneous collection, plugin system, return type varies)
    └─ Use trait objects (Box<dyn Trait> / &dyn Trait)
        ├─ Smaller binary
        └─ Requires object safety
```

### Builder APIs for Complex Construction

Use builders when:
- More than 3 parameters
- Many optional parameters with defaults
- Construction may fail (builder validates)

See [patterns/idioms.md](idioms.md) for the full builder pattern.

### Backwards Compatibility Rules

| Change | Breaking? |
|--------|-----------|
| Adding a method to a trait | ✅ Yes (unless default impl) |
| Adding a variant to an enum | ✅ Yes (unless `#[non_exhaustive]`) |
| Adding a public field to a struct | ✅ Yes (unless `#[non_exhaustive]`) |
| Adding a new function to a module | ❌ No |
| Adding a default trait method | ❌ No |
| Adding a new optional feature | ❌ No |
| Widening an argument type | ❌ No |
| Narrowing a return type | ❌ No |

> ⚠️ **COMMON MISTAKE: Leaking external crate types in public API**
> Re-exporting or using third-party types in your public API surface means your
> semver is now coupled to theirs. Wrap external types in newtypes.

```rust
// ❌ DON'T: Expose dependency in public API
// pub fn parse(input: &str) -> serde_json::Value { ... }

// ✅ DO: Wrap in your own type
pub struct JsonValue(serde_json::Value);
pub fn parse(input: &str) -> Result<JsonValue, ParseError> { /* ... */ }
```

### References

- Rust API Guidelines: [Checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
- Guidelines: [M-DONT-LEAK-TYPES](../../docs/rust_guidelines/libraries-interop.md), [M-AVOID-WRAPPERS](../../docs/rust_guidelines/libraries-ux.md)
- Guidelines: [M-DESIGN-FOR-AI](../../docs/rust_guidelines/ai.md)
- Related: [patterns/idioms.md](idioms.md), [core/traits.md](../core/traits.md), [checklists/api-design.md](../checklists/api-design.md)
