---
id: marky-io3h
title: 'H: Zig Document Engine — stateful composite scan with flat binary blob'
status: closed
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-77i
---








## Design

## Requirements (IMMUTABLE)

- Single FFI call (`marky_engine_create` / `marky_engine_update`) replaces all per-document scan FFI calls (md4c extract, SIMD tags, SIMD block_ids, N×slugify, token estimate, content hash)
- Zig DocumentEngine owns persistent document state across edits: headings, links, tags, block_ids, line_starts, slugs, positions, token_estimate, content_hash
- Blob format is a single contiguous allocation with fixed-size header, packed struct arrays, and text pool — zero pointer chasing, mmap-compatible layout
- Rust `DocumentIndex::from_blob()` produces identical output to current `from_scan()` for the same input document (correctness parity)
- Existing `from_ast()` and `from_scan()` paths remain functional (not deleted) for backward compatibility and MCP/batch use
- Tree-sitter lazy AST is NOT part of this engine — it stays separate for hover/goto-def

## Success Criteria (MUST ALL BE TRUE)

- [ ] Full pipeline (engine.update + get_blob + from_blob) is ≥1.5x faster than current from_scan() at 50KB
- [ ] Total FFI calls per document update: exactly 2 (update + get_blob) regardless of heading/link count
- [ ] from_blob() produces identical DocumentIndex fields as from_scan() for same input (heading text/slug/level/positions, link text/target/type/positions, tag names, block IDs)
- [ ] Blob format has magic number, version field, and validates on read (rejects corrupt/mismatched blobs)
- [ ] Engine lifecycle test: create → update 100 times → destroy without leaks (Zig GeneralPurposeAllocator leak detection)
- [ ] All existing tests pass (cargo nextest)
- [ ] Zero clippy warnings, fmt clean

## Anti-Patterns (FORBIDDEN)

- ❌ NO partial md4c parsing (correctness: md4c is a streaming single-pass parser, partial parse produces incorrect results)
- ❌ NO Rust-side incremental merge logic in the engine path (simplicity: the whole point is Zig owns this; Rust receives finished blob)
- ❌ NO Zig-to-Rust callbacks during parse (complexity: callbacks across FFI boundary add calling convention risk and make debugging hard)
- ❌ NO shared mutable state between engine handle and Rust (safety: Zig owns the handle, Rust borrows the blob read-only)
- ❌ NO removing from_ast()/from_scan() paths (compatibility: MCP batch indexing and tree-sitter AST path still need them)
- ❌ NO tree-sitter integration inside the engine (scope: engine owns scan/index state only, lazy AST is separate concern)

## Approach

Replace the current multi-call FFI scan pipeline with a stateful Zig Document Engine. On `did_change`, the LSP calls `engine.update(text)` (one FFI call), then `engine.get_blob()` to get a flat binary blob containing all pre-computed index data. Rust's `DocumentIndex::from_blob()` copies text from the blob's contiguous pool into bumpalo arena and builds the HashMap + TOC/Outline structures. All heavy computation (md4c parse, SIMD tag/block scans, slugification, position conversion, entity decoding) happens in Zig.

The engine maintains persistent state per document, enabling future incremental optimizations (slug caching, position reuse) without changing the FFI contract. Blob serialization is lazy — only built when get_blob() is called, so rapid edits absorbed by debounce never trigger serialization.

This eliminates ~850 lines of Rust incremental indexing code and reduces FFI calls from N+4 per document (N = heading count) to exactly 2, regardless of document complexity.

## Architecture

### Zig Modules (new: zig/src/engine/)

- `document.zig`: DocumentEngine struct with create/update/getBlob/destroy methods. Holds StoredHeading[], StoredLink[], StoredTag[], StoredBlockId[], line_starts[], cached_blob
- `blob.zig`: ScanBlobHeader (64 bytes), BlobHeading (40 bytes), BlobLink (40 bytes), BlobTag (24 bytes), BlobBlockId (28 bytes). Pack/unpack functions for the flat binary format
- `exports.zig`: C ABI exports — marky_engine_create, marky_engine_update, marky_engine_get_blob, marky_engine_destroy

### Rust Modules (new/modified)

- `markymark-kernels/src/engine.rs` (new): DocumentEngine FFI wrapper, ScanBlob view type
- `markymark-index/src/document/from_blob.rs` (new): DocumentIndex::from_blob() constructor
- `markymark-lsp/src/state/` (modified): Replace ScanBackend dispatch + incremental logic with engine.update() + from_blob() pipeline

### Blob Format

