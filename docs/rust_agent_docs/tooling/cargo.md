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
