# BRZA-markymark Integration Spec

**Version:** 0.1.0
**Status:** Draft
**Priority:** P2 (parallel exploration alongside v1.0 launch)
**Branch:** `feature/mark-brza`
**Beads Epic:** TBD

---

## 1. Overview

Apply the BRZA (Bun + Rust + Zig + ASM) four-layer performance stack to markymark,
adding SIMD-accelerated extraction, semantic search, and content intelligence to the
Markdown LSP/MCP server.

**Design principle:** Zig kernels complement tree-sitter, not replace it. Zig handles
fast extraction passes (headings, links, tags, block IDs). Tree-sitter provides the full
AST when needed (diagnostics, hover, go-to-definition). Benchmark both paths; promote
Zig to primary if it covers 95%+ of extraction needs.

**Kernel strategy:** Copy and diverge from forge BRZA. Shared kernels (embeddings,
similarity, quantization) are forked into markymark's zig/ directory and evolve
independently.

---

## 2. Layer Model (markymark-specific)

```text
Layer 4 - B (Bun/TypeScript)
          |  Claude Code plugin: select-binary.sh, hooks.json
          |  Plugin lifecycle, config, marketplace registration
          |  MCP tool registration (implicit via plugin system)
          |
          |  Process boundary: plugin spawns Rust binary
          v
Layer 3 - R (Rust)
          |  markymark-core, parser, index, lsp, mcp, cli
          |  Safety envelope: tree-sitter FFI, arena allocation (bumpalo)
          |  LSP/MCP protocol handling, RealmIndex, DocumentIndex
          |  Traits: ScanBackend, EmbeddingProvider
          |
          |  C ABI: extern "C", libmarky_kernels.a, build.rs
          v
Layer 2 - Z (Zig)
          |  SIMD hot paths:
          |    Extraction: heading_scan, link_scan, tag_scan, block_scan
          |    Intelligence: token_estimate, content_hash
          |    Shared: embedding_index, cosine_similarity, jaccard_similarity
          |    Shared: entity_hashes, normalize, quantize/dequantize
          |
          |  @Vector(4, f32) portable SIMD
          v
Layer 1 - A (ASM)
             Platform-specific SIMD via Zig's @Vector
             NEON (Apple Silicon), AVX2/SSE (x86_64)
             Comptime-verified inline assembly
```

### 2.1 Layer Responsibilities

| Layer | Language | Owns | Delegates To |
|-------|----------|------|-------------|
| **B** | Shell/TS | Plugin lifecycle, config, user-facing CLI surface | R (process spawn) |
| **R** | Rust | Safety boundary, traits, protocol handling, arena allocation | Z (C ABI calls) |
| **Z** | Zig | SIMD kernels, fast extraction, embedding index, similarity | A (inline asm) |
| **A** | ASM | Platform intrinsics via @Vector | Hardware |

### 2.2 Boundary Contracts

| Boundary | Mechanism | Ownership |
|----------|-----------|-----------|
| B <-> R | Process spawn | Plugin owns process lifecycle. Rust owns heap. |
| R <-> Z | `extern "C"` FFI, `libmarky_kernels.a` | Rust passes `(ptr, len)`. Zig writes to caller buffers. Zig never allocates for caller. |
| Z <-> A | Inline asm within Zig | Zig owns registers. ASM uses Zig stack. |

### 2.3 Memory Ownership at FFI Boundary

Same pattern as forge BRZA:
1. Rust allocates output buffers
2. Rust passes `(ptr, len)` pairs to Zig
3. Zig writes results into provided buffers
4. Zig returns status code (0 = success, negative = error)
5. Rust owns freeing everything

Zig uses arena allocators internally for temporary work, freed before returning.

---

## 3. Dependency Graph

```
markymark-kernels        <- Zig FFI bindings (no markymark deps)
    ^ (optional, feature-gated)
markymark-core           <- Traits: ScanBackend, EmbeddingProvider
    ^                       + optional ZigScanBackend, ZigEmbeddingIndex
markymark-parser         <- tree-sitter (unchanged)
markymark-index          <- Uses ScanBackend for extraction
    ^                       + optional EmbeddingIndex per realm
markymark-lsp            <- Unchanged
markymark-mcp            <- New: semantic-search tool
    ^
markymark-cli            <- Unchanged
```

### 3.1 Feature Flags

