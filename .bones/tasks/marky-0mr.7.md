---
id: marky-0mr.7
title: 'PR#39 review: scanner double-parse optimization'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


**T2-10: scan_headings and scan_links both independently call extract_md4c — double parse**
File: markymark-core/src/scanner.rs:212-247

When a caller needs both headings and links (the common case for indexing), the document is parsed twice by the md4c backend.

Fix options:
1. Add a private helper that caches the extraction result (interior mutability with RefCell)
2. Provide a combined scan_all method returning both headings and links from one parse
3. Document that callers should use extract_md4c directly if they need both

Option 2 is cleanest — add scan_all() that calls extract_md4c once and splits the results.

Source: PR #39 review — CodeRabbit

## Design

## Goal
Eliminate the double-parse in `Md4cScanBackend` when a caller needs both headings and links.
The only real call site (`document/mod.rs:560-561`) calls `scan_headings` then `scan_links`
on the same text — causing `extract_md4c` to run twice. Add `scan_all()` to the `ScanBackend`
trait with a default impl (calls both separately, correct for ZigScanBackend), and override it
in `Md4cScanBackend` to call `extract_md4c` once and split the result.

## Effort Estimate
3-4 hours.

## Files to Modify
- `markymark-core/src/scanner.rs` — add `ScanAllResult`, add `scan_all` to trait + default impl, override in `Md4cScanBackend`
- `markymark-index/src/document/mod.rs:560-561` — replace two calls with one `scan_all` call
- `markymark-core/src/scanner.rs` (test module) — add tests for `scan_all`

## New Type

In `markymark-core/src/scanner.rs` (above the `ScanBackend` trait):

```rust
/// Combined result from a single-pass scan of headings and links.
#[derive(Debug, Default)]
pub struct ScanAllResult {
    pub headings: Vec<HeadingResult>,
    pub links: Vec<LinkResult>,
}
```

`#[derive(Default)]` is required — the call site uses `.unwrap_or_default()`.

## Trait Change

Add to `ScanBackend` after `scan_links`:

```rust
/// Scan text for headings and links in a single pass.
///
/// The default implementation calls [`scan_headings`] and [`scan_links`]
/// separately. Backends that parse once internally (e.g., [`Md4cScanBackend`])
/// should override this to avoid a second parse.
fn scan_all(&self, text: &str) -> Result<ScanAllResult, ScanError> {
    Ok(ScanAllResult {
        headings: self.scan_headings(text)?,
        links: self.scan_links(text)?,
    })
}
```

The default impl is correct for `ZigScanBackend` (two separate SIMD passes — no shared
parse state). Do NOT override `scan_all` in `ZigScanBackend`.

## Md4cScanBackend Override

Add after the existing `scan_links` impl:

```rust
fn scan_all(&self, text: &str) -> Result<ScanAllResult, ScanError> {
    markymark_kernels::md4c::extract_md4c(text)
        .map(|extraction| ScanAllResult {
            headings: extraction.headings.into_iter().map(|h| HeadingResult {
                text: h.text,
                offset: h.source_offset,
                level: h.level,
            }).collect(),
            links: extraction.links.into_iter().map(|l| LinkResult {
                offset: l.source_offset,
                text: l.text,
                target: l.target,
                link_type: if l.is_wiki { ScanLinkType::Wiki } else { ScanLinkType::Markdown },
            }).collect(),
        })
        .map_err(|e| ScanError::InternalError(e.to_string()))
}
```

## Call Site Update

`markymark-index/src/document/mod.rs:560-561`, replace:
```rust
let scan_headings = backend.scan_headings(text).unwrap_or_default();
let scan_links = backend.scan_links(text).unwrap_or_default();
```
with:
```rust
let ScanAllResult { headings: scan_headings, links: scan_links } =
    backend.scan_all(text).unwrap_or_default();
```

Import: add `use markymark_core::scanner::ScanAllResult;` at the top of the file if not already imported via the prelude.

## Implementation Checklist

