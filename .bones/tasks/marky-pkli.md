---
id: marky-pkli
title: Create Md4cScanBackend implementing ScanBackend trait
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-6zl8]
parent: marky-0mr
---



## Goal
Create `Md4cScanBackend` struct that implements the `ScanBackend` trait using `extract_md4c()` for headings/links and delegating to existing Zig SIMD kernels for tags/block_ids/tokens. Verify it works with `DocumentIndex::from_scan()`.

## Context
- `extract_md4c()` in `markymark-kernels/src/md4c.rs` returns `Md4cExtraction { headings, links }` via FFI
- `ScanBackend` trait in `markymark-core/src/scanner.rs` has 5 methods: scan_headings, scan_links, scan_tags, scan_block_ids, estimate_tokens
- `ZigScanBackend` already implements ScanBackend using individual SIMD kernel FFI calls
- `DocumentIndex::from_scan()` at line 547 of `markymark-index/src/document/mod.rs` already accepts `&dyn ScanBackend`
- md4c covers headings + links only; tags, block_ids, and token estimation must delegate to existing Zig kernels

## Design
`Md4cScanBackend` in `markymark-core/src/scanner.rs`:
- `scan_headings()`: call `extract_md4c()`, map `Md4cHeading` → `HeadingResult`
- `scan_links()`: call `extract_md4c()`, map `Md4cLink` → `LinkResult` (is_wiki → ScanLinkType::Wiki/Markdown)
- `scan_tags()`: delegate to `markymark_kernels::scan::scan_tags()` (same as ZigScanBackend)
- `scan_block_ids()`: delegate to `markymark_kernels::scan::scan_block_ids()` (same as ZigScanBackend)
- `estimate_tokens()`: delegate to `markymark_kernels::tokens::estimate_tokens()` (same as ZigScanBackend)

Note: scan_headings and scan_links each call extract_md4c() independently (double-parse). At ~200MB/s, a 50KB doc takes ~0.25ms × 2 = 0.5ms, still 25x faster than tree-sitter 12.8ms. Acceptable trade-off for simplicity.

## Implementation Steps
1. Add `Md4cScanBackend` struct to scanner.rs (behind zig-kernels feature gate)
2. Implement all 5 ScanBackend methods
3. Add unit tests for Md4cScanBackend in scanner.rs tests
4. Add integration tests in document/tests.rs using `from_scan()` with Md4cScanBackend
5. Verify all existing tests still pass

## Success Criteria
- [ ] `Md4cScanBackend` implements `ScanBackend` with correct type mappings
- [ ] Unit tests verify headings, links (markdown + wiki), tags, block_ids
- [ ] Integration test: `DocumentIndex::from_scan()` with `Md4cScanBackend` produces correct index
- [ ] All existing tests pass (no regression)
- [ ] Entity non-decoding documented (known limitation from marky-yfh7)

## NOT in scope (follow-up task)
- Wiring into LSP did_change pipeline
- Lazy tree-sitter for hover/goto-def
- Benchmarking
- Incremental indexing with md4c

## Design

## Goal
Create `Md4cScanBackend` struct that implements the `ScanBackend` trait using `extract_md4c()` for headings/links and delegating to existing Zig SIMD kernels for tags/block_ids/tokens. Verify it works with `DocumentIndex::from_scan()`.

## Context
- `extract_md4c()` in `markymark-kernels/src/md4c.rs` returns `Md4cExtraction { headings, links }` via FFI
- `ScanBackend` trait in `markymark-core/src/scanner.rs` has 5 methods: scan_headings, scan_links, scan_tags, scan_block_ids, estimate_tokens
- `ZigScanBackend` already implements ScanBackend using individual SIMD kernel FFI calls
- `DocumentIndex::from_scan()` at line 547 of `markymark-index/src/document/mod.rs` already accepts `&dyn ScanBackend`
- md4c covers headings + links only; tags, block_ids, and token estimation must delegate to existing Zig kernels
- `markymark-core` depends on `markymark-kernels` behind `zig-kernels` feature gate

## Design
`Md4cScanBackend` in `markymark-core/src/scanner.rs` (behind `#[cfg(feature = "zig-kernels")]`):

**Struct:** Zero-sized type (like ZigScanBackend): `#[derive(Debug, Clone, Copy, Default)] pub struct Md4cScanBackend;`

**Method implementations:**
- `scan_headings(&self, text)`: call `markymark_kernels::md4c::extract_md4c(text)`, map each `Md4cHeading` → `HeadingResult { text: h.text, offset: h.source_offset, level: h.level }`
- `scan_links(&self, text)`: call `markymark_kernels::md4c::extract_md4c(text)`, map each `Md4cLink` → `LinkResult { offset: l.source_offset, text: l.text, target: l.target, link_type: if l.is_wiki { Wiki } else { Markdown } }`
- `scan_tags(&self, text)`: delegate to `markymark_kernels::scan::scan_tags(text)` (identical to ZigScanBackend)
- `scan_block_ids(&self, text)`: delegate to `markymark_kernels::scan::scan_block_ids(text)` (identical to ZigScanBackend)
- `estimate_tokens(&self, text)`: delegate to `markymark_kernels::tokens::estimate_tokens(text)` (identical to ZigScanBackend)