```toml
# markymark-kernels/Cargo.toml — no flags, pure FFI
[package]
name = "markymark-kernels"

# markymark-core/Cargo.toml
[features]
zig-kernels = ["dep:markymark-kernels"]

[dependencies]
markymark-kernels = { path = "../markymark-kernels", optional = true }

# markymark-index/Cargo.toml
[features]
zig-kernels = ["markymark-core/zig-kernels"]
embeddings = ["zig-kernels"]

# markymark-mcp/Cargo.toml
[features]
semantic-search = ["markymark-index/embeddings"]
```

**Without flags:** markymark works exactly as today.
**With `zig-kernels`:** Fast SIMD scanning path available.
**With `embeddings`:** Semantic search enabled (requires runtime embedding provider).

---

## 4. C ABI Surface

### 4.1 Extraction Kernels (markymark-specific)

| Function | Signature | Purpose |
|----------|-----------|---------|
| `marky_scan_headings` | `(text: [*]const u8, len: u32, out: [*]HeadingScan, cap: u32, written: *u32) -> i32` | SIMD heading extraction |
| `marky_scan_links` | `(text: [*]const u8, len: u32, out: [*]LinkScan, cap: u32, written: *u32) -> i32` | SIMD `[text](url)` and `[[wiki-link]]` detection |
| `marky_scan_tags` | `(text: [*]const u8, len: u32, out: [*]TagScan, cap: u32, written: *u32) -> i32` | SIMD `#tag` boundary detection |
| `marky_scan_block_ids` | `(text: [*]const u8, len: u32, out: [*]BlockIdScan, cap: u32, written: *u32) -> i32` | SIMD `^block-id` scanning |
| `marky_estimate_tokens` | `(text: [*]const u8, len: u32) -> u32` | Approximate BPE token count |
| `marky_content_hash` | `(text: [*]const u8, len: u32) -> u64` | FNV-1a content fingerprint |

### 4.2 Scan Result Structs (C ABI)

```zig
const HeadingScan = extern struct {
    offset: u32,       // byte offset in text
    length: u16,       // heading text length
    level: u8,         // 1-6
    _padding: u8,
};

const LinkScan = extern struct {
    offset: u32,       // byte offset of link start
    text_offset: u32,  // offset of link text
    text_length: u16,  // link text length
    target_offset: u32,// offset of link target
    target_length: u16,// target length
    link_type: u8,     // 0 = markdown, 1 = wiki-link
    _padding: u8,
};

const TagScan = extern struct {
    offset: u32,       // byte offset of # character
    length: u16,       // tag name length (without #)
    _padding: [2]u8,
};

const BlockIdScan = extern struct {
    offset: u32,       // byte offset of ^ character
    length: u16,       // block ID length (without ^)
    _padding: [2]u8,
};
```

### 4.3 Shared Kernels (forked from forge BRZA)

| Function | Signature | Purpose |
|----------|-----------|---------|
| `zig_embedding_index_create` | `(dims: u32) -> ?*anyopaque` | Create embedding index |
| `zig_embedding_index_destroy` | `(handle: *anyopaque) -> void` | Destroy index |
| `zig_embedding_index_add` | `(handle, id, id_len, embedding, dims) -> i32` | Add embedding |
| `zig_embedding_index_search` | `(handle, query, dims, results, ...) -> i32` | Top-K cosine search |
| `zig_embedding_index_count` | `(handle) -> i32` | Entry count |
| `zig_cosine_similarity` | `(a, b, dims) -> f32` | Cosine similarity |
| `zig_jaccard_similarity` | `(set1, len1, set2, len2) -> f32` | Jaccard similarity |
| `zig_extract_entity_hashes` | `(text, len, out, cap, written) -> i32` | FNV-1a entity extraction |
| `asm_normalize_f32_l2` | `(input, output, n) -> i32` | L2 normalization |
| `asm_quantize_f32_to_q4_0` | `(input, output, n) -> i32` | 4-bit quantization |
| `asm_dequantize_q4_0_to_f32` | `(input, output, n) -> i32` | 4-bit dequantization |

### 4.4 Error Codes

Same as forge BRZA:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `-1` | Invalid input (null pointer, zero length) |
| `-2` | Buffer too small |
| `-3` | Internal error |

---

## 5. Two-Tier Extraction Model

### 5.1 Architecture