```
[ScanBlobHeader: 64 bytes]
  magic(4) version(2) flags(2) heading_count(4) link_count(4)
  tag_count(4) block_id_count(4) line_count(4) text_pool_size(4)
  headings_offset(4) links_offset(4) tags_offset(4)
  block_ids_offset(4) line_starts_offset(4) text_pool_offset(4)
  token_estimate(4) content_hash(8)

[BlobHeading[N]: 40 bytes each]
  text_off(4) text_len(4) slug_off(4) slug_len(4)
  source_offset(4) start_line(4) start_col(4)
  end_line(4) end_col(4) level(1) _pad(3)

[BlobLink[N]: 40 bytes each]
  text_off(4) text_len(4) target_off(4) target_len(4)
  source_offset(4) start_line(4) start_col(4)
  end_line(4) end_col(4) is_wiki(1) _pad(3)

[BlobTag[N]: 24 bytes each]
  name_off(4) name_len(4) source_offset(4)
  start_line(4) start_col(4) _pad(4)

[BlobBlockId[N]: 28 bytes each]
  id_off(4) id_len(4) source_offset(4)
  start_line(4) start_col(4) end_line(4) end_col(4)

[line_starts: line_count * 4 bytes]
[text_pool: text_pool_size bytes]
```

### Data Flow

did_change → engine.update(text) [1 FFI call]
  → Zig: md4c parse + SIMD tags/blocks + line_starts + slugify + positions
  → Zig: store in DocumentEngine, invalidate cached_blob

engine.get_blob() [1 FFI call]
  → Zig: serialize state to flat blob (lazy, cached between updates)
  → Rust: receives read-only pointer + length

DocumentIndex::from_blob(blob)
  → Rust: copy strings from text_pool → bumpalo arena
  → Rust: build slug_to_heading HashMap, TOC, Outline
  → Return DocumentIndex (same type as from_ast/from_scan)

## Design Rationale

### Problem

The current md4c integration (marky-0mr) achieved 2.8x speedup over tree-sitter but has significant FFI overhead. At 50KB: Zig parse takes 2.1ms, FFI boundary costs 2.6ms (string copies, multiple calls), and Rust from_scan() adds 4.7ms (line_starts, N×slugify, positions, arena). Text data is copied 4 times between Zig parse and DocumentIndex. N+4 FFI calls per document (N = heading count for slugify). The Rust codebase also carries ~850 lines of incremental indexing logic that becomes unnecessary if full scan is fast enough.

### Research Findings

**Codebase:**
- zig/src/md4c/extraction_renderer.zig: Per-heading: 3 allocations (text + vector). Per-link: 4 allocations. All using page_allocator.
- zig/src/md4c/exports.zig: Consolidates to text_blob for FFI but Rust then copies each string to owned String, then copies again to arena.
- markymark-index/src/document/mod.rs: from_scan() does line_starts (O(n) scan), N×slugify FFI calls, byte→position binary search, arena allocation, HashMap + TOC + Outline.
- markymark-kernels/src/scan.rs: call_scan_ffi() retry pattern (64→128→256→512 capacity).
- Existing stateful Zig patterns: EmbeddingIndex and LinkGraph use same opaque handle + stateful FFI pattern.
- marky_multi_scan already exists as composite SIMD scan but md4c path doesn't use it.

**Performance data (marky-jpot benchmarks):**
- Zig-only extraction at 50KB: 2.1ms
- FFI extraction at 50KB: 4.7ms (2.2x overhead from blob packing, String allocation)
- Full from_scan at 50KB: 9.4ms
- Throughput drops at scale: 53 MB/s at 10KB → 23 MB/s at 50KB (extraction allocation density)

### Approaches Considered

#### 1. Stateful Zig Document Engine ✓

**What it is:** Zig maintains persistent per-document state. On update, full md4c reparse + SIMD scans. State diffed internally. Blob serialized lazily. Rust wraps blob with from_blob().

**Investigation:**
- Existing pattern: EmbeddingIndex, LinkGraph use identical opaque handle approach
- md4c is streaming/full-reparse — no incremental parse possible, so engine always has latest state
- Lazy blob means rapid edits (absorbed by debounce) never serialize intermediate states
- Arena allocator in Zig would replace per-element page_allocator, recovering throughput

**Pros:**
- Maximum FFI reduction (N+4 → 2 calls)
- Eliminates quadruple text copy (Zig parse → blob → arena, one copy instead of four)
- Deletes ~850 lines of Rust incremental logic
- Clean architectural separation (Zig = computation engine, Rust = data presentation)
- Foundation for future optimizations (slug caching, position reuse) without FFI contract changes

**Cons:**
- New Zig module (~600-800 lines)
- Handle lifecycle management
- Blob format versioning overhead

**Chosen because:** Matches user's architectural direction of sinking computation into Zig. Reuses proven handle pattern. Simplifies Rust codebase significantly.

#### 2. Stateless Composite Scan (flat blob, no persistent state) ❌

**What it is:** Single marky_composite_scan() call replaces all FFI calls. Returns flat blob. No persistent state in Zig.

**Why we looked at this:** Simpler than stateful engine — no handle management, no state diffing.

**Investigation:**
- Would achieve same blob format and from_blob() path
- Same FFI reduction (many → 1 call per scan)
- But: every update re-slugifies ALL headings (no caching)
- And: no lazy serialization (blob built every time)
- Can't evolve to incremental without adding state

**Pros:** Simpler Zig code. No lifecycle management. Stateless is easier to reason about.
**Cons:** No slug caching. No lazy blob. Can't incrementally optimize later.

