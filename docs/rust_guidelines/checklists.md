# Rust Guidelines Checklists (one-liners)

<agent>
<goal>Fast “did we remember the basics?” scan during review.</goal>
<canonical>../rust_guidelines_full.md</canonical>
<editing_rules>Keep guideline IDs stable; see `AGENTS.md` when updating this list.</editing_rules>
</agent>

Quick reminders; see themed files for detail and `../rust_guidelines_full.md` for the original text.

## AI
- M-DESIGN-FOR-AI: Idiomatic APIs, strong types, thorough docs/examples, testable, good coverage.

## Applications
- M-APP-ERROR: Use one app-level error crate (anyhow/eyre); libraries use canonical errors.
- M-MIMALLOC-APPS: Set mimalloc as global allocator for apps.

## Documentation
- M-CANONICAL-DOCS: Canonical sections; summary sentence required.
- M-DOC-INLINE: `#[doc(inline)]` for `pub use`; avoid globs.
- M-FIRST-DOC-SENTENCE: First sentence one line (~15 words).
- M-MODULE-DOCS: Modules documented with purpose/examples/links.

## FFI
- M-ISOLATE-DLL-STATE: Share only portable `#[repr(C)]` data; no statics/TypeId/allocs across DLLs.

## Performance
- M-HOTPATH: Find hot paths early; benchmark/profile; document hotspots.
- M-THROUGHPUT: Optimize items per cycle; batch, avoid empty spins, yield when idle.
- M-YIELD-POINTS: Add `yield_now().await` in long-running tasks (esp. CPU-bound).

## Safety
- M-UNSAFE-IMPLIES-UB: `unsafe` only when misuse risks UB.
- M-UNSAFE: Use `unsafe` only for novel abstractions/perf/FFI; reason + Miri + guidelines.
- M-UNSOUND: Never ship unsound safe APIs; expose `unsafe` instead if needed.

## Universal
- M-CONCISE-NAMES: No weasel words; name by responsibility.
- M-DOCUMENTED-MAGIC: Document magic values; prefer strong types.
- M-LINT-OVERRIDE-EXPECT: Use `#[expect(..., reason)]`, not `allow`.
- M-LOG-STRUCTURED: Structured logs/templates; name events; redact sensitive fields.
- M-PANIC-IS-STOP: Panic only to stop.
- M-PANIC-ON-BUG: Bugs panic, not errors.
- M-PUBLIC-DEBUG: Public types derive `Debug`.
- M-PUBLIC-DISPLAY: User-facing types implement `Display`.
- M-REGULAR-FN: Prefer free functions when not tied to state.
- M-SMALLER-CRATES: Split crates if in doubt.
- M-STATIC-VERIFICATION: Strong lints; minimal justified expects.
- M-UPSTREAM-GUIDELINES: Follow Rust API/ecosystem norms.

## Libraries / Build
- M-FEATURES-ADDITIVE: Features additive; combos work.
- M-OOBE: Crates build on Tier 1 with only Rust/Cargo.
- M-SYS-CRATES: Own native build in `build.rs`; embed sources; avoid external tools.

## Libraries / Interop
- M-DONT-LEAK-TYPES: Wrap external types; don’t expose them.
- M-ESCAPE-HATCHES: Provide explicit escape hatches.
- M-TYPES-SEND: Exported futures/types should be `Send` when safe.
- M-STRONG-TYPES: Use strong types and correct families.

## Libraries / Resilience
- M-AVOID-STATICS: Avoid `static` state; prefer instance/DI.
- M-MOCKABLE-SYSCALLS: Abstract I/O/syscalls for mocking.
- M-NO-GLOB-REEXPORTS: No `pub use *`; be explicit.
- M-TEST-UTIL: Gate test utilities via feature.

## Libraries / UX
- M-AVOID-WRAPPERS: Avoid pointer/wrapper-heavy APIs.
- M-DI-HIERARCHY: Prefer concrete types; generics over dyn.
- M-ERRORS-CANONICAL-STRUCTS: Errors are structs, not anyhow.
- M-ESSENTIAL-FN-INHERENT: Core functions are inherent.
- M-IMPL-ASREF: Accept `impl AsRef` when sensible.
- M-IMPL-IO: Design IO-agnostic interfaces where reasonable.
- M-IMPL-RANGEBOUNDS: Accept `impl RangeBounds` where useful.
- M-INIT-BUILDER: Builders for complex init.
- M-INIT-CASCADED: Cascade hierarchies to avoid combinatorial ctors.
- M-SERVICES-CLONE: Services should be cheaply `Clone`.
- M-SIMPLE-ABSTRACTIONS: Keep abstractions shallow and readable.