```text
Document arrives (raw markdown text)
|
+- TIER 1: Zig SIMD Scan (zig-kernels feature)
|   |  scan_headings() -> byte positions, level, text
|   |  scan_links()    -> byte positions, text, target, type
|   |  scan_tags()     -> byte positions, tag name
|   |  scan_block_ids() -> byte positions, block ID
|   |
|   |  Result: StructuralScan { headings, links, tags, block_ids }
|   |
|   |  NOTE: May produce false positives from code blocks.
|   |  See Section 5.2 for mitigation strategy.
|   |
|   |  Sufficient for: search-symbols, find-references, realm-stats,
|   |                  semantic-search indexing, content fingerprinting,
|   |                  near-duplicate detection
|   |
|   +- PROMOTION PATH: Benchmark both tiers. If Zig covers 95%+
|      of extraction needs accurately, promote to primary.
|      Tree-sitter becomes optional for precision operations.
|
+- TIER 2: tree-sitter Full Parse (always available)
    |  Full AST with nested structure, frontmatter, code blocks
    |  Context-aware: knows code blocks, inline code, escapes
    |
    |  Needed for: go-to-definition, hover, diagnostics,
    |              XML tag analysis, callout parsing, frontmatter
    |
    +- Current behavior, no changes
```

### 5.2 Code Block Problem

Markdown is context-sensitive. Zig SIMD scanning sees raw bytes without knowing
whether content is inside a fenced code block or inline code:

```text
# This IS a heading
```python
# This is NOT a heading (inside code block)
```
[[real-link]] vs `[[not-a-link]]`
```

**Mitigation strategy (complement path):**
1. Accept false positives in Tier 1 scans
2. Use Tier 1 for speed-critical operations where precision isn't critical
   (search ranking, stats, fingerprinting, embedding indexing)
3. Use Tier 2 for precision operations (diagnostics, go-to-def, hover)

**Mitigation strategy (promotion path, future):**
1. Two-pass Zig scan: first find code fences (scan for ``` or ~~~), build exclusion ranges
2. Second pass: scan for headings/links/tags ignoring exclusion ranges
3. Validates with tree-sitter parity tests before promotion

---

## 6. Semantic Search

### 6.1 EmbeddingProvider Trait

```rust
// markymark-core/src/embeddings.rs
pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dimensions(&self) -> u32;
}
```

Source-agnostic. Implementations can be:
- Local ONNX model (all-MiniLM-L6-v2, 384-dim)
- API call (OpenAI, Anthropic, Cohere)
- TF-IDF in Zig (zero model dependency)
- PAI inference tool

Decision on provider deferred (per brainstorm).

### 6.2 Embedding Index per Realm

```rust
// markymark-index/src/semantic.rs
pub struct SemanticIndex {
    index: ZigEmbeddingIndex,  // FFI to Zig SIMD
    entries: Vec<SemanticEntry>,
}

pub struct SemanticEntry {
    doc_uri: String,
    heading: String,
    heading_level: u8,
    section_start: usize,
    section_end: usize,
}
```

Each realm optionally maintains a `SemanticIndex`. When a document is added:
1. Extract headings (Tier 1 or Tier 2)
2. For each heading, embed the heading text + first N tokens of section content
3. Add to the Zig embedding index with doc_uri + heading as the key

### 6.3 MCP Tool

```json
{
  "tool": "semantic-search",
  "description": "Search documents by meaning, not just keywords",
  "inputSchema": {
    "properties": {
      "query": { "type": "string" },
      "realm": { "type": "string" },
      "top_k": { "type": "integer", "default": 10 },
      "min_score": { "type": "number", "default": 0.5 }
    },
    "required": ["query"]
  }
}
```

---

## 7. Content Intelligence

### 7.1 Near-Duplicate Detection

For each document pair in a realm:
1. `zig_extract_entity_hashes(doc_a)` -> hash set A
2. `zig_extract_entity_hashes(doc_b)` -> hash set B
3. `zig_jaccard_similarity(A, B)` -> similarity score
4. If score > 0.8, flag as potential duplicate

Exposed via enhanced `realm-stats --check-duplicates`.

### 7.2 Token Estimation

`marky_estimate_tokens(text)` provides approximate BPE token count:
- Byte-level heuristic: `len * 0.3` for English text (rough)
- Or: SIMD word boundary detection + average tokens-per-word multiplier
- Useful for AI context budget reporting

### 7.3 Content Fingerprinting

`marky_content_hash(text)` provides FNV-1a hash for:
- Change detection faster than mtime (content-aware)
- Cache invalidation for re-indexing
- Deduplication keys

---

## 8. Build System

### 8.1 Artifact

`libmarky_kernels.a` - Zig static library containing all C ABI exports.

**Build:** `cd zig && zig build lib`

### 8.2 Rust Integration

```
markymark-kernels/build.rs  - Compiles Zig, links libmarky_kernels.a
```

The build.rs:
1. Checks for Zig compiler (`zig version`)
2. Runs `zig build lib` in `../zig/`
3. Links `libmarky_kernels.a` from `zig/zig-out/lib/`
4. Generates `#[link]` directives

