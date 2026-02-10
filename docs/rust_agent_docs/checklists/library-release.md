## Library Release Checklist

> **TL;DR:** Use this checklist before publishing a library to crates.io.

### Documentation
- [ ] All public items documented with canonical sections
- [ ] Crate-level documentation (`//!` in lib.rs) with overview and examples
- [ ] Modules have `//!` docs
- [ ] CHANGELOG.md updated with user-facing changes
- [ ] README.md is current and includes usage example

### API Quality
- [ ] No glob re-exports (`pub use *`)
- [ ] Re-exports use `#[doc(inline)]`
- [ ] External types wrapped (not leaked in public API)
- [ ] Non-exhaustive on public enums and structs with private fields
- [ ] Error types implement `std::error::Error` + `Display` + `Debug`
- [ ] Naming follows Rust API Guidelines

### Versioning
- [ ] Version bumped following semver
  - [ ] Patch: bug fixes, no API changes
  - [ ] Minor: new features, backwards compatible
  - [ ] Major: breaking changes
- [ ] MSRV (minimum supported Rust version) documented and tested
- [ ] `rust-version` field set in Cargo.toml

### Features & Dependencies
- [ ] Feature flags tested in isolation (`--no-default-features --features X`)
- [ ] All feature combinations compile
- [ ] Features are additive (enabling a feature doesn't remove items)
- [ ] Optional dependencies use `dep:` prefix in feature definitions
- [ ] Dependencies are minimal and up-to-date

### Quality
- [ ] `cargo clippy` is clean (no warnings)
- [ ] `cargo doc` is clean (no warnings)
- [ ] `cargo test` passes on all Tier 1 targets
- [ ] `cargo fmt -- --check` shows no formatting issues
- [ ] No TODO/FIXME items in code that affects public API
- [ ] License file present and correct

### Publishing
- [ ] `cargo publish --dry-run` succeeds
- [ ] Git tag created matching version
- [ ] CI passes on latest stable Rust
- [ ] CI tests MSRV

### References
- Cargo: [Publishing](https://doc.rust-lang.org/cargo/reference/publishing.html)
- Guidelines: [M-FEATURES-ADDITIVE](../../docs/rust_guidelines/libraries-build.md), [M-OOBE](../../docs/rust_guidelines/libraries-build.md)
- Semver: [semver.org](https://semver.org/)
