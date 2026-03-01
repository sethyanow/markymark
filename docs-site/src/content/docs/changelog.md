---
title: Changelog
description: Release history for markymark
---

All notable changes to markymark are documented here. Each release links to the full diff on GitHub.

## v0.7.0 (2026-02-28)

Adds semantic search powered by vector embeddings, giving AI agents the ability to find relevant document sections by meaning rather than exact text. Supports both cloud (Voyage API) and fully local (ONNX via fastembed) embedding providers. Several concurrency fixes harden the MCP workspace management layer.

### Features

- **Semantic search** — new `semantic-search` MCP tool ranks document sections by relevance to natural-language queries, with `top_k` and `min_score` controls
- **Voyage embedding provider** — cloud embeddings via the Voyage API, gated behind the `semantic-search` feature flag
- **Local ONNX embeddings** — fully offline embeddings via fastembed-rs, gated behind the `local-embeddings` feature flag
- **Batch embedding on startup** — workspace roots are embedded in batch during `add-root` indexing
- **Incremental re-embedding** — `update_document()` re-embeds changed sections without full workspace reindex

### Bug Fixes

- Split semantic search into separate embed and query phases to reduce mutex serialization across concurrent searches
- Snapshot-then-rollback for atomic `add_document` — embedding failures no longer leave partial entries in the semantic index
- Clean up semantic entries on concurrent root removal
- Prevent `add-root` reinsertion after concurrent `remove-root`
- Release state write lock before `add-root` indexing to avoid blocking concurrent MCP requests
- Release realm read lock before semantic search await to prevent deadlock

### Refactoring

- Semantic module extracted into directory with data types, helpers, add/remove operations, update/search operations, and tests as separate submodules
- Engine tests split into 3 files to stay under 500-line threshold

### Infrastructure

- Embedding smoke test suite — local ONNX and Voyage provider test scripts with JSON-RPC assertions
- Feature flag consolidation — `voyage` flag folded into `semantic-search`

