## Modules — Code Organization, Visibility & Workspaces

> **TL;DR:** Use `mod` for organization, `pub(crate)` as default visibility, explicit
> re-exports, feature flags for optional functionality, and workspaces for multi-crate projects.
> Never use `pub use *` in libraries.

### Module System & File Mapping

```
src/
├── main.rs (or lib.rs)    ← crate root
├── config.rs              ← declared as `mod config;` in main.rs
├── handlers/
│   ├── mod.rs             ← declared as `mod handlers;` in main.rs
│   ├── auth.rs            ← declared as `mod auth;` in mod.rs
│   └── api.rs             ← declared as `mod api;` in mod.rs
```

**Modern style** (Rust 2018+): prefer `handlers.rs` + `handlers/` directory over `handlers/mod.rs`.

```rust
// src/main.rs
mod config;        // loads src/config.rs
mod handlers;      // loads src/handlers.rs (then its submodules)

use config::AppConfig;
use handlers::api::Router;
```

### Visibility Modifiers

| Modifier | Visible To | Use For |
|----------|-----------|---------|
| (none) | Same module only | Implementation details |
| `pub(crate)` | Anywhere in the crate | Internal APIs shared across modules |
| `pub(super)` | Parent module | Helper functions for parent |
| `pub(in crate::path)` | Specific ancestor | Rare; targeted internal sharing |
| `pub` | Everyone (public API) | Intentional public surface |

**Default to `pub(crate)`** for internal sharing. Use `pub` only for your intentional public API.

### Re-export Patterns

```rust
// ✅ Explicit re-exports with doc inlining
#[doc(inline)]
pub use crate::config::AppConfig;

#[doc(inline)]
pub use crate::errors::AppError;

// ❌ DON'T: Glob re-exports in libraries
// pub use crate::internals::*;  // Breaks API stability, hides API shape
```

> ⚠️ **COMMON MISTAKE: Glob re-exports (`pub use module::*`) in libraries**
> This leaks internal items into your public API surface, makes documentation unclear,
> and causes accidental breaking changes when internal items are renamed.
> Always use explicit `pub use` with `#[doc(inline)]`.

### Workspace Organization

```toml
# Cargo.toml (workspace root)
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
```

```toml
# crates/my-lib/Cargo.toml
[package]
name = "my-lib"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
```

**When to create a workspace:**
- Multiple related binaries sharing code
- Library + CLI tool
- Core library + FFI bindings + WASM bindings
- Compile time is a concern (parallel crate compilation)

### Feature Flags Best Practices

```toml
[features]
default = ["json"]
json = ["dep:serde_json"]
xml = ["dep:quick-xml"]
# Use `dep:` prefix to avoid creating implicit features
```

**Rules:**
- Features MUST be **additive** — enabling any combination must compile
- NO `no-std` feature; use a `std` feature (additive) instead
- Adding a feature must not remove or alter public items
- Document each feature in crate-level docs
- Test features in isolation: `cargo test --no-default-features --features json`

```rust
// Conditional compilation
#[cfg(feature = "json")]
pub fn parse_json(input: &str) -> Result<Value, JsonError> { /* ... */ }

// Conditional dependency use
#[cfg(feature = "json")]
use serde_json as json;
```

### Prelude Pattern (for Applications)

```rust
// src/prelude.rs — internal convenience, NOT for libraries
pub use crate::config::AppConfig;
pub use crate::errors::{AppError, AppResult};
pub use tracing::{debug, error, info, warn};

// In other modules:
use crate::prelude::*;
```

Only use this pattern in applications, never in libraries.

### References

- The Rust Book: [Modules](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)
- Cargo Book: [Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html), [Features](https://doc.rust-lang.org/cargo/reference/features.html)
- Guidelines: [M-NO-GLOB-REEXPORTS](../../docs/rust_guidelines/libraries-resilience.md), [M-FEATURES-ADDITIVE](../../docs/rust_guidelines/libraries-build.md)
- Related: [tooling/cargo.md](../tooling/cargo.md) (Cargo details)