**Error handling:** Map `KernelError` → `ScanError::InternalError(e.to_string())` (same pattern as ZigScanBackend).

**Double-parse note:** scan_headings and scan_links each call extract_md4c() independently. At ~200MB/s, 50KB × 2 = 0.5ms, still 25x faster than tree-sitter 12.8ms. Acceptable trade-off for trait simplicity.

## Implementation Steps
1. Add `Md4cScanBackend` struct to `markymark-core/src/scanner.rs` after ZigScanBackend (line ~195), behind `#[cfg(feature = "zig-kernels")]`
2. Implement `ScanBackend for Md4cScanBackend` — 5 methods with type mapping
3. Add unit tests in `markymark-core/src/scanner.rs` test module (after existing ZigScanBackend tests):
   - test_md4c_scan_backend_send_sync: verifies Send+Sync (catches !Send FFI leakage)
   - test_md4c_scan_backend_trait_object: verifies dyn-compatible (catches object-safety regression)
   - test_md4c_scan_headings_basic: "# Hello" → HeadingResult{text:"Hello", offset:0, level:1} (catches type mapping bugs)
   - test_md4c_scan_links_markdown: "[click](url)" → LinkResult{link_type: Markdown} (catches link type discriminant bug)
   - test_md4c_scan_links_wiki: "[[Target]]" → LinkResult{link_type: Wiki} (catches is_wiki → ScanLinkType mapping)
   - test_md4c_scan_empty_input: "" → Ok(empty vecs) (catches null/error on empty)
   - test_md4c_scan_entity_not_decoded: "# Hello &amp; World" → text contains "&amp;" (documents marky-yfh7 limitation)
4. Add integration tests in `markymark-index/src/document/tests.rs` scan_tests module:
   - build_index_from_md4c_scan() helper using Md4cScanBackend
   - test_md4c_from_scan_single_heading: heading text, level, slug correct (catches from_scan integration bug)
   - test_md4c_from_scan_mixed_links: wiki + markdown links split correctly in index (catches link routing bug in from_scan)
   - test_md4c_parity_headings: md4c vs zig scan same heading count/text/level (catches divergent extraction)
5. Run: `cargo nextest -p markymark-core` and `cargo nextest -p markymark-index` (verify green)
6. Run: `cargo clippy --workspace --all-targets` (verify clean)

## Success Criteria
- [ ] `Md4cScanBackend` implements `ScanBackend` — compiles with correct type mappings
- [ ] `Md4cScanBackend` is Send + Sync (verified by compile-time test)
- [ ] `Md4cScanBackend` is dyn-compatible (verified by trait object test)
- [ ] 7 unit tests pass in scanner.rs (headings, markdown links, wiki links, empty, entity, send_sync, trait_object)
- [ ] 3 integration tests pass in document/tests.rs (single heading, mixed links, parity)
- [ ] All existing tests pass: `cargo nextest --workspace` returns 0 failures
- [ ] Clippy clean: `cargo clippy --workspace --all-targets` returns 0 warnings
- [ ] Entity non-decoding documented in test comment (known limitation marky-yfh7)

## Anti-patterns
- No `unwrap()` or `expect()` in production code — use `map_err` for error conversion
- No `todo!()` or `unimplemented!()` — all 5 methods must have real implementations
- No duplicated delegation logic — tags/block_ids/tokens delegation should follow ZigScanBackend pattern exactly
- Do NOT add caching/interior mutability to handle double-parse — accept it for simplicity

## Key Considerations (SRE Review)

**Edge Case: Empty input**
- `extract_md4c("")` returns `Ok(Md4cExtraction { headings: [], links: [] })` — verified in existing md4c.rs tests
- Test `test_md4c_scan_empty_input` verifies this flows through ScanBackend correctly

**Edge Case: Entity references**
- md4c ExtractionRenderer does NOT decode HTML entities (marky-yfh7)
- "# Hello &amp; World" → heading text is "Hello &amp; World" not "Hello & World"
- Test `test_md4c_scan_entity_not_decoded` documents this known limitation
- This differs from what tree-sitter AST produces — parity test should check heading text but allow entity differences

**Edge Case: Reference links**
- md4c resolves reference link definitions: `[text][ref]` with `[ref]: url` → target is "url"
- ZigScanBackend regex-based extraction may not resolve references
- Parity test should document differences rather than require exact match

**Edge Case: Autolinks**
- md4c detects autolinks (`<https://example.com>`) and reports them as links
- ZigScanBackend may handle these differently
- Document in parity test comments

**Error propagation:**
- `extract_md4c()` returns `Result<Md4cExtraction, KernelError>`
- On error, map to `ScanError::InternalError(e.to_string())` — same pattern as ZigScanBackend
- `from_scan()` calls `unwrap_or_default()` on backend results — errors silently produce empty results
- This is the existing behavior and acceptable for now

## NOT in scope (follow-up task)
- Wiring into LSP did_change pipeline
- Lazy tree-sitter for hover/goto-def
- Benchmarking
- Incremental indexing with md4c
