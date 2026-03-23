---
id: marky-sjrn
title: Add golden-blob roundtrip test to catch unilateral Zig/Rust blob format drift
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


Code review finding: existing test_from_blob_parity_with_from_scan provides dynamic cross-crate roundtrip coverage, but a checked-in golden blob file would catch unilateral Zig-side drift where both Zig and Rust agree on the wrong format.

## Design

## Goal
Add a checked-in golden blob file and a test that deserializes it through the Rust from_blob parser, catching unilateral format drift between the Zig blob serializer and the Rust blob deserializer.

## Why (Gap Analysis)
The existing test_from_blob_parity_with_from_scan (from_blob.rs:833) dynamically generates a blob via the Zig engine and parses it. This is excellent for catching bilateral drift (both sides change together). However, if the Zig engine silently changes field ordering, offset computation, or padding within a struct — and the Rust parser changes to match — both agree on the wrong format and the parity test still passes. A checked-in golden blob created at a known-good version catches this scenario.

## Effort Estimate
2-4 hours

## Success Criteria
- [ ] A golden blob file exists at a checked-in path (e.g., markymark-index/src/document/testdata/golden_v1.blob)
- [ ] Golden blob was generated from a known markdown input with headings, wiki links, markdown links, tags, and block IDs
- [ ] The generating markdown input is documented in the test or in a companion .md file
- [ ] test_golden_blob_roundtrip passes: calls validate_blob() and from_blob() on the golden bytes
- [ ] Test asserts header fields: magic, version, heading_count, link_count, tag_count, block_id_count
- [ ] Test asserts at least one heading text, slug, and level
- [ ] Test asserts at least one wiki link target and one markdown link url
- [ ] Test asserts at least one tag name and one block ID
- [ ] Test fails if validate_blob() returns Err
- [ ] Test fails if from_blob() returns Err
- [ ] Clippy clean, cargo test passes

## Implementation Checklist
- [ ] Choose canonical markdown input covering all element types (reuse mixed document from test_from_blob_mixed_document)
- [ ] Generate golden blob: write a one-off helper or use existing blob_for() to produce bytes and write to file
- [ ] Commit golden blob file to markymark-index/src/document/testdata/golden_v1.blob
- [ ] Commit companion file documenting provenance (input text, blob version, generation date)
- [ ] Write test_golden_blob_roundtrip in from_blob.rs tests module
- [ ] Include golden blob via include_bytes!() macro
- [ ] Assert all header fields and internal field values
- [ ] Run cargo test -p markymark-index to verify

## Key Considerations (SRE Review)

**Edge Case: Blob Version Bumps**
When BLOB_VERSION increments from 1 to 2, the golden file must be regenerated or a new golden_v2.blob added. Document this in the companion file. The test should assert the expected version number.

**Edge Case: Endianness**
Golden blob is little-endian. If the project ever targets big-endian platforms, the golden file remains valid because the Rust parser uses from_le_bytes(). No action needed but worth noting.

**Edge Case: Empty vs Populated Sections**
The golden file should have non-zero counts in ALL sections (headings, links, tags, block_ids) to maximize coverage. An empty section would not exercise the offset computation chain.

**Test Meaningfulness**
This test catches: (1) unilateral Zig struct field reordering, (2) padding/alignment changes in extern structs, (3) text pool offset computation drift, (4) section order changes. These are real risks because blob.zig uses extern struct which is ABI-dependent.

**Reference Implementation**
- Existing tests in from_blob.rs:654-1015 (use blob_for() helper)
- Blob format documented in from_blob.rs:1-21

## Anti-patterns
- Do NOT generate the golden blob at test time — it must be a static checked-in file
- Do NOT use assert!(result.is_ok()) — unwrap and assert specific field values
- Do NOT skip any section type in assertions (all must be checked)
- Do NOT hardcode byte offsets — use the from_blob public API for parsing