- [ ] Add `ScanAllResult` struct above `ScanBackend` trait in scanner.rs
- [ ] Add `scan_all` default method to `ScanBackend` trait (as shown above)
- [ ] Add `scan_all` override to `Md4cScanBackend` impl
- [ ] Confirm `ZigScanBackend` does NOT override `scan_all` (default is correct)
- [ ] Update `document/mod.rs:560-561` to use `scan_all`
- [ ] Add `use markymark_core::scanner::ScanAllResult` to document/mod.rs if needed
- [ ] Export `ScanAllResult` from `markymark-core` public API (add to lib.rs re-exports if missing)
- [ ] Add tests (see below)
- [ ] Run `cargo nextest -p markymark-core -p markymark-index` — all pass
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings
- [ ] Verify `scanner.rs` line count did not grow beyond 1000

## Success Criteria

- [ ] `ScanAllResult` type exists and is `pub` in `markymark-core::scanner`
- [ ] `ScanBackend::scan_all` has a default implementation that delegates to `scan_headings` + `scan_links`
- [ ] `Md4cScanBackend` overrides `scan_all` (confirmed by searching for `fn scan_all` under the impl block)
- [ ] `ZigScanBackend` does NOT override `scan_all` (grep check)
- [ ] `document/mod.rs:560-561` is replaced with `scan_all` call (grep for old two-liner is empty)
- [ ] 5+ tests pass (see Test Specs)
- [ ] `cargo nextest -p markymark-core` exits 0
- [ ] `cargo nextest -p markymark-index` exits 0
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0

## Test Specifications

Add to the existing `#[cfg(test)]` block in `scanner.rs`:

- `test_scan_all_headings_match_scan_headings` — call `scan_all` and `scan_headings` separately on the same input; assert `scan_all.headings == scan_headings()`. Catches: override divergence from the individual method.
- `test_scan_all_links_match_scan_links` — same pattern for links. Catches: link extraction differs between combined and separate paths.
- `test_scan_all_empty_text_returns_default` — call `scan_all("")` on both backends; assert both `headings` and `links` are empty. Catches: unwrap_or_default assumption.
- `test_scan_all_combined_doc` — text with both headings and links; assert `headings.len() > 0 && links.len() > 0`. Catches: one field silently zeroed in override.
- `test_md4c_scan_all_consistent_with_separate` — specifically for `Md4cScanBackend`: compare `scan_all()` result against separately called `scan_headings()` + `scan_links()` on a 3-heading, 3-link document. This directly verifies the override produces identical output to the old code path.

## Key Considerations

**ZigScanBackend default is correct:**
Zig SIMD passes are independent — `scan_tags` is already a separate pass. Two passes for
headings + links is the existing behavior and is correct. Do not force a combined Zig path.

**ScanAllResult must derive Default:**
`document/mod.rs` uses `.unwrap_or_default()` on both calls today. The new `scan_all` call
must also use `.unwrap_or_default()`. `ScanAllResult` needs `#[derive(Default)]` to support this,
which requires `Vec<HeadingResult>` and `Vec<LinkResult>` to be Default (they are).

**scan_tags and scan_block_ids are NOT part of this change:**
`document/mod.rs:562-563` calls `scan_tags` and `scan_block_ids` separately — leave those alone.
This task is specifically heading+link double-parse elimination.

**Do not change error handling at the call site:**
The existing call site swallows errors with `.unwrap_or_default()`. Changing that behavior is out
of scope. Preserve it exactly.

**Verify the override mapping matches the individual methods:**
`Md4cScanBackend::scan_headings` maps `h.source_offset` → `offset`. The override must use the
same field. Cross-check against the existing `scan_headings` impl before writing the override.
The test `test_md4c_scan_all_consistent_with_separate` will catch any divergence.

## Anti-patterns

- ❌ No `unwrap`/`expect` in production code (the mapping is infallible, use `map`/`map_err`)
- ❌ Do NOT override `scan_all` in `ZigScanBackend` — default impl is correct
- ❌ Do NOT add `scan_all` to the `MockScanBackend` in tests — the default impl should propagate through automatically
- ❌ Do NOT change the `.unwrap_or_default()` error handling at the call site — behavioral preservation only
- ❌ Do NOT rename existing `scan_headings`/`scan_links` methods — they remain on the trait for callers that only need one
- ❌ No TODOs without issue numbers
