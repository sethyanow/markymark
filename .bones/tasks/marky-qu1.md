---
id: marky-qu1
title: 'Phase 4.5: Delete blob serialization path (Rust from_blob + Zig serialize/blob)'
status: open
type: task
priority: 2
parent: marky-0xtn
---


## Context

- Phase 4.4 (marky-7ru) deleted the scanner module. The blob path (get_blob → from_blob) is the next deletion target.
- LSP switched to CEngineResult in Phase 2 (marky-2nxj). MCP switched in Phase 3 (marky-xfgb). Zero production callers of get_blob or from_blob remain.
- The blob path spans both Rust and Zig: Zig serializes DocumentEngine state into a flat binary blob, Rust deserializes it via from_blob into DocumentIndex. Both sides must be cleaned atomically.
- Correct deletion order: Rust consumers first (remove FFI declaration), then Zig producers (remove export + files). Reversing would break the link step.
- exports_serde.zig is for index_serde (kernel index serialization), NOT blob serialization — it stays.

**Blocked by:** marky-7ru (closed — scanner module deleted, no scanner types remain to confuse with blob types)
**Unlocks:** Epic criterion 6 fully satisfied ("serialize.zig, blob.zig, from_blob/ — ALL deleted"). Enables criterion 7 (CMd4c* types) and criterion 9 (md4c module) as the final deletion phases.

## Requirements

