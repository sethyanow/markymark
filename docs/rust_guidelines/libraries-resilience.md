# Libraries / Resilience (progressive)

**TL;DR:** Avoid statics, make system calls mockable, do not glob re-export, and gate test utilities.

**Checklist:**
- Prefer instance state over `static`; avoid hidden shared mutable state.
- Abstract I/O/system calls for mocking; provide traits or feature-gated shims.
- Avoid `pub use *`; explicitly re-export with `#[doc(inline)]` when needed.
- Gate test utilities with features (e.g., `test-util`).

## Avoid Statics (M-AVOID-STATICS) { #M-AVOID-STATICS }

Avoid `static` state in libraries; prefer instance state or dependency injection to keep code testable and multi-tenant safe.

## I/O and System Calls Are Mockable (M-MOCKABLE-SYSCALLS) { #M-MOCKABLE-SYSCALLS }

Design I/O and syscalls behind traits or adapters so tests can replace them. Provide feature-gated mocks/fakes when helpful.

## Don't Glob Re-Export Items (M-NO-GLOB-REEXPORTS) { #M-NO-GLOB-REEXPORTS }

Avoid `pub use foo::*`; it obscures API shape and documentation. Use explicit exports and `#[doc(inline)]` where appropriate.

## Test Utilities are Feature Gated (M-TEST-UTIL) { #M-TEST-UTIL }

Put test helpers behind a feature (e.g., `test-util`) to keep production deps clean and avoid shipping test-only APIs by default.

### Related
- Docs on re-exports: `docs.md#M-DOC-INLINE`
- Safety (statics, mocking implications): `safety.md`
- Original: `../rust_guidelines_full.md`
