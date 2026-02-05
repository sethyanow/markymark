# Safety Guidelines (progressive)

**TL;DR:** Reserve `unsafe` for cases where misuse risks UB; avoid ad-hoc unsafe; prove soundness; never ship unsound code.

**Checklist:**
- Only mark items `unsafe` when misuse can cause UB.
- Use `unsafe` only for novel abstractions, performance (after benchmarking), or FFI/platform calls.
- Provide plain-text safety reasoning; run Miri; follow unsafe code guidelines.
- Harden unsafe abstractions against adversarial code; document allowed call patterns.
- Never ship unsound code; if you cannot make it sound, expose `unsafe` APIs with docs.

## Unsafe Implies Undefined Behavior (M-UNSAFE-IMPLIES-UB) { #M-UNSAFE-IMPLIES-UB }

<why>To ensure semantic consistency and prevent warning fatigue.</why>
<version>1.0</version>

`unsafe` may only be applied if misuse risks undefined behavior (UB). Do not use it merely for dangerous-but-safe operations.

```rust
// Valid use of unsafe
unsafe fn print_string(x: *const String) { }

// Invalid use of unsafe
unsafe fn delete_database() { }
```

## Unsafe Needs Reason, Should be Avoided (M-UNSAFE) { #M-UNSAFE }

<why>To prevent undefined behavior, attack surface, and similar 'happy little accidents'.</why>
<version>0.2</version>

Valid reasons for `unsafe`:
1) novel abstractions (e.g., new smart pointer or allocator),
1) performance (after benchmarking), e.g., `.get_unchecked()`,
1) FFI/platform calls.

Avoid ad-hoc unsafe to shorten code, bypass `Send`, or bypass lifetimes. Follow these patterns:

### Novel Abstractions
- Verify no established alternative; keep minimal and testable.
- Harden against adversarial code (panic poisoning, misbehaving `Deref`/`Clone`/`Drop`).
- Provide plain-text safety reasoning.
- Pass [Miri](https://github.com/rust-lang/miri); follow [unsafe code guidelines](https://rust-lang.github.io/unsafe-code-guidelines/).

### Performance
- Benchmark first; only then consider unsafe.
- Add plain-text safety reasoning; cover `_unchecked` calls.
- Pass Miri; follow unsafe code guidelines.

### FFI
- Prefer established interop libs.
- Follow unsafe code guidelines.
- Document generated bindings and permissible call patterns.

### Further Reading
- [Nomicon](https://doc.rust-lang.org/nightly/nomicon/)
- [Unsafe Code Guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)
- [Miri](https://github.com/rust-lang/miri)
- ["Adversarial code"](https://cheats.rs/#adversarial-code)

## All Code Must be Sound (M-UNSOUND) { #M-UNSOUND }

<why>To prevent unexpected runtime behavior, leading to potential bugs and incompatibilities.</why>
<version>1.0</version>

Unsound code is seemingly safe code that can cause UB. Unsound abstractions are never permissible—expose `unsafe` APIs instead and document correct use.

> No exceptions: unsound code is never acceptable.

Soundness boundaries follow module boundaries: safe functions may rely on invariants established elsewhere in the same module, but must not appear safe while enabling UB.

### Related
- FFI constraints: `ffi.md`
- Performance yield points: `performance.md`
- Universal panic/logging posture: `universal.md`
- Original: `../rust_guidelines_full.md`
