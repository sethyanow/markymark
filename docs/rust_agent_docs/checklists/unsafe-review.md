## Unsafe Code Review Checklist

> **TL;DR:** Use this checklist when writing or reviewing `unsafe` code.

### Safety Documentation
- [ ] Every `unsafe` block has a `// SAFETY:` comment explaining why this is sound
- [ ] Every `unsafe fn` has a `# Safety` doc section listing caller obligations
- [ ] All invariants that the unsafe code relies on are documented
- [ ] The safety reasoning does not rely on implementation details of other crates

### Memory Safety
- [ ] No null pointer dereferences possible
- [ ] No dangling pointer dereferences (use-after-free)
- [ ] No uninitialized memory reads
- [ ] No double-free possible
- [ ] All raw pointers are properly aligned before dereference
- [ ] No references to `#[repr(packed)]` struct fields (read by value instead)

### Aliasing & Concurrency
- [ ] No aliasing violations (`&mut` and `&` to same data simultaneously)
- [ ] No data races (concurrent unsynchronized access with at least one write)
- [ ] `Send`/`Sync` implementations are justified and correct
- [ ] PhantomData used correctly for variance (check [variance table](../advanced/unsafe.md))

### Type Invariants
- [ ] No invalid values constructed (e.g., `bool` must be 0 or 1)
- [ ] `Drop` implementation handles all resources correctly
- [ ] PhantomData correctly expresses ownership and variance
- [ ] `#[repr(C)]` or `#[repr(transparent)]` used where layout matters

### Testing
- [ ] Tested with Miri where possible (`cargo +nightly miri test`)
- [ ] Edge cases tested (empty input, max values, concurrent access)
- [ ] Adversarial code considered (misbehaving `Drop`, `Clone`, `Deref`)
- [ ] unsafe abstraction provides safe public API (encapsulation)

### FFI-Specific (if applicable)
- [ ] Panic safety ensured (`catch_unwind` at FFI boundaries)
- [ ] No `String`/`Vec`/`Box` crossing FFI boundary
- [ ] Opaque pointers used with create/destroy pairs

### References
- Detail: [advanced/unsafe.md](../advanced/unsafe.md)
- Nomicon: [Working with Unsafe](https://doc.rust-lang.org/nomicon/working-with-unsafe.html)
- Guidelines: [M-UNSAFE](../../docs/rust_guidelines/safety.md)