- R5 (from epic): All blob code deleted (serialize.zig, blob.zig, from_blob/*.rs)
- This task covers: delete Rust from_blob/ directory, ScanBlob type, get_blob() method, marky_engine_get_blob FFI; delete Zig serialize.zig, blob.zig, cached_blob field, marky_engine_get_blob export; delete gen_golden_blob binary + golden test data

## Implementation

1. **Delete Rust from_blob directory and golden test data** — Remove entire `markymark-index/src/document/from_blob/` directory (decode.rs 459L, header.rs 306L, mod.rs 441L, owned.rs 170L, tests/ 5 files 1179L). Delete `markymark-index/src/document/testdata/golden_v1.blob`. Delete `markymark-index/src/bin/gen_golden_blob.rs` (37L).

2. **Update document/mod.rs** — Remove `mod from_blob;` declaration (line 6). Remove `BlobError` re-export (line 15: `pub use from_blob::BlobError;`). Check for any other from_blob references.

3. **Remove blob FFI from engine.rs (Rust)** — Remove `marky_engine_get_blob` from the `extern "C"` block (lines 22-26). Remove `ScanBlob` struct and its impl (lines 34-55). Remove `get_blob()` method (lines 155-184). Update doc comment (lines 5-6) to remove "serializes state to a flat binary blob" language. Update the SAFETY comment at line 90 if it references get_blob.

4. **Delete blob-related tests from engine.rs** — Delete 6 tests that use `get_blob()`: `test_engine_lifecycle` (line 270), `test_engine_empty_input` (285), `test_engine_update_changes_blob` (297), `test_engine_multiple_updates` (311), `test_engine_blob_header_valid` (331), `test_engine_blob_caching` (353). Keep 4 non-blob tests: `test_engine_is_send_not_sync`, `test_engine_debug_format`, `test_engine_get_result_basic`, `test_engine_get_result_generation_increments`.

5. **Remove marky_engine_get_blob from Zig exports.zig** — Delete the `marky_engine_get_blob` export function and its 4 test functions (`engine_get_blob_basic`, `engine_get_blob_null_handle`, `engine_get_blob_null_output_ptrs`, `engine_get_blob_caching`). Remove `blob` import (line 9). Keep all get_result exports and non-blob tests.

6. **Remove blob support from Zig document.zig** — Remove `serialize_mod` import (line 20). Remove `cached_blob: ?[]u8 = null` field (line 65). Remove blob caching logic: line 145 (invalidate), lines 150-153 (lazy compute + cache), lines 225-227 (cleanup/free). Remove `serializeState` alias (line 639). Remove the `getBlob` method from DocumentEngine.

7. **Delete blob-related tests from Zig document_test.zig** — Delete all tests that use `getBlob`, `blob.readHeader`, `blob.validateBlob`, `cached_blob`, or `serializeState`: test_blob_header, test_blob_text_pool, test_blob_empty_document, test_blob_validate_rejects_bad_magic, test_blob_validates_after_serialize, test_update_invalidates_blob, blob_line_starts_roundtrip, engine_code_span_blob_roundtrip, engine_code_span_blob_roundtrip_empty, engine_xml_tags_blob_serialization_roundtrip, and any others referencing blob. Remove `blob` import (line 27) and blob test import (line 31). Keep non-blob engine tests.

8. **Delete Zig serialize.zig and blob.zig** — Remove `zig/src/engine/serialize.zig` (409L) and `zig/src/engine/blob.zig` (595L).

9. **Check for cascading dead code** — Run `cargo check --workspace --all-targets`. Run Zig build. Address any dead code warnings, unused imports, or orphaned types on both sides.

10. **Verify** — `cargo test --workspace`, `cargo clippy --all-targets`, `zig build test` (if applicable), verify zero from_blob/get_blob/ScanBlob/serialize.zig/blob.zig references remain via rg.

## Key Considerations

- Deletion order matters for the FFI boundary: Rust FFI declaration must be removed BEFORE the Zig export function. Removing the Zig export first breaks the link step.
- The `cached_blob` field in document.zig is part of the DocumentEngine struct's memory layout. Removing it changes the struct layout — but since no Rust code accesses DocumentEngine internals (only through FFI function calls), this is safe.
- `serializeState` in document.zig is called from the blob caching path inside `getBlob`. After removing `getBlob`, the `serializeState` call and the `serialize_mod` import become dead code.
- gen_golden_blob.rs is a one-off binary that was supposed to be deleted after generating golden_v1.blob (comment says "Commit the output file, then delete this binary"). Both go now.
- Several engine.rs tests that reference get_blob ALSO test non-blob behavior (create, update). After deleting the blob tests, verify the remaining get_result tests still cover engine create/update lifecycle.
- document_test.zig likely has tests that test non-blob engine behavior (update, get_result, code spans) — identify and keep these. Only delete tests whose primary assertion involves blob data.
- The `H2: serializeState returns OutOfMemory` test at document_test.zig:426 is a hardening test for serialize — it should be deleted since serialize.zig is going away.
- After this task, the Zig DocumentEngine's `getBlob` method disappears. The only way to get extraction results is `getResult` (CEngineResult). This is the intended end state.

## Success Criteria

- [ ] from_blob/ directory deleted (decode.rs, header.rs, mod.rs, owned.rs, tests/)
- [ ] gen_golden_blob.rs and golden_v1.blob deleted
- [ ] ScanBlob struct, get_blob() method, marky_engine_get_blob FFI removed from engine.rs
- [ ] Blob-related tests deleted from engine.rs (6 tests: lifecycle, empty_input, update_changes_blob, multiple_updates, blob_header_valid, blob_caching)
- [ ] marky_engine_get_blob export removed from Zig exports.zig
- [ ] cached_blob field, serializeState, getBlob removed from Zig document.zig
- [ ] Blob-related tests deleted from Zig document_test.zig
- [ ] serialize.zig and blob.zig deleted
- [ ] No from_blob/get_blob/ScanBlob/serialize.zig/blob.zig references remain in workspace
- [ ] cargo test --workspace passes
- [ ] cargo clippy --all-targets passes

## Anti-Patterns

- FORBIDDEN: deleting exports_serde.zig — it's for index_serde (kernel index), not blob serialization
- FORBIDDEN: deleting CEngineResult, get_result, or engine_ffi.rs — these are the REPLACEMENT for blob
- FORBIDDEN: deleting markymark-kernels/src/md4c/ module — separate scope (criterion 9)
- FORBIDDEN: deleting non-blob tests from document_test.zig — only delete tests whose assertions depend on blob data
- No keeping ScanBlob "for compatibility" — zero production consumers since Phase 2
- No keeping cached_blob as a "performance optimization" — CEngineResult eliminates the need for blob caching
