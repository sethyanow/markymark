## Rust 2024 Edition — Migration Guide

> **TL;DR:** Rust 2024 (stabilized in 1.85.0, February 2025) is the largest edition yet.
> Key changes: `unsafe` required for extern blocks and env functions, `gen` keyword reserved,
> improved temporaries scoping, `Future`/`IntoFuture` added to prelude, async closures,
> and Cargo's rust-version-aware resolver.

### Language Changes

**Unsafe extern blocks** — all `extern` blocks must be marked `unsafe`:
```rust
// 2021: allowed as safe
extern "C" { fn foo(); }

// 2024: must be marked unsafe
unsafe extern "C" { fn foo(); }
```

**Environment functions now unsafe** — `set_var`/`remove_var` require unsafe:
```rust
// 2024: mutating env is unsafe (data race risk in multi-threaded code)
unsafe { std::env::set_var("KEY", "value"); }
unsafe { std::env::remove_var("KEY"); }
```

**Reserved syntax:**
- `gen` keyword reserved for future generator blocks — cannot use as identifier
- `#"foo"#` guarded string literals reserved
- `##` tokens reserved

**impl Trait lifetime capture:**
- RPIT now auto-captures all in-scope lifetimes by default
- Use `use<'a, T>` for explicit capture control (Rust 1.82+)

**Temporaries scoping:**
- Temporaries in `if let` expressions have shorter, more predictable scope
- Temporaries in block final expressions have more precise scope
- Reduces unwanted borrowing conflicts

**Never type fallback:**
- `!` (never type) fallback changes affect type inference in some edge cases

**Match ergonomics restrictions:**
- Stricter rules for pattern matching with references

### Standard Library Changes

- `Future` and `IntoFuture` added to prelude (may cause name collisions)
- `IntoIterator` for `Box<[T]>` implemented (new iteration capability)

### Async Changes

```rust
// Async closures — new in 2024 edition
let f = async || { fetch_data().await };
let result = f().await;
```

### Cargo Changes

- **Rust-version-aware resolver:** uses `rust-version` field to select compatible dependency versions
- **Kebab-case enforcement:** consistent key naming in Cargo.toml (`default-features` not `default_features`)
- Better default features handling for workspaces

### Migration Steps

```bash
# 1. Update Cargo.toml
#    edition = "2024"

# 2. Run automatic migration
cargo fix --edition

# 3. Build and test
cargo build --all
cargo test --all
cargo clippy --all
```

`cargo fix` is conservative — it won't change semantics. Manual review needed for:
- `unsafe extern` blocks
- `unsafe { set_var(...) }` / `unsafe { remove_var(...) }`
- Any use of `gen` as an identifier

### References

- [Rust 2024 Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)
- [Announcing Rust 1.85.0](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html)
