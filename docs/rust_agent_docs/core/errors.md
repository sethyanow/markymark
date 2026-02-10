## Error Handling — Option, Result & Error Design

> **TL;DR:** Use `Option` for absence, `Result` for failure. Libraries: use `thiserror` with
> descriptive error structs. Applications: use `anyhow`/`eyre`. Never `unwrap()` in library code.
> Propagate errors with `?`.

### Option Combinators

| Combinator | Purpose | Example |
|-----------|---------|---------|
| `map(f)` | Transform inner value | `Some(3).map(|x| x * 2)` → `Some(6)` |
| `and_then(f)` | Chain fallible transforms | `opt.and_then(|x| x.parse().ok())` |
| `unwrap_or(default)` | Provide fallback | `None.unwrap_or(0)` → `0` |
| `unwrap_or_else(f)` | Lazy fallback | `None.unwrap_or_else(|| expensive())` |
| `ok_or(err)` | Convert to `Result` | `opt.ok_or("missing")?` |
| `filter(pred)` | Keep if predicate true | `Some(3).filter(|x| x > &5)` → `None` |
| `flatten()` | `Option<Option<T>>` → `Option<T>` | Removes nesting |

### Result Combinators & `?` Operator

```rust
use std::fs;
use std::io;

fn read_config(path: &str) -> Result<Config, io::Error> {
    let contents = fs::read_to_string(path)?;  // ? propagates Err
    let config = parse_config(&contents)?;
    Ok(config)
}
```

The `?` operator: on `Err`, returns early with the error (calling `From::from()` for conversion). On `Ok`, unwraps the value.

### Library vs Application Errors Decision Tree

```
Are you writing a library (used by other crates)?
├─ YES → Use thiserror with descriptive error structs
│   ├─ One error enum per module or logical group
│   ├─ Implement std::error::Error, Display, Debug
│   ├─ Include context (what failed, with what input)
│   ├─ ❌ Do NOT use anyhow/eyre in library code
│   └─ ❌ Do NOT use unwrap()/expect()
└─ NO (application code)
    └─ Use anyhow or eyre
        ├─ Re-export Result type: use anyhow::Result;
        ├─ Add context: .context("failed to read config")?
        └─ Library errors auto-convert via From
```

### thiserror Pattern (Libraries)

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("record not found: {id}")]
    NotFound { id: String },

    #[error("connection timeout after {elapsed_ms}ms")]
    Timeout { elapsed_ms: u64 },

    #[error("serialization failed")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O error reading {path}")]
    Io {
        path: String,
        #[source]
        cause: std::io::Error,
    },
}
```

### anyhow Pattern (Applications)

```rust
use anyhow::{Context, Result};

fn start_server() -> Result<()> {
    let config = load_config()
        .context("failed to load server config")?;
    let listener = bind_address(&config.address)
        .with_context(|| format!("failed to bind {}", config.address))?;
    Ok(())
}
```

### Error Type Design Checklist

- [ ] Implements `std::error::Error`, `Display`, `Debug`
- [ ] Variants are descriptive (not just `Generic(String)`)
- [ ] Includes context: what operation failed and relevant parameters
- [ ] Uses `#[from]` for automatic conversion from source errors
- [ ] Uses `#[source]` to preserve error chains
- [ ] No catch-all `Other(String)` variant (use `#[non_exhaustive]` instead)

### Panic vs Result

| Situation | Use |
|-----------|-----|
| Programmer bug (invariant violation) | `panic!` / `unreachable!` |
| Invalid input from user/network | `Result::Err` |
| Unrecoverable system error | `Result::Err` (let caller decide) |
| Prototype / test code | `unwrap()` is acceptable |
| Library code | **Never** `unwrap()` — always `?` |

> ⚠️ **COMMON MISTAKE: Using `unwrap()` in library code**
> Library code must never panic on errors. Use `?` to propagate errors to the caller.
> Only `unwrap()` when you can **prove** the value is always `Some`/`Ok` and add a comment
> explaining why.

> ⚠️ **COMMON MISTAKE: Non-descriptive error types**
> Don't use `Box<dyn Error>` or `String` as error types in libraries. Define proper error
> enums with `thiserror` that tell callers *what* went wrong and *why*.

### References

- The Rust Book: [Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- Guidelines: [M-ERRORS-CANONICAL-STRUCTS](../../docs/rust_guidelines/libraries-ux.md)
- Guidelines: [M-APP-ERROR](../../docs/rust_guidelines/applications.md)
- Related: [patterns/api-design.md](../patterns/api-design.md) (error design in APIs)