### 8.3 Platform Targets

| Platform | SIMD Backend | Status |
|----------|-------------|--------|
| macOS arm64 (M-series) | NEON via @Vector(4, f32) | Primary dev |
| Linux x86_64 | SSE/AVX via @Vector(4, f32) | CI target |
| macOS x86_64 | SSE via @Vector(4, f32) | CI target |
| Linux arm64 | NEON via @Vector(4, f32) | CI target |
| Windows x86_64 | SSE/AVX via @Vector(4, f32) | CI target |

Zig's @Vector compiles to native SIMD on each platform.

---

## 9. Testing Strategy

### 9.1 Per-Boundary Testing

| Boundary | Test Type | Scope |
|----------|----------|-------|
| Z internal | Unit (correctness vs reference impl) | Each kernel against scalar reference |
| Z internal | Perf (SIMD vs scalar threshold) | Minimum 2x speedup assertion |
| Z -> C ABI | Export verification | All exports callable, error codes correct |
| R -> Z FFI | Safe wrapper validation | Null handling, buffer sizing, error propagation |
| R integration | ScanBackend parity | Zig scan vs tree-sitter extraction on same docs |
| R integration | EmbeddingIndex lifecycle | Create/add/search/destroy cycle |
| R integration | Feature flag gating | Without zig-kernels: all tests pass, no Zig dependency |
| End-to-end | MCP semantic-search | Query through MCP → get ranked results |

### 9.2 Extraction Parity Testing

Compare Tier 1 (Zig) vs Tier 2 (tree-sitter) on the same documents:
- Heading count match (excluding code blocks)
- Link count match (excluding code blocks)
- Tag count match
- Block ID count match

Track false positive rate from code blocks. If < 5% across a 600+ doc
corpus, the complement strategy is validated.

### 9.3 Benchmark Suite

| Benchmark | Metric | Baseline | Target |
|-----------|--------|----------|--------|
| heading_scan vs tree-sitter | ops/sec | tree-sitter baseline | 10-100x |
| link_scan vs regex | ops/sec | regex baseline | 10-50x |
| embedding search (1K entries) | latency | N/A (new) | < 1ms |
| embedding search (100K entries) | latency | N/A (new) | < 10ms |
| content_hash vs md5 | ops/sec | md5 baseline | 2-5x |
| bulk re-index (600 docs) | total time | tree-sitter baseline | 5-20x |

---

## 10. Workspace Structure

```text
markymark/
+-- zig/                           # NEW
|   +-- build.zig                  # Produces libmarky_kernels.a
|   +-- src/
|   |   +-- kernels/
|   |   |   +-- heading_scan.zig
|   |   |   +-- link_scan.zig
|   |   |   +-- tag_scan.zig
|   |   |   +-- block_scan.zig
|   |   |   +-- token_estimate.zig
|   |   |   +-- content_hash.zig
|   |   +-- shared/                # Forked from forge BRZA
|   |   |   +-- embeddings.zig
|   |   |   +-- similarity.zig
|   |   |   +-- entities.zig
|   |   |   +-- quantize.zig
|   |   |   +-- normalize.zig
|   |   |   +-- abi.zig
|   |   +-- c_adapter.zig          # C ABI exports
|   |   +-- reference/             # Scalar reference impls
|   |   +-- fixtures.zig           # Test data
|   |   +-- harness.zig            # Test harness
|   +-- test/                      # Zig tests
|
+-- markymark-kernels/             # NEW Rust crate
|   +-- Cargo.toml
|   +-- build.rs
|   +-- src/
|       +-- lib.rs
|       +-- scan.rs
|       +-- embed.rs
|       +-- similarity.rs
|       +-- tokens.rs
|       +-- hash.rs
|
+-- markymark-core/                # MODIFIED
|   +-- src/
|       +-- scanner.rs             # NEW: ScanBackend trait
|       +-- embeddings.rs          # NEW: EmbeddingProvider trait
|
+-- markymark-index/               # MODIFIED
|   +-- src/
|       +-- semantic.rs            # NEW: SemanticIndex
|
+-- markymark-mcp/                 # MODIFIED
|   +-- src/
|       +-- tools/
|           +-- semantic_search.rs # NEW: semantic-search tool
|
+-- markymark-parser/              # UNCHANGED
+-- markymark-lsp/                 # UNCHANGED
+-- markymark-cli/                 # UNCHANGED
+-- markymark-plugin/              # B LAYER (unchanged)
```

