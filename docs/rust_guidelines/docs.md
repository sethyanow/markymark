# Documentation Guidelines (progressive)

**TL;DR:** Use canonical doc sections, inline re-exports with `#[doc(inline)]`, keep first sentences short, and ensure modules are documented.

**Checklist:**
- Follow canonical sections on public items; first sentence ~15 words.
- Inline `pub use` items with `#[doc(inline)]`; avoid glob exports.
- Add module-level docs; include examples and related reading.
- Keep re-export docs clear; use canonical sections not parameter tables.

## Documentation Has Canonical Sections (M-CANONICAL-DOCS) { #M-CANONICAL-DOCS }

<why>To follow established and expected Rust best practices.</why>
<version>1.0</version>

Public library items must contain the canonical doc sections. The summary sentence must always be present. Extended documentation and examples are strongly encouraged. The other sections must be present when applicable.

```rust
/// Summary sentence < 15 words.
///
/// Extended documentation in free form.
///
/// # Examples
/// One or more examples that show API usage like so.
///
/// # Errors
/// If fn returns `Result`, list known error conditions
///
/// # Panics
/// If fn may panic, list when this may happen
///
/// # Safety
/// If fn is `unsafe` or may otherwise cause UB, this section must list
/// all conditions a caller must uphold.
///
/// # Abort
/// If fn may abort the process, list when this may happen.
pub fn foo() {}
```

Avoid parameter tables; explain parameters inline:

```rust,ignore
/// Copies a file from `src` to `dst`.
fn copy(src: File, dst: File) {}
```

### Related Reading
- Function docs include error, panic, and safety considerations ([C-FAILURE](https://rust-lang.github.io/api-guidelines/documentation.html#c-failure))

## Mark `pub use` Items with `#[doc(inline)]` (M-DOC-INLINE) { #M-DOC-INLINE }

<why>To make re-exported items 'fit in' with their non re-exported siblings.</why>
<version>1.0</version>

When publicly re-exporting crate items via `pub use foo::Foo` or `pub use foo::*`, they show up in an opaque re-export block. In most cases, this is not helpful to the reader:

![TEXT](../M-DOC-INLINE_BAD.png)

Instead, annotate them with `#[doc(inline)]` at the `use` site, for them to be inlined organically:

```rust,edition2021,ignore
# pub(crate) mod foo { pub struct Foo; }
#[doc(inline)]
pub use foo::*;

// or

#[doc(inline)]
pub use foo::Foo;
```

![TEXT](../M-DOC-INLINE_GOOD.png)

This does not apply to `std` or 3rd party types; these should always be re-exported without inlining to make it clear they are external.

> ### <alert></alert> Still avoid glob exports
>
> The `#[doc(inline)]` trick above does not change [M-NO-GLOB-REEXPORTS](./libraries-resilience.md#M-NO-GLOB-REEXPORTS); you generally should not re-export items via wildcards.

## First Sentence is One Line; Approx. 15 Words (M-FIRST-DOC-SENTENCE) { #M-FIRST-DOC-SENTENCE }

<why>To make API docs easily skimmable.</why>
<version>1.0</version>

The first sentence of your docs should be one line (~15 words) to aid scanning.

![TEXT](../M-FIRST-DOC-SENTENCE_GOOD.png)

![TEXT](../M-FIRST-DOC-SENTENCE_BAD.png)

## Has Comprehensive Module Documentation (M-MODULE-DOCS) { #M-MODULE-DOCS }

<why>To make navigating larger Rust crates much easier.</why>
<version>1.0</version>

Modules should include comprehensive docs describing purpose, key types, and how to extend or use them. Include examples and links to related modules or APIs where helpful.

### Related
- Glob exports: `libraries-resilience.md#M-NO-GLOB-REEXPORTS`
- AI guidance: `ai.md`
- Original: `../rust_guidelines_full.md`
