---
id: marky-8s3.6
title: Implement binary index serialization format
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ccv, marky-qv6]
parent: marky-8s3
---




Create zig/src/kernels/index_serde.zig. Binary serialization format for DocumentIndex and RealmIndex data. Memory-mappable layout: fixed header (magic bytes, version, counts), then packed arrays of headings, links, tags, block_ids with string table. Operations: serialize_index(data, output_buf, cap) -> i32, deserialize_index(buf, len) -> handle, query operations on deserialized handle. Goal: instant startup by mmapping a .markymark-index file instead of re-parsing all documents. Tests: round-trip correctness, backward compatibility header, corrupt data handling, large index (1000+ docs). Rust FFI wrapper with mmap integration.

## Design

## Goal
Create zig/src/kernels/index_serde.zig implementing a binary serialization format for DocumentIndex and RealmIndex data. Memory-mappable layout for instant startup: fixed header (magic bytes, version, counts), packed arrays of headings/links/tags/block_ids, and a string table. Enables mmapping a .markymark-index file instead of re-parsing all documents on startup.

## Effort Estimate
12-14 hours

## Success Criteria
- [ ] index_serde.zig compiles with C ABI exports: serialize_index, deserialize_index, query operations
- [ ] Binary format has magic bytes (e.g., "MKYI"), version field, and section counts
- [ ] Round-trip: serialize -> deserialize produces identical data
- [ ] Memory-mappable: deserialized handle can be used directly from mmap'd buffer (no copying)
- [ ] Backward compatibility: version field enables future format evolution
- [ ] Handles corrupt data gracefully (magic byte check, bounds validation)
- [ ] cd zig && zig build test passes
- [ ] Large index (1000+ docs) serializes and deserializes correctly

## Implementation Checklist
- [ ] Define binary format layout: header | string_table | heading_array | link_array | tag_array | block_id_array
- [ ] Header: magic (4 bytes), version (u16), flags (u16), section_count, doc_count, total_headings, total_links, total_tags, total_block_ids, string_table_size
- [ ] String table: packed null-terminated strings, referenced by offset from arrays
- [ ] Implement serialize_index: write header, then packed arrays, then string table
- [ ] Implement deserialize_index: validate magic+version, return handle to mapped data
- [ ] Implement query_headings, query_links etc. on deserialized handle (read from mapped memory)
- [ ] Add C ABI exports to c_adapter.zig
- [ ] Rust FFI wrapper with mmap integration (using memmap2 crate)
- [ ] Write round-trip tests
- [ ] Write corruption handling tests
- [ ] Write large-index test (1000+ docs)
- [ ] Update build.zig

## Edge Cases
- Empty index (0 docs): serialize produces valid header with zero counts
- Very large string table (>4GB): use u64 offsets or document 4GB limit clearly
- Corrupt magic bytes: deserialize returns error immediately
- Truncated file: deserialize validates file size against header counts
- Endianness: use little-endian consistently (document this)
- Version mismatch: deserialize rejects future versions, accepts current
- Null bytes in heading text: string table uses length-prefixed strings or tracks lengths separately
- Index file from different platform: endianness must be consistent
- Concurrent read/write: document that writes need exclusive access

## Anti-patterns
- NO using JSON/text format (this is a binary format for performance)
- NO copying data on deserialize (the point is zero-copy mmap)
- NO variable-length headers (fixed header enables direct offset calculation)
- NO heap allocation in the deserialize path (work directly on mmap'd memory)
- NO ignoring alignment requirements (structs must be naturally aligned for mmap)
- NO skipping validation on deserialize (corrupt files must be detected)

## Error Handling
- Null input buffer: return -1
- Buffer too small for serialize output: return -2
- Invalid magic bytes on deserialize: return -1
- Version mismatch on deserialize: return -1 (with version field in error struct)
- Truncated file: return -3
- Corrupt section counts (would read past buffer): return -3
- Allocation failure in serialize: return -3

## Test Specifications (what bug does each test catch?)
- test_empty_index_round_trip: catches serialization failure on zero-count index
- test_single_doc_round_trip: catches basic field ordering or alignment bugs
- test_large_index_round_trip: catches overflow in offset calculations at scale (1000+ docs)
- test_corrupt_magic_bytes: catches missing validation in deserialize path
- test_truncated_file: catches buffer overread on short input
- test_version_check: catches accepting incompatible future format versions
- test_string_table_correctness: catches off-by-one in string offset resolution
- test_heading_query_on_deserialized: catches incorrect pointer arithmetic in query functions
- test_link_query_on_deserialized: catches field alignment issues in packed link structs
- test_endianness_consistency: catches mixed endian reads (if testing on big-endian platform)
- test_alignment_of_packed_structs: catches mmap page fault from misaligned struct access
- test_zero_copy_verification: catches hidden copying in deserialize path