---

## 11. Implementation Phases

### Phase 0: Foundation
- [ ] This spec document
- [ ] Zig directory scaffold (build.zig, src/, test/)
- [ ] Zig build system producing libmarky_kernels.a (empty exports initially)
- [ ] markymark-kernels crate scaffold with build.rs

### Phase 1: Shared Kernels (fork from forge)
- [ ] Port embeddings.zig (create/add/search/destroy)
- [ ] Port similarity.zig (cosine + jaccard)
- [ ] Port entities.zig (FNV-1a entity extraction)
- [ ] Port quantize/dequantize (Q4)
- [ ] Port normalize (L2)
- [ ] Port abi.zig
- [ ] C ABI adapter for shared kernels
- [ ] Zig unit tests for all shared kernels
- [ ] Rust FFI wrappers: embed.rs, similarity.rs

### Phase 2a: Extraction Kernels (individual, then combined)
- [ ] fence_map.zig — SIMD code fence boundary detection (foundational)
- [ ] heading_scan.zig + scalar reference + tests
- [ ] link_scan.zig + scalar reference + tests
- [ ] tag_scan.zig + scalar reference + tests
- [ ] block_scan.zig + scalar reference + tests
- [ ] token_estimate.zig + tests
- [ ] content_hash.zig + tests
- [ ] slug.zig — SIMD slug generation from heading text
- [ ] C ABI adapter for extraction kernels
- [ ] Rust FFI wrappers: scan.rs, tokens.rs, hash.rs

### Phase 2b: Advanced Kernels
- [ ] multi_scan.zig — Single-pass Aho-Corasick scanner (replaces individual scans)
- [ ] fuzzy_match.zig — SIMD fuzzy string matcher for search-symbols
- [ ] link_graph.zig — Graph data structure with SIMD traversal
- [ ] index_serde.zig — Binary mmap-friendly index serialization
- [ ] formats/ — Multi-format extractors (env, ini, toml, json, yaml)

### Phase 3: Core Integration
- [ ] ScanBackend trait in markymark-core
- [ ] EmbeddingProvider trait in markymark-core
- [ ] zig-kernels feature flag
- [ ] ZigScanBackend implementation
- [ ] ZigEmbeddingIndex wrapper
- [ ] TreeSitterScanBackend (wraps current extraction into trait)

### Phase 4: Index Integration
- [ ] Wire ScanBackend into DocumentIndex extraction
- [ ] Add SemanticIndex to RealmIndex (optional, feature-gated)
- [ ] Near-duplicate detection logic
- [ ] Token estimation in realm-stats
- [ ] Content fingerprinting for cache invalidation
- [ ] Benchmark: Zig scan vs tree-sitter extraction

### Phase 5: MCP Integration
- [ ] semantic-search MCP tool
- [ ] Enhanced realm-stats (duplicates, tokens)
- [ ] Feature flag gating in CLI

### Phase 6: Validation
- [ ] Extraction parity tests (Zig vs tree-sitter, 600+ doc corpus)
- [ ] Embedding index lifecycle tests
- [ ] End-to-end semantic search test
- [ ] Benchmark suite
- [ ] CI integration (zig build step)
- [ ] False positive rate measurement

### Phase 7: Future Directions (Research)
- [ ] WASM compilation target for Zig kernels
- [ ] VS Code extension with WASM-accelerated markymark

---

## 12. Extended C ABI Surface (Phase 2b Additions)

### 12.1 Fence Map

| Function | Signature | Purpose |
|----------|-----------|---------|
| `marky_build_fence_map` | `(text: [*]const u8, len: u32, ranges: [*]FenceRange, cap: u32, written: *u32) -> i32` | Code fence boundary detection |

### 12.2 Multi-Pattern Scanner

| Function | Signature | Purpose |
|----------|-----------|---------|
| `marky_multi_scan` | `(text: [*]const u8, len: u32, fence_ranges: [*]const FenceRange, fence_count: u32, results: [*]ScanResult, cap: u32, written: *u32) -> i32` | Single-pass all-element extraction |

### 12.3 Fuzzy Matcher

