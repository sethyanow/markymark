## Documentation — Doc Comments & AI-Friendly Docs

> **TL;DR:** Document all public items with canonical sections. First sentence ≤15 words.
> Include examples that compile. Use `#[doc(inline)]` for re-exports. Write docs that
> both humans and AI agents can understand and use.

### Canonical Doc Sections

```rust
/// Summary sentence — one line, ~15 words.
///
/// Extended description in free-form prose.
/// Explain the "why", not just the "what".
///
/// # Examples
///
/// ```
/// use my_crate::process;
/// let result = process("input")?;
/// assert_eq!(result, "output");
/// # Ok::<(), my_crate::Error>(())
/// ```
///
/// # Errors
///
/// Returns [`ProcessError::InvalidInput`] if `input` is empty.
///
/// # Panics
///
/// Panics if the global config has not been initialized.
///
/// # Safety
///
/// (Only for unsafe functions) Caller must ensure `ptr` is valid and aligned.
pub fn process(input: &str) -> Result<String, ProcessError> { /* ... */ }
```

### Section Order

1. Summary sentence (always required)
2. Extended description
3. `# Examples` (strongly encouraged)
4. `# Errors` (if returns `Result`)
5. `# Panics` (if can panic)
6. `# Safety` (if `unsafe`)
7. `# Abort` (if can abort)

### First Sentence Rule

Keep the first doc sentence to one short line (~15 words). It appears in summaries and search results.

```rust
// ✅ GOOD: Concise, scannable
/// Parses a configuration file from the given path.

// ❌ BAD: Too long, buries the key information
/// This function takes a path to a configuration file and reads it from disk,
/// parsing the contents into a Config struct.
```

### Module Documentation

```rust
//! # Storage Module
//!
//! Provides persistence for application state using SQLite.
//!
//! ## Key Types
//!
//! - [`Database`] — Connection pool and query interface
//! - [`Migration`] — Schema migration runner
//!
//! ## Usage
//!
//! ```no_run
//! use my_crate::storage::Database;
//! let db = Database::connect("sqlite:app.db").await?;
//! ```
```

### Intra-doc Links

```rust
/// Processes items from the [`Database`].
///
/// See [`Config::timeout`] for configuring the timeout.
/// Uses [`std::io::Error`] for I/O failures.
pub fn process() {}
```

### AI-Friendly Documentation Patterns

- **Use strong types** — agents rely on type signatures even more than docs
- **Include runnable examples** — agents can verify their understanding
- **Document error conditions explicitly** — agents need to handle all cases
- **Avoid ambiguity** — be precise about preconditions and postconditions
- **Use `#[doc(inline)]`** for re-exports so docs appear at the expected location

```rust
// ✅ Re-export with inline docs
#[doc(inline)]
pub use crate::config::AppConfig;

// ❌ Without inline, docs appear in the re-export section (harder to find)
// pub use crate::config::AppConfig;
```

### References

- Rust API Guidelines: [Documentation](https://rust-lang.github.io/api-guidelines/documentation.html)
- Guidelines: [M-CANONICAL-DOCS](../../docs/rust_guidelines/docs.md), [M-FIRST-DOC-SENTENCE](../../docs/rust_guidelines/docs.md), [M-MODULE-DOCS](../../docs/rust_guidelines/docs.md)
- Guidelines: [M-DESIGN-FOR-AI](../../docs/rust_guidelines/ai.md)
- Related: [tooling/testing.md](testing.md) (doc tests)
