---
id: marky-h1t
title: Write Rust FFI wrappers for extraction kernels
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-v8r, marky-yo7, marky-50d, marky-59i]
---








In markymark-kernels/src/, implement: scan.rs (scan_headings, scan_links, scan_tags, scan_block_ids — each returns Vec of typed scan results), tokens.rs (estimate_tokens), hash.rs (content_hash, add to existing). All wrappers: validate inputs, allocate output buffers, handle error codes, convert C structs to Rust types. Rust tests for each function.

## Design

## Goal
Implement safe Rust FFI wrappers in markymark-kernels/src/ for all extraction kernels: scan.rs (scan_headings, scan_links, scan_tags, scan_block_ids returning Vec of typed results), tokens.rs (estimate_tokens returning u32), hash.rs (content_hash returning u64 — extends existing hash.rs from marky-0u5). All wrappers validate inputs, allocate output buffers with retry logic, handle error codes, and convert C structs to idiomatic Rust types.

## Effort Estimate
6-8 hours

## Success Criteria
- [ ] scan.rs: scan_headings() returns Result<Vec<HeadingScan>, KernelError>
- [ ] scan.rs: scan_links() returns Result<Vec<LinkScan>, KernelError>
- [ ] scan.rs: scan_tags() returns Result<Vec<TagScan>, KernelError>
- [ ] scan.rs: scan_block_ids() returns Result<Vec<BlockIdScan>, KernelError>
- [ ] tokens.rs: estimate_tokens() returns u32 (infallible for valid input)
- [ ] hash.rs: content_hash() returns u64 (infallible for valid input)
- [ ] Rust scan result types mirror Zig C ABI structs but use idiomatic Rust (String instead of offset+length)
- [ ] Buffer allocation with exponential retry on -2 (buffer too small)
- [ ] cargo test -p markymark-kernels passes all integration tests
- [ ] cargo clippy -p markymark-kernels -- -D warnings is clean

## Implementation Checklist
- [ ] Define Rust HeadingScan, LinkScan, TagScan, BlockIdScan types in scan.rs
- [ ] Define C ABI repr(C) mirror types for FFI boundary (CHeadingScan, etc.)
- [ ] Implement scan_headings: allocate buffer, call FFI, retry if -2, convert to Rust types
- [ ] Implement scan_links: same pattern with LinkScan
- [ ] Implement scan_tags: same pattern with TagScan
- [ ] Implement scan_block_ids: same pattern with BlockIdScan
- [ ] Implement estimate_tokens in tokens.rs: simple FFI call, no buffer needed
- [ ] Add content_hash to hash.rs: simple FFI call returning u64
- [ ] Add extern "C" declarations for all 6 new FFI functions
- [ ] Write integration tests for each wrapper function
- [ ] Test buffer retry logic with documents that have many headings

## Edge Cases
- Empty text: all scan functions return Ok(empty vec), tokens returns 0, hash returns offset basis
- Very long document with 1000+ headings: buffer retry must work (initial cap too small)
- Non-UTF-8 in scan results: C struct has offset+length into original text, Rust wrapper can borrow from input
- Concurrent calls: each call allocates its own buffers, no shared state
- Input with interior null bytes: len parameter is authoritative, null bytes are valid content

## Anti-patterns
- NO cloning the input text for every scan call (pass pointer and length directly)
- NO fixed-size buffer without retry (use exponential growth: 64 -> 128 -> 256 -> ...)
- NO converting offsets to String slices without validating UTF-8 boundaries
- NO unwrap/expect in production paths
- NO unsafe blocks without SAFETY comments documenting invariants

## Error Handling
- FFI returns -1: KernelError::InvalidInput
- FFI returns -2: retry with doubled buffer (max 3 retries), then KernelError::BufferTooSmall
- FFI returns -3: KernelError::InternalError
- UTF-8 boundary issue: when converting byte offsets to &str slices, round to nearest char boundary
- Allocation failure: propagate via Rust's OOM handler

## Test Specifications (what bug does each test catch?)
- test_scan_headings_basic: catches incorrect C-to-Rust struct conversion (field ordering mismatch)
- test_scan_headings_empty: catches null deref or error on empty input
- test_scan_headings_many: catches buffer retry failure when initial capacity is exceeded
- test_scan_links_markdown: catches incorrect link type mapping (0=markdown, 1=wiki)
- test_scan_links_wiki: catches wiki-link offset calculation errors
- test_scan_tags_basic: catches tag length field extraction error
- test_scan_block_ids_basic: catches block ID offset extraction error
- test_estimate_tokens_basic: catches incorrect return value interpretation (u32 vs i32)
- test_content_hash_deterministic: catches non-determinism in hash wrapper
- test_buffer_retry_succeeds: catches infinite loop or premature error in retry logic
- test_scan_headings_offset_validation: catches returning byte offsets that split UTF-8 characters