**Full diff:** [v0.6.0...v0.7.0](https://github.com/sethyanow/markymark/compare/v0.6.0...v0.7.0)

---

## v0.6.0 (2026-02-25)

Completes the extraction pipeline migration started in v0.5.0. All remaining symbol types — code spans, XML tags, tasks, embeds, callouts, block refs, link definitions, query blocks, and properties — are now extracted through the full Zig→FFI→Rust→LSP/MCP stack. The legacy regex-based Rust extractors have been removed; `zig-kernels` is now mandatory. The RealmIndex has been rebuilt as v2 with string interning and incremental document updates.

> **Note:** Agents like Claude Code use both LSP and MCP simultaneously — they are complementary protocols, not alternative paths. This release completes MCP parity with LSP for all extracted element types.

### Features

- **Complete extraction pipeline (ix3)** — 9 new element types extracted by the Zig md4c ExtractionRenderer, serialized in blob v2 format (128-byte header with backward compatibility), and wired through `from_blob`, `from_scan`, `ScanBackend`, and LSP/MCP
- **RealmIndex v2 (n7wx)** — String interning via `lasso` deduplicates URI and heading allocations; new `update_document()` computes contribution diffs incrementally; O(1) `stem_to_uris` index for wiki link resolution; lazy `tag_to_docs` built on first access
- **Blob v2 header** — 128-byte header with v1/v2 backward compatibility
- **MCP batch indexing** migrated from `from_ast` to `from_scan`

### Bug Fixes

- Preserve frontmatter and mask it before engine/scan parsing
- Pick earliest frontmatter close delimiter in mixed-ending files
- Property scan past non-property lines and CRLF frontmatter handling
- Correct offset recovery for code spans and tasks
- Windows: append `.exe` to binary path and download URL
- VSCode: return bare name for PATH fallback on unsupported platforms

### Breaking Changes

- `zig-kernels` is now mandatory — regex extractors have been removed
- Zig 0.15.2+ required for all builds (enforced by `build.rs`)

### Refactoring

- `scanner.rs` split into 4 submodules
- `from_blob.rs` converted to module directory with `header.rs`, `owned.rs`, and 4 test submodules
- `document.zig` split into helpers, free functions, stored types, and FFI types
- `extraction_renderer_tests.zig` split into 4 thematic test files
- `extract.rs` converted to submodule directory with frontmatter, tasks, blocks, links, and tags

### Infrastructure

- Zig 0.15.2 archive corruption fix — archives repacked with system `ar` on Linux targets

**Full diff:** [v0.5.1...v0.6.0](https://github.com/sethyanow/markymark/compare/v0.5.1...v0.6.0)

---

## v0.5.1 (2026-02-21)

Focused improvements to the Zig md4c parsing pipeline and Rust error handling.

### Bug Fixes

- **O(n) autolink paren trimming** — the GFM autolink parenthesis-balancing loop in the Zig md4c port was quadratic; rewritten as a single-pass O(n) scan

### Improvements

- `BlobError` now implements `Display` and `Error` traits, enabling `Box<dyn Error>`, `?` operator, and `anyhow` integration with per-variant diagnostic messages
- Extract `map_md4c_heading` / `map_md4c_link` helpers in `Md4cScanBackend`

**Full diff:** [v0.5.0...v0.5.1](https://github.com/sethyanow/markymark/compare/v0.5.0...v0.5.1)

---

## v0.5.0 (2026-02-20)

Replaces the tree-sitter incremental indexing pipeline with a new md4c-based DocumentEngine. The new pipeline vendors Bun's md4c Zig parser for single-pass markdown extraction, serializes results to a compact binary blob format, and crosses the FFI boundary into Rust — eliminating double-parse overhead.

### Features

- **New md4c parsing pipeline** — Vendored Bun's md4c CommonMark parser (Zig), streaming `ExtractionRenderer` extracts headings, links, tags, and block IDs in a single pass, exposed through C ABI with Rust FFI bindings
- **DocumentEngine with blob serialization** — Zig `DocumentEngine` produces a compact binary blob; Rust-side `DocumentIndex::from_blob()` deserializes without re-parsing
- **LSP pipeline overhaul** — `scan_all` replaces tree-sitter incremental indexing, eliminating the previous double-parse (tree-sitter parse + separate extraction pass)
- **Async debounce** — `did_change` notifications debounced at 75ms with generation counters preventing stale batches after close/reopen cycles

### Bug Fixes

- Removed undefined behavior in `DocumentIndex` arena_ref mutex escape
- Removed unsound `Sync` impl on `DocumentIndex`
- Fixed `extractFromMarkdown` double-free and heading text leak on OOM
- Fixed `toOwnedSlice` cascade leak and append double-frees in `parseAll`
- Fixed `normalizeLabel` memory leak in vendored md4c parser
- Wiki link alias detection: compare `text != target`
- Slug truncation no longer returns empty string

### Breaking Changes

- Tree-sitter incremental indexing replaced by md4c-based DocumentEngine
- New document processing pipeline — plugins relying on tree-sitter internals will need updates

### Infrastructure

- Release process formalized in RELEASING.md with 7-crate publish order
- `prepare-release` skill added for guided 4-phase release workflow
- Golden blob roundtrip test catches unilateral Zig/Rust format drift

**Full diff:** [v0.4.2...v0.5.0](https://github.com/sethyanow/markymark/compare/v0.4.2...v0.5.0)

---

## v0.4.2 (2026-02-20)

### Features

- **get-diagnostics MCP tool** — new MCP tool with `file://` URI validation and structured doc support

### Bug Fixes

- Include new entries from large insertions in incremental merge
- Correct assertion message in realm_stats preview check
- Create parent dirs in `TempWorkspace::write` for nested paths

### Performance

- Eliminate eager allocation of heading names in `SearchSymbols`

### Refactoring

- Split `incremental/tests.rs` into submodules
- Split `runtime_engine_tests.rs` into 9 submodules
- Move `realm.rs` to `realm/` module directory with `types.rs`, `helpers.rs`, and `tests.rs`
- Extract shared `TempWorkspace` test helper and migrate across test suites

**Full diff:** [v0.4.1...v0.4.2](https://github.com/sethyanow/markymark/compare/v0.4.1...v0.4.2)

---

## v0.4.1 (2026-02-19)

### Features

- **LSP debounce** — `did_change` notifications debounced with 75ms async cancellation

### Bug Fixes

- Detect edits in large gaps between extractor entries during incremental indexing
- Deduplicate link edges per document in graph analysis
- Improve markdown link resolution with path-relative lookup

**Full diff:** [v0.4.0...v0.4.1](https://github.com/sethyanow/markymark/compare/v0.4.0...v0.4.1)

---

## v0.4.0 (2026-02-19)

Major release introducing Zig SIMD acceleration kernels, MCP intelligence tools, incremental indexing, and a VSCode extension.

> **Note:** Agents like Claude Code use both LSP and MCP simultaneously — they are complementary protocols, not alternative paths. This release adds MCP tools that pair with the existing LSP capabilities.

### Features

- **Zig SIMD kernels** — Format scanners (`env_scan`, `ini_scan`, `toml_scan`, `yaml_scan`, `json_keys`) with SIMD-accelerated key extraction; link graph engine; batched fuzzy match; Aho-Corasick multi-pattern scanner; slug generation via C ABI
- **MCP tools** — `search-workspace` (full-text search with frontmatter/property/tag filtering), `search-for-pattern` (regex search with glob filtering and context lines), `graph-analysis` (link graph intelligence with orphans, hubs, broken links, clusters)
- **Incremental indexing (Phase 3)** — All 5 extractors carry byte offsets for selective merge; range intersection with neighbor window and tail-boundary guard; benchmarked 1.23x speedup
- **VSCode extension** — Marketplace-ready extension with binary discovery and LSP client

### Bug Fixes

- Implement `___chkstk_ms` for Windows x86_64 to resolve stack frame issues
- CRLF offset drift fix in incremental indexing
- Insertion-point boundary correction

### Breaking Changes

- New feature-gated Zig dependency (`zig-kernels` feature flag)

### Refactoring

- Split `state.rs` (1170 lines) into `state/{mod,completion,navigation,rename}.rs`
- Renamed `runtime_engine` to `engine` with submodules
- Extracted tool handlers into `tools/` submodule

**Full diff:** [v0.3.0...v0.4.0](https://github.com/sethyanow/markymark/compare/v0.3.0...v0.4.0)

---

## v0.3.0 (2026-02-16)

Introduces the Zig SIMD kernel foundation and incremental tree-sitter parsing.

### Features

- **Zig kernel scaffold** — `zig/` directory with `build.zig`, source structure, and `markymark-kernels` Rust crate with FFI bridge
- **SIMD kernels** — `heading_scan`, `link_scan`, `tag_scan`, `block_scan`, `token_estimate`, `content_hash` implemented in Zig with Rust FFI wrappers
- **Shared BRZA kernels** — Similarity, normalize, entities, quantize, and embedding index kernels ported with Rust FFI wrappers
- **Incremental tree-sitter parsing** — `MarkdownTree` stored per document, `TextDocumentSyncKind::INCREMENTAL` enabled, incremental parsing wired end-to-end
- **Core traits** — `ScanBackend` and `EmbeddingProvider` traits added to `markymark-core`

### Bug Fixes

- Enable PIC in Zig static library for Linux x86_64 linking
- Defensive FFI hardening: replace `as u32` casts with `try_from` at FFI boundary
- Initialize written parameter in scan FFI functions
- Skip invalid incremental edits when `old_end < start`

### Breaking Changes

- First Zig dependency introduced (optional via `zig-kernels` feature flag)

**Full diff:** [v0.2.0...v0.3.0](https://github.com/sethyanow/markymark/compare/v0.2.0...v0.3.0)

---

## v0.2.0 (2026-02-16)

Introduces arena allocation, multi-format structured document support, and security hardening.

### Features

- **Arena allocation** — `bumpalo`-based arena infrastructure in `markymark-core`, migrated through parser and index layers with `DocumentArena` and `ArenaHashMap`
- **Multi-format support** — JSON, JSONC, JSON5, JSONL, YAML, TOML, `.env`, and INI parsers with `StructuredDocumentIndex`, `AnyDocumentIndex`, and `RealmIndex` integration
- **LSP multi-format** — `DocumentSymbols`, hover, and find-references for structured documents
- **Tree-sitter migration** — Upgraded from tree-sitter 0.19 to 0.26 with `tree-sitter-md` wrapper
- **Security** — Advisory security workflow with SARIF uploads, lefthook pre-commit hooks, custom semgrep rules with fixture validation
- **Plugin distribution** — `marketplace.json` for self-hosted plugin distribution

### Bug Fixes

- Resolve `frontmatter_and_properties` SIGSEGV
- Preserve duplicate block IDs across documents
- Ignore XML-like syntax inside fenced code blocks
- `BlockEntry` range propagation for go-to-definition

### Performance

- Optimize `remove_from_cross_doc_indexes` to O(doc size)
- Real corpus benchmarks added

### Refactoring

- Split `types.rs` into submodules below 1000 LOC

**Full diff:** [v0.1.0-alpha.2...v0.2.0](https://github.com/sethyanow/markymark/compare/v0.1.0-alpha.2...v0.2.0)

---

## v0.1.0-alpha.2 (2026-02-13)

Second alpha release focused on plugin distribution improvements.

### Features

- CI per-platform pre-packaging for plugin binary distribution
- Download-on-first-run fallback for marketplace installs

**Full diff:** [v0.1.0-alpha.1...v0.1.0-alpha.2](https://github.com/sethyanow/markymark/compare/v0.1.0-alpha.1...v0.1.0-alpha.2)

---

## v0.1.0-alpha.1 (2026-02-13)

Initial alpha release of markymark — a high-performance Markdown LSP server built in Rust. Includes core LSP capabilities: document symbols, go-to-definition, find-references, rename, hover, completion, and diagnostics for Markdown files with wiki-link support.

**Full changelog:** [v0.1.0-alpha.1](https://github.com/sethyanow/markymark/commits/v0.1.0-alpha.1)
