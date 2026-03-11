---
id: marky-7vey
title: 'scanner.rs: factor Md4cScanBackend heading/link conversion into private helpers'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal
Eliminate mapping duplication in `Md4cScanBackend` by extracting two private
conversion helpers shared by `scan_headings`, `scan_links`, and `scan_all`.
Pure refactor — no behavior change.

## Context
`markymark-core/src/scanner.rs` — the `Md4cScanBackend` impl currently has
three methods that each inline identical struct-field mappings:
- `scan_headings` (lines 233-247): maps `Md4cHeading` → `HeadingResult`
- `scan_links` (lines 249-267): maps `Md4cLink` → `LinkResult`
- `scan_all` (lines 302-330): inlines BOTH mappings again

If `Md4cHeading` or `Md4cLink` fields change (e.g., `source_offset` renamed),
the fix must be applied in three places instead of one. CodeRabbit finding from
PR #41 round 2/3 review.

## Source Types (from markymark-kernels/src/md4c.rs)
```
Md4cHeading { text: String, source_offset: u32, level: u8 }
Md4cLink    { text: String, target: String, source_offset: u32, is_wiki: bool }
```

## Target Types (from markymark-core/src/scanner.rs)
```
HeadingResult { text: String, offset: u32, level: u8 }
LinkResult    { offset: u32, text: String, target: String, link_type: ScanLinkType }
```

## Implementation

**File:** `markymark-core/src/scanner.rs`

**Step 1:** Add two private helper functions immediately after the closing `}` of
`impl ScanBackend for Md4cScanBackend` (after line 331), inside the
`#[cfg(feature = "zig-kernels")]` gate:

```rust
#[cfg(feature = "zig-kernels")]
#[inline]
fn map_md4c_heading(h: markymark_kernels::md4c::Md4cHeading) -> HeadingResult {
    HeadingResult {
        text: h.text,
        offset: h.source_offset,
        level: h.level,
    }
}

#[cfg(feature = "zig-kernels")]
#[inline]
fn map_md4c_link(l: markymark_kernels::md4c::Md4cLink) -> LinkResult {
    LinkResult {
        offset: l.source_offset,
        text: l.text,
        target: l.target,
        link_type: if l.is_wiki {
            ScanLinkType::Wiki
        } else {
            ScanLinkType::Markdown
        },
    }
}
```

**Step 2:** Update `scan_headings` (lines 233-247) to:
```rust
fn scan_headings(&self, text: &str) -> Result<Vec<HeadingResult>, ScanError> {
    markymark_kernels::md4c::extract_md4c(text)
        .map(|extraction| extraction.headings.into_iter().map(map_md4c_heading).collect())
        .map_err(|e| ScanError::InternalError(e.to_string()))
}
```

**Step 3:** Update `scan_links` (lines 249-267) to:
```rust
fn scan_links(&self, text: &str) -> Result<Vec<LinkResult>, ScanError> {
    markymark_kernels::md4c::extract_md4c(text)
        .map(|extraction| extraction.links.into_iter().map(map_md4c_link).collect())
        .map_err(|e| ScanError::InternalError(e.to_string()))
}
```

**Step 4:** Update `scan_all` (lines 302-330) to:
```rust
fn scan_all(&self, text: &str) -> Result<ScanAllResult, ScanError> {
    markymark_kernels::md4c::extract_md4c(text)
        .map(|extraction| ScanAllResult {
            headings: extraction.headings.into_iter().map(map_md4c_heading).collect(),
            links: extraction.links.into_iter().map(map_md4c_link).collect(),
        })
        .map_err(|e| ScanError::InternalError(e.to_string()))
}
```

## Effort Estimate
~1 hour (30 min edit + 30 min verify)

## Success Criteria
- [ ] `cargo nextest -p markymark-core` passes — all existing md4c_tests pass unchanged
- [ ] `cargo clippy -p markymark-core --all-targets` — no warnings
- [ ] `cargo nextest -p markymark-core -- md4c_tests::test_scan_all_consistent_with_separate` passes
- [ ] `cargo nextest -p markymark-core -- md4c_tests::test_md4c_scan_headings_basic` passes
- [ ] `cargo nextest -p markymark-core -- md4c_tests::test_md4c_scan_links_markdown` passes
- [ ] Diff shows no lines changed outside scanner.rs — zero behavior change

## Key Considerations

**cfg gate MUST match:** Both helpers are only compiled when `zig-kernels` feature
is active — same gate as the `impl ScanBackend for Md4cScanBackend` block.
Missing the cfg will produce "unused function" warnings or compile errors when
the feature is off. Use exactly `#[cfg(feature = "zig-kernels")]` on each helper.

**No new tests needed:** This is a pure refactor. The existing test suite in
`md4c_tests` (lines 474+) already covers `scan_headings`, `scan_links`, and
`scan_all` with real md4c extraction. The correctness guarantee is:
"all existing tests pass" = behavior is identical.

**scan_all calls extract_md4c once:** The refactored `scan_all` still calls
`extract_md4c` exactly once and maps both result vectors — same as before.
Do NOT change this to call `scan_headings` + `scan_links` (that would be two
FFI calls instead of one, doubling FFI overhead for scan_all).

**`#[inline]` rationale:** Both helpers are simple struct constructors.
`#[inline]` lets the compiler eliminate the call overhead when used in
`.map()` chains — same performance as the original inlined code.

## Anti-patterns
- ❌ Do NOT remove the `#[cfg(feature = "zig-kernels")]` gates on helpers
- ❌ Do NOT make helpers `pub` — they are internal conversion details
- ❌ Do NOT implement `From<Md4cHeading> for HeadingResult` — that would
  add a trait impl to a foreign type (violates Rust orphan rules) or pollute
  the public API of markymark-core
- ❌ Do NOT change scan_all to call scan_headings + scan_links separately
