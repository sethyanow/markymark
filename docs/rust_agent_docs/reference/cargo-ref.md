## Cargo.toml Quick Reference

> **TL;DR:** All common Cargo.toml fields in one place.

### [package]

```toml
[package]
name = "my-crate"               # Crate name (required)
version = "0.1.0"               # Semver (required)
edition = "2021"                 # Rust edition (required)
rust-version = "1.75"           # MSRV
authors = ["Name <email>"]
description = "Short description"
documentation = "https://docs.rs/my-crate"
homepage = "https://example.com"
repository = "https://github.com/user/repo"
license = "MIT OR Apache-2.0"
keywords = ["async", "http"]     # Max 5
categories = ["web-programming"]
exclude = ["tests/*", ".github/*"]
publish = true                   # false to prevent publishing
```

### [dependencies]

```toml
[dependencies]
# Version specifications
serde = "1"                      # ^1.0.0 (compatible)
serde = "=1.0.193"               # Exact version
serde = ">=1.0, <2.0"            # Range

# With features
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }

# Optional (enabled by feature flags)
serde_json = { version = "1", optional = true }

# Path (local development)
my-lib = { path = "../my-lib" }

# Git
my-lib = { git = "https://github.com/user/repo", branch = "main" }

[dev-dependencies]               # Test-only
criterion = "0.5"

[build-dependencies]             # build.rs only
cc = "1"
```

### [features]

```toml
[features]
default = ["json"]               # Enabled by default
json = ["dep:serde_json"]        # dep: prefix for optional deps
xml = ["dep:quick-xml"]
full = ["json", "xml"]           # Feature grouping
```

### [profile]

```toml
[profile.dev]
opt-level = 0                    # No optimization
debug = true                     # Full debug info

[profile.release]
opt-level = 3                    # Max optimization
lto = true                       # Link-time optimization
strip = true                     # Strip symbols
codegen-units = 1                # Better optimization, slower build
panic = "abort"                  # Smaller binary, no unwinding

[profile.bench]
debug = 1                        # Debug symbols for profiler
```

### [workspace]

```toml
[workspace]
members = ["crates/*"]
exclude = ["experimental/*"]
resolver = "2"                   # Required for edition 2021

[workspace.package]
edition = "2021"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }

[workspace.lints.clippy]
pedantic = "warn"
```

### [lib] and [[bin]]

```toml
[lib]
name = "my_lib"
crate-type = ["lib"]             # lib, cdylib, staticlib, rlib, dylib
proc-macro = false               # true for proc-macro crates

[[bin]]
name = "my-tool"
path = "src/bin/tool.rs"
```

### [lints]

```toml
[lints.rust]
unsafe_code = "forbid"
unused = "warn"

[lints.clippy]
pedantic = "warn"
nursery = "warn"
enum_glob_use = "deny"

# In workspace member:
[lints]
workspace = true
```

### References

- Cargo Reference: [Manifest Format](https://doc.rust-lang.org/cargo/reference/manifest.html)
- Related: [tooling/cargo.md](../tooling/cargo.md), [core/modules.md](../core/modules.md)