**REJECTED BECAUSE:** Leaves optimization potential on the table. Stateful engine is same initial complexity but enables future gains. User explicitly chose stateful direction.

**DO NOT REVISIT UNLESS:** Engine state management proves too complex or leaky.

#### 3. Callback-driven (Zig calls Rust function pointers) ❌

**What it is:** During md4c parse, Zig calls Rust callbacks for each heading/link. Rust arena-allocates directly. No intermediate buffer.

**Why we looked at this:** True zero-copy — no blob format, no serialization overhead.

**Pros:** Zero-copy for text data. No blob format to maintain.
**Cons:** FFI callbacks per element are high overhead. Complex calling convention. Debugging across FFI boundary is hard. Unusual pattern in codebase.

**REJECTED BECAUSE:** Per-element FFI callbacks would be MORE overhead than current approach. Violates the consolidation goal.

**DO NOT REVISIT UNLESS:** Blob serialization proves to be a significant bottleneck (unlikely given lazy caching).

### Scope Boundaries

**In scope:**
- Zig DocumentEngine with create/update/getBlob/destroy
- Flat binary blob format with header + packed structs + text pool
- Rust DocumentIndex::from_blob() constructor
- LSP integration replacing current scan dispatch
- Correctness tests (from_blob == from_scan for same input)
- Performance benchmarks (engine vs from_scan pipeline)

**Out of scope (deferred/never):**
- Incremental md4c parsing (never — md4c is streaming, doesn't support it)
- Internal incremental diffing in engine.update() (deferred to v2 — full rebuild is fast enough for v1)
- Tree-sitter integration inside engine (never — separate concern)
- Deleting from_ast()/from_scan() paths (never — needed for MCP batch and AST-dependent operations)
- Zero-copy DocumentIndex that borrows directly from blob (deferred — start with arena copy, evolve later)

### Open Questions
- Should engine.update() accept edit ranges for future incremental diffing? (defer — v1 takes full text only)
- Should blob include XML tags? (currently empty for scan path — defer to implementation)
- What allocator should engine internals use? (GPA for debug, page_allocator for release — decide during implementation)
- Should get_blob() return Zig-owned or caller-owned memory? (Zig-owned with explicit free, matching existing pattern)

## Design Discovery (Reference Context)

### Key Decisions Made

| Question | User Answer | Implication |
|----------|-------------|-------------|
| How aggressive with Zig consolidation? | Full composite | Single FFI call, maximum work in Zig |
| Blob vs expanded C struct vs callbacks? | Flat binary blob | Same pattern as index_serde, version-able, mmap-compatible |
| Include line_starts in blob? | Yes | Zig computes once, Rust reuses. Enables future incremental. |
| Arena vs zero-copy for DocumentIndex? | Start with arena, evolve to ZC | One copy from blob to arena. Same DocumentIndex API. Ship faster. |
| New epic or child of marky-0mr? | New epic under marky-77i | Distinct optimization phase with own success criteria |
| Stateful engine or stateless scan? | Stateful Zig engine | Persistent state enables slug caching, lazy blob, future incremental |
| Incremental strategy? | Full rebuild, rely on speed + debounce | ~4ms with debounce is fast enough. Delete Rust incremental code. |

### Research Deep-Dives

#### FFI Overhead Analysis
**Question:** Where is the FFI boundary costing time?
**Sources:** markymark-kernels/src/md4c.rs, scan.rs; zig/src/md4c/exports.zig, extraction_renderer.zig
**Findings:**
- Quadruple text copy: Zig parse → blob → Rust String → arena
- N+4 FFI calls per document (50 headings = 54 calls)
- Per-element page_allocator in ExtractionRenderer (3-4 allocs per heading/link)
- Rust from_scan: line_starts + N×slugify + positions + arena = 4.7ms at 50KB
**Conclusion:** FFI boundary costs 2.6ms (more than Zig parse itself). Consolidating into single call with flat blob eliminates most of this.

#### Existing Stateful Zig Patterns
**Question:** Is opaque handle pattern proven in this codebase?
**Sources:** exports_embed.zig, exports_graph.zig
**Findings:**
- EmbeddingIndex: create/add/search/destroy lifecycle, Zig-owned via page_allocator
- LinkGraph: create/add_document/find_orphans/destroy lifecycle
- Both work correctly in production
**Conclusion:** Pattern is proven and well-understood. DocumentEngine follows same shape.

### Dead-End Paths

#### Extended CMd4cResult (widen existing struct)
**Why explored:** Evolutionary approach — just add more fields to existing C struct.
**Why abandoned:** Too many pointer fields becomes fragile. No version management. Hard to extend further. Blob format is cleaner contract.

### Open Concerns Raised

- "How about incremental?" → md4c is always full reparse. At ~4ms with debounce, incremental unnecessary. Blob format supports future diffing if needed.
- "We're already set up doing incremental with Zig" → Current incremental is Rust-side (from_ast path). Engine replaces this with fast full rebuild. Rust incremental code becomes dead code on engine path.
