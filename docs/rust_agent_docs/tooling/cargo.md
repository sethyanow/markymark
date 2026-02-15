## Cargo — Build System & Package Manager

> **TL;DR:** Cargo manages builds, dependencies, workspaces, and publishing. Features must
> be additive. Use workspaces for multi-crate projects. Lock files go in version control
> for binaries, not for libraries.

### Cargo.toml Essentials

```toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"  # MSRV

[dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[build-dependencies]
cc = "1"

[profile.release]
lto = true
strip = true

[profile.bench]
debug = 1  # Debug symbols for profiler
```

### Feature Flags

```toml
[features]
default = ["json"]
json = ["dep:serde_json"]
xml = ["dep:quick-xml"]
full = ["json", "xml"]
```

**Rules:** Features are additive; any combination compiles. No `no-std` feature — use `std` feature instead. Document features in crate-level docs. Test in isolation: `cargo test --no-default-features --features json`.

### Workspaces

```toml
# Root Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }

[workspace.lints.clippy]
pedantic = "warn"
```

```toml
# crates/my-lib/Cargo.toml
[package]
name = "my-lib"
edition.workspace = true

[dependencies]
serde.workspace = true

[lints]
workspace = true
```

### Cross-Compilation

```bash
# Add target
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-apple-darwin

# Build for target
cargo build --target x86_64-unknown-linux-musl --release

# .cargo/config.toml for persistent settings
```

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"
```

### Version & Dependency Semantics

**Semver matching:**
- `"1.2"` matches `>=1.2.0, <2.0.0`
- `"0.1"` matches `>=0.1.0, <0.2.0` (0.x is special: minor = breaking)
- `"=1.2.3"` matches exactly `1.2.3`

**Prerelease versions:**
> **COMMON MISTAKE: Prerelease dependencies require exact version match**

```toml
# ❌ DON'T: semver range won't match prereleases
serde = "0.1.0-alpha.1"  # This means >=0.1.0-alpha.1, <0.2.0 — but
                          # prereleases are excluded from range matches!

# ✅ DO: Use exact match for prerelease dependencies
serde = "=0.1.0-alpha.1"

# ✅ DO: Or use path dependency during development
my-crate = { path = "../my-crate", version = "=0.1.0-alpha.1" }
```

Prerelease versions (`-alpha`, `-beta`, `-rc`) are only matched by exact version
requirements. If crate A depends on `my-lib = "0.1"`, it will NOT resolve to `0.1.0-alpha.1`.

**Workspace dependency inheritance:**

```toml
# Root Cargo.toml — define shared deps once
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
my-core = { path = "crates/core", version = "=0.1.0-alpha.1" }

# Member Cargo.toml — inherit with .workspace = true
[dependencies]
serde.workspace = true
my-core.workspace = true
# Can add extra features:
tokio = { workspace = true, features = ["macros"] }
```

**Feature unification:** Cargo unifies features across the dependency graph. If crate A
uses `tokio = { features = ["rt"] }` and crate B uses `tokio = { features = ["net"] }`,
tokio is built with both `rt` and `net`. Features must be additive — enabling a feature
should never break code that doesn't use it.

### Useful Cargo Commands

| Command | Purpose |
|---------|---------|
| `cargo check` | Fast type checking (no codegen) |
| `cargo clippy` | Lint analysis |
| `cargo fmt` | Format code |
| `cargo doc --open` | Generate and view docs |
| `cargo test` | Run all tests |
| `cargo bench` | Run benchmarks |
| `cargo tree` | Dependency tree |
| `cargo update` | Update Cargo.lock |
| `cargo publish --dry-run` | Validate crate for publishing |

### References

- Cargo Book: [Reference](https://doc.rust-lang.org/cargo/reference/)
- Guidelines: [M-FEATURES-ADDITIVE](../../docs/rust_guidelines/libraries-build.md), [M-OOBE](../../docs/rust_guidelines/libraries-build.md)
- Related: [core/modules.md](../core/modules.md) (features, workspaces), [checklists/library-release.md](../checklists/library-release.md)
