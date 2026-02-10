## Public API Design Checklist

> **TL;DR:** Use this checklist when designing or reviewing public library APIs.

### Documentation
- [ ] All public items have documentation with `///` doc comments
- [ ] First sentence is ≤15 words and summarizes the item
- [ ] Examples section with runnable code (`# Examples`)
- [ ] Error conditions documented (`# Errors`)
- [ ] Panic conditions documented (`# Panics`)
- [ ] Modules have `//!` docs with purpose and key types

### Types & Errors
- [ ] Error types are descriptive enums/structs (not `String` or `Box<dyn Error>`)
- [ ] Error types implement `std::error::Error`, `Display`, `Debug`
- [ ] No `anyhow`/`eyre` used in library error types
- [ ] Public enums are `#[non_exhaustive]`
- [ ] Public types derive `Debug`; user-facing types implement `Display`

### API Surface
- [ ] No external crate types leaked in public API (wrap in newtypes)
- [ ] Arguments use borrowed types where ownership is not needed (`&str` not `String`)
- [ ] Methods follow Rust naming conventions (`as_`, `to_`, `into_`, `is_`, `try_`)
- [ ] Builders used for complex constructors (>3 params)
- [ ] `impl AsRef`, `impl Into` used where appropriate for ergonomics
- [ ] Essential functions are inherent methods, not trait methods

### Stability
- [ ] No glob re-exports (`pub use *`)
- [ ] Re-exports use `#[doc(inline)]`
- [ ] Feature flags are additive — all combinations compile
- [ ] Feature flags are documented in crate-level docs
- [ ] Conversion traits (`From`, `TryFrom`) implemented where natural

### References
- Guidelines: [M-DONT-LEAK-TYPES](../../docs/rust_guidelines/libraries-interop.md), [M-NO-GLOB-REEXPORTS](../../docs/rust_guidelines/libraries-resilience.md)
- Detail: [patterns/api-design.md](../patterns/api-design.md)
