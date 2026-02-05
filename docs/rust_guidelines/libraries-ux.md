# Libraries / UX (progressive)

<agent>
<goal>Design small, idiomatic Rust APIs that stay pleasant at scale.</goal>
<when_to_use>When shaping public library APIs: errors, inputs, constructors/builders, services/DI, and abstraction depth.</when_to_use>
<contains>M-AVOID-WRAPPERS, M-DI-HIERARCHY, M-ERRORS-CANONICAL-STRUCTS, M-ESSENTIAL-FN-INHERENT, M-IMPL-ASREF, M-IMPL-IO, M-IMPL-RANGEBOUNDS, M-INIT-BUILDER, M-INIT-CASCADED, M-SERVICES-CLONE, M-SIMPLE-ABSTRACTIONS</contains>
<see_also>applications.md, ai.md, universal.md</see_also>
<canonical>../rust_guidelines_full.md</canonical>
</agent>

**TL;DR:** Keep APIs slim and idiomatic: avoid unnecessary wrappers, prefer types over generics/dyn, use canonical error structs, keep essential funcs inherent, accept ergonomic `impl` forms, provide builders/cascaded init, make services `Clone`, and keep abstractions shallow.

**Checklist:**
- Avoid smart-pointer/wrapper-heavy signatures; expose plain types where possible.
- Prefer concrete types over generics; generics over dyn traits when needed.
- Errors: use canonical structs with backtraces; don’t use anyhow in libs.
- Keep essential functions inherent; offer ergonomic inputs (`impl AsRef`, IO, RangeBounds).
- Provide builders for complex construction; cascade init where needed.
- Make service abstractions cheaply cloneable; avoid visibly nested abstractions.

## Avoid Smart Pointers and Wrappers in APIs (M-AVOID-WRAPPERS) { #M-AVOID-WRAPPERS }

Favor straightforward types in public APIs; avoid forcing callers into `Arc`, boxes, or custom wrappers unless required.

## Prefer Types over Generics, Generics over Dyn Traits (M-DI-HIERARCHY) { #M-DI-HIERARCHY }

Prefer concrete types for clarity; if extensibility is needed, use generics before `dyn` trait objects.

## Errors are Canonical Structs (M-ERRORS-CANONICAL-STRUCTS) { #M-ERRORS-CANONICAL-STRUCTS }

Library errors should be well-defined structs (with backtrace where useful), implementing `std::error::Error`, `Display`, `Debug`. Do not use anyhow/eyre in libs.

## Essential Functionality Should be Inherent (M-ESSENTIAL-FN-INHERENT) { #M-ESSENTIAL-FN-INHERENT }

Put core behaviors on inherent impls; leave traits for extension/customization. Keep essential API discoverable on the type itself.

## Accept `impl AsRef<>` Where Feasible (M-IMPL-ASREF) { #M-IMPL-ASREF }

Take `impl AsRef<T>` for flexible inputs (paths, slices, strings) when it does not harm clarity.

## Accept `impl 'IO'` Where Feasible ('Sans IO') (M-IMPL-IO) { #M-IMPL-IO }

Design IO-agnostic APIs that accept traits/abstractions rather than concrete IO types when sensible.

## Accept `impl RangeBounds<>` Where Feasible (M-IMPL-RANGEBOUNDS) { #M-IMPL-RANGEBOUNDS }

Use `impl RangeBounds` to accept flexible ranges where appropriate.

## Complex Type Construction has Builders (M-INIT-BUILDER) { #M-INIT-BUILDER }

Provide builders for complex initialization or many optional params; keep builders minimal and chainable.

## Complex Type Initialization Hierarchies are Cascaded (M-INIT-CASCADED) { #M-INIT-CASCADED }

Use cascaded builders/initializers for hierarchical construction to avoid exponential constructor combos.

## Services are Clone (M-SERVICES-CLONE) { #M-SERVICES-CLONE }

Service abstractions should generally be cheap to `Clone` so they can be passed and reused easily.

## Abstractions Don't Visibly Nest (M-SIMPLE-ABSTRACTIONS) { #M-SIMPLE-ABSTRACTIONS }

Avoid visible nesting/wrapping layers; keep API layers shallow and comprehensible.

### Related
- Application errors: `applications.md#M-APP-ERROR`
- Builders, ranges, IO flexibility feed into UX choices in `ai.md` and `performance.md`
- Original: `../rust_guidelines_full.md`