| Function | Signature | Purpose |
|----------|-----------|---------|
| `marky_fuzzy_match` | `(query: [*]const u8, q_len: u32, candidates: [*]const CandidateStr, count: u32, scores: [*]f32, indices: [*]u32, top_k: u32) -> i32` | SIMD fuzzy search |

### 12.4 Link Graph

| Function | Signature | Purpose |
|----------|-----------|---------|
| `marky_graph_create` | `() -> ?*anyopaque` | Create graph handle |
| `marky_graph_destroy` | `(handle: *anyopaque) -> void` | Destroy graph |
| `marky_graph_add_doc` | `(handle: *anyopaque, id: u32, links: [*]const u32, link_count: u32) -> i32` | Add document with outbound links |
| `marky_graph_remove_doc` | `(handle: *anyopaque, id: u32) -> i32` | Remove document |
| `marky_graph_find_orphans` | `(handle: *anyopaque, orphans: [*]u32, cap: u32, written: *u32) -> i32` | Find docs with no inbound links |
| `marky_graph_pagerank` | `(handle: *anyopaque, scores: [*]f32, count: u32, iterations: u32) -> i32` | Compute PageRank |

### 12.5 Slug Generator

| Function | Signature | Purpose |
|----------|-----------|---------|
| `marky_slugify` | `(text: [*]const u8, len: u32, output: [*]u8, cap: u32) -> i32` | SIMD slug generation (returns written length) |

### 12.6 Binary Index Serialization

| Function | Signature | Purpose |
|----------|-----------|---------|
| `marky_index_serialize` | `(data: *const IndexData, output: [*]u8, cap: u32) -> i32` | Serialize index to binary |
| `marky_index_deserialize` | `(buf: [*]const u8, len: u32) -> ?*anyopaque` | Deserialize (mmap-friendly) |
| `marky_index_destroy` | `(handle: *anyopaque) -> void` | Free deserialized index |

---

## 13. Decisions Log

| ID | Decision | Rationale |
|----|----------|-----------|
| dec-brza-mm-001 | Copy and diverge Zig kernels from forge | Simplest start, avoids coordination overhead. Can sync later if patterns converge. |
| dec-brza-mm-002 | Complement tree-sitter with promotion path | Zig scans fast but context-unaware. Tree-sitter precise but slower. Benchmark to decide promotion. |
| dec-brza-mm-003 | markymark-kernels below markymark-core | Core defines traits, kernels implements. Other crates only depend on core. |
| dec-brza-mm-004 | Embedding source agnostic | EmbeddingProvider trait with pluggable implementations. Provider decision deferred. |
| dec-brza-mm-005 | P2 parallel exploration | Work alongside v1.0 launch. Not blocking. |
| dec-brza-mm-006 | Accept code-block false positives in Tier 1 | Complement strategy: precision from tree-sitter when needed. |
| dec-brza-mm-007 | Code fence exclusion map as shared primitive | Foundational kernel that enables context-aware scanning for all other kernels. Solves the promotion path's biggest design risk. |
| dec-brza-mm-008 | Single-pass Aho-Corasick replaces individual scans (optimization) | Build individual kernels first (simpler, testable), then combine into multi-pattern scanner. Individual kernels become stepping stones. |
| dec-brza-mm-009 | Zig SIMD fuzzy matcher for search-symbols | User-facing performance. Covers text search while embeddings cover semantic search. Two complementary search modes. |
| dec-brza-mm-010 | Zig link graph engine for document network analysis | Orphan detection, broken link chains, PageRank importance. Builds on forge petgraph patterns but Zig-native for SIMD traversal. |
| dec-brza-mm-011 | Binary mmap-friendly index serialization | Instant startup for large realms by memory-mapping serialized index. Extends the opaque Zig handle pattern from embedding index to full document index. |
| dec-brza-mm-012 | Zig slug generator kernel | Small kernel but called per-heading. SIMD ASCII lowercasing + space-to-hyphen is a clean SIMD win at scale. |
| dec-brza-mm-013 | Multi-format extractors (JSON/YAML/TOML/env/ini) | Ties into marky-lkj epic. SIMD key extraction for simple formats. Extends markymark from markdown-only to config-file-aware. |
| dec-brza-mm-014 | WASM + VS Code extension as future research | Zig compiles to WASM. Track as research direction. Enables browser-based markymark and VS Code web extension. |
| dec-brza-mm-015 | Zig 0.15.2 required (not 0.14.x) | Forge experience: agents using stale 0.14 patterns caused bugs. Must enforce 0.15.2 API in docs and build.zig. |
