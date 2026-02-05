# Libraries / Interoperability (progressive)

**TL;DR:** Don’t leak external types, provide escape hatches, ensure exported types are `Send` where appropriate, and choose the correct type family/strong types for interfaces.

**Checklist:**
- Keep public surfaces free of external crate types; wrap them.
- Offer native escape hatches for power users.
- Make exported types `Send` (futures, regular types) when safe and expected.
- Choose the proper type family and use strong/new types for semantics.

## Don't Leak External Types (M-DONT-LEAK-TYPES) { #M-DONT-LEAK-TYPES }

Avoid exposing third-party types in public APIs; wrap them to keep control, compatibility, and swap options.

## Native Escape Hatches (M-ESCAPE-HATCHES) { #M-ESCAPE-HATCHES }

Provide explicit escape hatches (e.g., raw handles, lower-level accessors) so advanced users can bypass abstractions without forking.

## Types are Send (M-TYPES-SEND) { #M-TYPES-SEND }

Futures and regular types that cross threads should implement `Send` when safe and expected. Audit captures and shared state accordingly.

![TEXT](../M-TYPES-SEND.png)

## Use the Proper Type Family (M-STRONG-TYPES) { #M-STRONG-TYPES }

Prefer strong types over primitives to encode semantics and avoid confusion; pick the right type family for OS/arch correctness (e.g., paths, ranges, time).

### Related
- Resilience (statics/mocking): `libraries-resilience.md`
- UX (API ergonomics): `libraries-ux.md`
- Original: `../rust_guidelines_full.md`
