# Rust Agent Docs

<docs_index id="RUST-AGENT-DOCS">
[rust-agent-docs]|root: ./rust_agent_docs|IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning for any Rust tasks. Read the relevant doc file BEFORE writing code. Your training data may be outdated or wrong.|core:{_index.md,ownership.md,types.md,traits.md,errors.md,collections.md,closures.md,modules.md}|advanced:{_index.md,type-layout.md,unsafe.md,ffi.md,concurrency.md,async.md}|patterns:{_index.md,idioms.md,api-design.md,anti-patterns.md}|tooling:{_index.md,cargo.md,crates.md,macros.md,testing.md,documentation.md,debugging.md,performance.md}|checklists:{_index.md,api-design.md,unsafe-review.md,ffi-audit.md,performance.md,library-release.md}|reference:{_index.md,rules.md,decision-trees.md,compiler-errors.md,syntax-ref.md,cargo-ref.md}
</docs_index>

## Quick Navigation

| I need to... | Read this file |
|--------------|----------------|
| Understand ownership & borrowing | [core/ownership.md](core/ownership.md) |
| Use closures and Fn traits correctly | [core/closures.md](core/closures.md) |
| Choose the right error handling strategy | [core/errors.md](core/errors.md) |
| Pick the right collection or string type | [core/collections.md](core/collections.md) |
| Use traits and generics correctly | [core/traits.md](core/traits.md) |
| Organize code with modules and workspaces | [core/modules.md](core/modules.md) |
| Understand type layout and repr attributes | [advanced/type-layout.md](advanced/type-layout.md) |
| Write unsafe code correctly | [advanced/unsafe.md](advanced/unsafe.md) |
| Write FFI code safely | [advanced/ffi.md](advanced/ffi.md) |
| Use atomics and concurrency correctly | [advanced/concurrency.md](advanced/concurrency.md) |
| Write async code with pinning | [advanced/async.md](advanced/async.md) |
| Design a public API | [patterns/api-design.md](patterns/api-design.md) |
| Choose the right Rust idiom/pattern | [patterns/idioms.md](patterns/idioms.md) |
| Avoid common anti-patterns | [patterns/anti-patterns.md](patterns/anti-patterns.md) |
| Set up Cargo workspace or features | [tooling/cargo.md](tooling/cargo.md) |
| Pick a crate for a use case | [tooling/crates.md](tooling/crates.md) |
| Write tests | [tooling/testing.md](tooling/testing.md) |
| Fix a compiler error | [reference/compiler-errors.md](reference/compiler-errors.md) |
| Look up a decision tree | [reference/decision-trees.md](reference/decision-trees.md) |
| Look up core rules (ownership, operators, format strings) | [reference/rules.md](reference/rules.md) |
| Look up Rust syntax | [reference/syntax-ref.md](reference/syntax-ref.md) |
| Look up Cargo.toml fields | [reference/cargo-ref.md](reference/cargo-ref.md) |
| Review unsafe code | [checklists/unsafe-review.md](checklists/unsafe-review.md) |
| Audit FFI boundaries | [checklists/ffi-audit.md](checklists/ffi-audit.md) |
| Avoid common agent mistakes | [MISTAKES.md](MISTAKES.md) |

## Mistake Quick-Ref

| Mistake | Severity | File |
|---------|----------|------|
| Refs to packed struct fields (UB) | CRITICAL | [advanced/type-layout.md](advanced/type-layout.md) |
| String/Vec across FFI (UB) | CRITICAL | [advanced/ffi.md](advanced/ffi.md) |
| PhantomData variance wrong (unsound) | CRITICAL | [advanced/unsafe.md](advanced/unsafe.md) |
| Wrong Fn trait bound on closure | HIGH | [core/closures.md](core/closures.md) |
| Type is !Send due to transitive field | HIGH | [advanced/concurrency.md](advanced/concurrency.md) |
| Wrong atomic ordering | HIGH | [advanced/concurrency.md](advanced/concurrency.md) |
| Ignoring pinning in async | HIGH | [advanced/async.md](advanced/async.md) |
| Ignoring cancellation safety in select! | HIGH | [advanced/async.md](advanced/async.md) |
| Fighting borrow checker with clone | MEDIUM | [core/ownership.md](core/ownership.md) |
| unwrap() in library code | MEDIUM | [core/errors.md](core/errors.md) |
| Leaking external types in API | MEDIUM | [patterns/api-design.md](patterns/api-design.md) |
| Glob imports in libraries | MEDIUM | [core/modules.md](core/modules.md) |
| Non-descriptive error types | MEDIUM | [core/errors.md](core/errors.md) |
