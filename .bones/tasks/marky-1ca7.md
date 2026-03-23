---
id: marky-1ca7
title: 'from_blob.rs: implement Display + Error for BlobError'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal
Implement `std::fmt::Display` and `std::error::Error` for `BlobError` so it
integrates with standard Rust error handling — `?` propagation into
`Box<dyn Error>`, `anyhow`, and `thiserror` contexts, and human-readable
error messages in logs.

## Context
`markymark-index/src/document/from_blob.rs`, lines 52-67.
`BlobError` is `#[derive(Debug, Clone, PartialEq, Eq)]` but missing `Display`
and `Error`. Without `Display`, `format!("{}", err)` fails. Without `Error`,
the type cannot be used as `Box<dyn std::error::Error>` or propagated with `?`
in functions returning `Result<_, Box<dyn Error>>`.

CodeRabbit PR #41 round 2/3 finding. Low risk, pure addition.

## BlobError Variants (current, lines 54-67)
```
TooSmall           — Data shorter than 64-byte minimum header
InvalidMagic       — Magic bytes ≠ 0x4D4B5343 ("MKSC")
UnsupportedVersion — Version field ≠ 1
SizeMismatch       — Header total_blob_size ≠ actual data length or computed size
TextPoolOutOfBounds — offset+length pair exceeds text_pool slice
InvalidUtf8        — Text pool bytes fail UTF-8 validation
```

## Implementation

**File:** `markymark-index/src/document/from_blob.rs`

**Insert after line 67** (after the closing `}` of the `BlobError` enum),
before the `// Low-level byte readers` comment:

```rust
impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooSmall => write!(f, "blob too small (minimum 64 bytes required for header)"),
            Self::InvalidMagic => {
                write!(f, "invalid blob magic (expected MKSC / 0x4D4B5343)")
            }
            Self::UnsupportedVersion => {
                write!(f, "unsupported blob version (only version 1 is supported)")
            }
            Self::SizeMismatch => write!(
                f,
                "blob size mismatch (header total_blob_size differs from actual or computed size)"
            ),
            Self::TextPoolOutOfBounds => {
                write!(f, "text pool offset + length exceeds text pool bounds")
            }
            Self::InvalidUtf8 => write!(f, "text pool contains invalid UTF-8 bytes"),
        }
    }
}

impl std::error::Error for BlobError {}
```

No `source()` override needed — `BlobError` variants have no wrapped errors.
No `Cargo.toml` changes needed — these are `std` traits.

## Effort Estimate
~45 minutes (15 min code + 30 min tests)

## Success Criteria
- [ ] `cargo nextest -p markymark-index -- from_blob` passes — all 18+ existing tests pass
- [ ] `cargo clippy -p markymark-index --all-targets` — no warnings
- [ ] New test `test_blob_error_display_messages` passes (see below)
- [ ] New test `test_blob_error_is_std_error` compiles and passes
- [ ] `format!("{}", BlobError::TooSmall)` produces a non-empty human-readable string

## Tests to Add

Add inside the existing `#[cfg(test)] mod tests` block
(markymark-index/src/document/from_blob.rs, near line 656):

```rust
#[test]
fn test_blob_error_display_messages() {
    // Each variant must produce a non-empty, distinct human-readable message.
    let cases: &[(BlobError, &str)] = &[
        (BlobError::TooSmall, "64 bytes"),
        (BlobError::InvalidMagic, "MKSC"),
        (BlobError::UnsupportedVersion, "version 1"),
        (BlobError::SizeMismatch, "size mismatch"),
        (BlobError::TextPoolOutOfBounds, "text pool"),
        (BlobError::InvalidUtf8, "UTF-8"),
    ];
    for (err, expected_substr) in cases {
        let msg = format!("{}", err);
        assert!(
            msg.contains(expected_substr),
            "Display for {err:?} = {msg:?}; expected to contain {expected_substr:?}"
        );
    }
}

#[test]
fn test_blob_error_is_std_error() {
    // BlobError must be usable as Box<dyn std::error::Error>.
    fn accepts_error(_: &dyn std::error::Error) {}
    accepts_error(&BlobError::InvalidMagic);

    // Must be usable with ? in Box<dyn Error> context.
    fn returns_box_err() -> Result<(), Box<dyn std::error::Error>> {
        let data: &[u8] = &[0u8; 4]; // too small
        DocumentIndex::from_blob(data)?; // should propagate BlobError::TooSmall
        Ok(())
    }
    assert!(returns_box_err().is_err());
}

#[test]
fn test_blob_error_display_all_variants_distinct() {
    // All 6 variant messages must be distinct (catch copy-paste errors).
    use std::collections::HashSet;
    let messages: HashSet<String> = [
        BlobError::TooSmall,
        BlobError::InvalidMagic,
        BlobError::UnsupportedVersion,
        BlobError::SizeMismatch,
        BlobError::TextPoolOutOfBounds,
        BlobError::InvalidUtf8,
    ]
    .iter()
    .map(|e| format!("{e}"))
    .collect();
    assert_eq!(messages.len(), 6, "All BlobError variants must have distinct Display messages");
}
```

## Key Considerations

**No `source()` override.** `BlobError` wraps no inner errors — all variants
are terminal. The default `source()` returning `None` is correct.

**Message content guidelines:**
- Include the concrete expected value (e.g., "64 bytes", "MKSC", "version 1")
  so users can immediately identify what was wrong vs. what was expected.
- Avoid mentioning internal field names (e.g., "total_blob_size") in user-facing
  messages — use semantic names ("blob size") instead.
- Keep messages lowercase (Rust Error convention for Display messages).

**`impl std::error::Error for BlobError {}` is intentionally empty.** The
default impl provides `source()` → `None` and the deprecated `description()`.
No override is needed or desired here.

**`BlobError` already derives `Debug`.** The test can use `{err:?}` for debug
output in assertion messages. Do not remove the `Debug` derive.

**No import changes.** `std::fmt` and `std::error::Error` are both in `std`
which is always available. No `use` statements needed.

**Existing tests unaffected.** All 18+ existing tests in `mod tests` test
`from_blob`/`from_blob_with_xml_tags` behavior — they don't test `Display`.
The new tests are purely additive.

## Anti-patterns
- ❌ Do NOT use `thiserror` — adds an unnecessary dependency for 6 simple match arms
- ❌ Do NOT implement `From<BlobError> for anyhow::Error` — that's handled
  automatically once `std::error::Error` is implemented
- ❌ Do NOT use the deprecated `fn description()` method — implement `Display`
  only (the modern approach)
- ❌ Do NOT add `unwrap()`/`expect()` in new tests — use `assert!` + `is_err()`
