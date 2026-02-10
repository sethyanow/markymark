## Anti-Patterns — What NOT to Do

> **TL;DR:** Avoid stringly-typed APIs, excessive `unwrap()`, cloning as a crutch, fighting
> the borrow checker, unnecessary heap allocations, ignoring clippy, glob imports,
> leaky abstractions, and excessive generics. Each entry shows the fix.

### Anti-Pattern to Idiomatic Fix Table

| Anti-Pattern | Problem | Idiomatic Fix |
|-------------|---------|---------------|
| Stringly-typed APIs | No compile-time validation | Enums or newtypes |
| `unwrap()` everywhere | Panics in production | `?` with proper error types |
| `.clone()` as escape hatch | Hides ownership issues, wastes perf | Restructure borrows, use `Cow` |
| Fighting the borrow checker | Complex workarounds, fragile code | Restructure data/algorithm |
| Unnecessary `Box<dyn Trait>` | Heap allocation, vtable overhead | Generics / `impl Trait` |
| Ignoring clippy warnings | Missing idioms and bugs | Fix warnings, use `#[expect]` |
| `pub use module::*` | Unstable API, unclear docs | Explicit re-exports |
| Leaking external types | Semver coupling | Newtype wrappers |
| Excessive generics | Unreadable signatures, slow compilation | Concrete types first |
| God structs | Everything in one type | Split into focused types |

### Stringly-Typed APIs → Enums

```rust
// ❌ DON'T: String for known set of values
fn set_log_level(level: &str) { /* "info", "debug", "error" */ }

// ✅ DO: Enum for compile-time exhaustiveness
#[derive(Debug, Clone, Copy)]
enum LogLevel { Debug, Info, Warn, Error }
fn set_log_level(level: LogLevel) { /* ... */ }
```

### Excessive unwrap → Error Propagation

```rust
// ❌ DON'T: Panic on every possible failure
fn load_config() -> Config {
    let text = std::fs::read_to_string("config.toml").unwrap();
    toml::from_str(&text).unwrap()
}

// ✅ DO: Propagate errors with context
fn load_config() -> anyhow::Result<Config> {
    let text = std::fs::read_to_string("config.toml")
        .context("failed to read config.toml")?;
    let config = toml::from_str(&text)
        .context("failed to parse config.toml")?;
    Ok(config)
}
```

### Clone as Escape Hatch → Restructure

```rust
// ❌ DON'T: Clone to avoid borrow issues
fn process(data: &mut Vec<String>) {
    let cloned = data.clone();  // Expensive!
    for item in &cloned {
        if item.starts_with("remove") {
            data.retain(|x| x != item);
        }
    }
}

// ✅ DO: Restructure to avoid the conflict
fn process(data: &mut Vec<String>) {
    data.retain(|item| !item.starts_with("remove"));
}
```

### Unnecessary Heap Allocation → Stack or Generics

```rust
// ❌ DON'T: Box<dyn Trait> when type is known
fn get_handler() -> Box<dyn Handler> { Box::new(MyHandler) }

// ✅ DO: Return concrete type or impl Trait
fn get_handler() -> impl Handler { MyHandler }

// ❌ DON'T: Vec for fixed-size data
fn get_pair() -> Vec<i32> { vec![1, 2] }

// ✅ DO: Use array or tuple
fn get_pair() -> [i32; 2] { [1, 2] }
```

### Ignoring Clippy → Fix or Expect

```rust
// ❌ DON'T: Blanket-suppress warnings
#[allow(clippy::all)]

// ✅ DO: Fix warnings, or justify exceptions
#[expect(clippy::cast_possible_truncation, reason = "value guaranteed < 256 by validation")]
let byte = value as u8;
```

### God Structs → Focused Types

```rust
// ❌ DON'T: Everything in one struct
struct App {
    config: Config,
    database: Database,
    cache: Cache,
    auth: Auth,
    mailer: Mailer,
    // ... 20 more fields
}

// ✅ DO: Compose focused types
struct App { services: Services, config: AppConfig }
struct Services { database: Database, cache: Cache, auth: Auth }
```

### References

- Clippy lints: [rust-lang.github.io/rust-clippy](https://rust-lang.github.io/rust-clippy/)
- Guidelines: [M-CONCISE-NAMES](../../docs/rust_guidelines/universal.md), [M-STATIC-VERIFICATION](../../docs/rust_guidelines/universal.md)
- Related: [core/errors.md](../core/errors.md), [core/ownership.md](../core/ownership.md), [patterns/idioms.md](idioms.md)
