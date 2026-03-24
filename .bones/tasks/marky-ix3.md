---
id: marky-ix3
title: '[EPIC] ix3: Code span extraction + Zig extraction consolidation'
status: open
status: closed
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8s3, marky-bt3e, marky-pdyo]
---













Investigate feasibility of cross-language symbol resolution — finding mentions of Rust symbols (structs, traits, fns) in markdown docs and vice versa.

## Findings So Far (2026-02-16 testing session)

### Current State
- markymark only indexes .md files. Adding Rust source roots silently skips .rs files (89 docs regardless of how many roots added).
- Built-in LSP (rust-analyzer) indexes Rust only. workspaceSymbol surfaces markdown headings alongside Rust symbols, but that's the markdown language server contributing to the same LSP protocol — not cross-language linking.
- No tool today can answer 'find all mentions of DocumentArena across both code and docs.'

### Test Results

| Query | markymark MCP | Built-in LSP workspaceSymbol |
|-------|---------------|-------------------------------|
| DocumentArena (exact) | 0 results — no heading match | Found struct in arena.rs:59 |
| document arena (fuzzy) | 1 result — heading in bumpalo.md | Same struct result |

### Gap Analysis
1. markymark search-symbols only matches heading text, not inline code references (`DocumentArena` in prose)
2. No code fence parsing — Rust symbols inside markdown code blocks are invisible to markymark
3. No reverse lookup — given a Rust symbol, no way to find which docs discuss it

### Potential Approaches
- A: Parse inline code spans and code fences in markdown, extract identifiers, cross-reference with workspace symbol index
- B: Add a grep-based fallback tool that searches both .md and .rs files for a pattern
- C: Extend export-index to include code span contents as a new symbol type
- D: Build a dedicated cross-reference index that combines markymark heading/tag data with rust-analyzer symbol data

### Assessment
This is a differentiating feature for markymark — no existing markdown LSP does this. Most valuable for large Rust projects with extensive docs (exactly our use case). Approach C (code spans as symbols) is probably the lowest-effort highest-value starting point.

## Design

## Requirements (IMMUTABLE)

1. CodeSpanEntry type in DocumentIndex with: text, range, start_byte, end_byte, language_hint (Option), kind (Option)
2. Tier 1 extraction (inline backtick code spans) works across all 3 construction paths: from_scan (Zig FFI), from_blob (blob v2), from_ast (extract.rs regex)
3. Blob v2 format adds code_span_count and reserves slots for ALL planned Phase B extraction types (no v3 bump needed)
4. ScanBackend trait extended with scan_code_spans() method
5. LSP workspaceSymbol returns code span matches alongside headings/tags
6. LSP hover on backtick text shows cross-document references
7. MCP search-symbols returns code span matches
8. fgl8 (extract.rs split) completed before any extract.rs code span additions
9. Phase B: All 11 markdown-content extractors migrate from Rust regex to Zig ExtractionRenderer, one at a time, tests green at every step
10. Phase C end state: extract.rs retains only frontmatter + from_ast orchestration; all markdown-content extraction owned by Zig ExtractionRenderer
11. RealmIndex dedup by (identifier, uri) for code span entries
12. from_blob handles both v1 and v2 blobs transparently (backward-compatible)

## Success Criteria (MUST ALL BE TRUE)

- [ ] CodeSpanEntry type in document/types.rs with optional kind and language_hint
- [ ] Tier 1: backtick inline code spans extracted via all 3 paths
- [ ] Blob v2: code_span_count field, reserved slots for future types, backward-compatible v1 read
- [ ] ScanBackend::scan_code_spans() implemented for ZigScanBackend and Md4cScanBackend
- [ ] LSP workspaceSymbol returns code span results
- [ ] LSP hover shows code span backlinks
- [ ] MCP search-symbols includes code spans
- [ ] extract.rs split into submodules (fgl8 complete)
- [ ] All 11 extractors migrated from extract.rs to Zig ExtractionRenderer (except frontmatter)
- [ ] extract.rs reduced to frontmatter + orchestration shim
- [ ] RealmIndex dedup for code span entries
- [ ] All existing tests pass after each migration step
- [ ] New tests for code span extraction on each path
- [x] CodeSpanEntry type in document/types.rs with optional kind and language_hint
- [x] Tier 1: backtick inline code spans extracted via all 3 paths
- [x] Blob v2: code_span_count field, reserved slots for future types, backward-compatible v1 read
- [x] ScanBackend::scan_code_spans() implemented for ZigScanBackend and Md4cScanBackend
- [x] LSP workspaceSymbol returns code span results
- [x] LSP hover shows code span backlinks
- [x] MCP search-symbols includes code spans
- [x] extract.rs split into submodules (fgl8 superseded — Zig migration eliminated split target)
- [x] All 11 extractors migrated from extract.rs to Zig ExtractionRenderer (except frontmatter)
- [x] extract.rs reduced to frontmatter + orchestration shim
- [x] RealmIndex dedup for code span entries
- [x] All existing tests pass after each migration step
- [x] New tests for code span extraction on each path
- [ ] Pre-commit hooks passing

## Anti-Patterns (FORBIDDEN)

- NO tree-sitter AST walking for code span extraction (md4c/Zig is strategic path; tree-sitter extraction is throwaway)
- NO confidence field in Tier 1 (all backtick spans are definite; confidence adds complexity for zero Tier 1 value)
- NO adding extraction to extract.rs before fgl8 split completes (862 lines, 1000-line hard stop)
- NO big-bang extractor migration (each extractor migrates independently: Zig -> blob -> wiring -> remove Rust regex -> tests green)
- NO frontmatter migration to Zig (YAML/TOML parsing has mature Rust crates, Zig gains nothing)
- NO blob v3 bump within ix3 scope (v2 must reserve enough slots for all planned migrations)
- NO breaking v1 blob reads (from_blob must handle both v1 and v2 blobs gracefully)

## Approach

Three-phase extraction consolidation:

**Phase A (Tier 1 Code Spans):** Add CodeSpanEntry type. Implement backtick inline code span extraction across all 3 DocumentIndex construction paths: Zig ExtractionRenderer (from_scan), blob v2 format (from_blob), and extract.rs regex (from_ast). Extend ScanBackend trait. Surface via LSP workspaceSymbol, hover, and MCP search-symbols. Dedup in RealmIndex.

**Phase B (Zig Extraction Migration):** Migrate ~11 extractors from Rust regex (extract.rs) to Zig ExtractionRenderer, one at a time. Each migration follows: add Zig callback -> add blob section -> wire from_scan/from_blob -> remove Rust regex -> tests green. Extractors: wiki_links, markdown_links, link_definitions, block_ids, block_refs, tags, embeds, tasks, callouts, query_blocks, xml_tags, page_properties.

**Phase C (Shim Completion):** Trim extract.rs to frontmatter + from_ast orchestration only. All markdown-content extraction owned by Zig ExtractionRenderer, serialized via blob format.

## Architecture

Data flow (all 3 paths):
- from_ast: tree-sitter AST -> extract/code_spans.rs (regex) -> CodeSpanEntry
- from_scan: Zig ExtractionRenderer -> FFI -> ScanBackend::scan_code_spans() -> CodeSpanEntry
- from_blob: Zig DocumentEngine -> blob v2 -> from_blob deserialize -> CodeSpanEntry

Key files (Phase A):
- Types: document/types.rs (CodeSpanEntry, CodeSpanOwned)
- Zig extraction: extraction_renderer.zig (ExtractedCodeSpan, enterSpan/leaveSpan for code)
- Zig blob: blob.zig (BlobCodeSpan struct, header v2 with generous reserved slots)
- Zig engine: document.zig (code span lifecycle)
- Rust trait: scanner.rs (ScanBackend::scan_code_spans(), ScanAllResult.code_spans)
- from_scan: document/mod.rs (wire code spans)
- from_blob: document/from_blob.rs (v2 deserialization, v1 backward-compat)
- from_ast: extract/code_spans.rs (post-fgl8, regex backtick extraction)
- LSP: server.rs (workspaceSymbol + hover)
- MCP: tools/search.rs (search-symbols)
- RealmIndex: realm/mod.rs (dedup, cross-doc index)

Blob v2 design principle: Header expands reserved space to accommodate code_span_count plus future extraction type counts. Exact layout at implementation time, but must not require v3 for any Phase B migration. from_blob reads v1 (no code spans) and v2 (with code spans) transparently.

Phase B migration pattern (per extractor):
1. Add callback handling in ExtractionRenderer (Zig)
2. Add blob section + header count field
3. Wire ScanBackend method + from_scan processing
4. Wire from_blob deserialization
5. Remove regex from extract.rs submodule
6. Tests green at every step

Phase C end state: extract.rs -> extract/mod.rs containing only frontmatter/YAML/TOML parsing and from_ast orchestration.

## Design Rationale

### Problem
markymark cannot answer 'find all docs mentioning DocumentArena.' search-symbols only matches heading text. Additionally, extract.rs (862 lines of Rust regex) duplicates work the Zig ExtractionRenderer already does for headings and links. Consolidating extraction in Zig aligns with Option H trajectory and eliminates the two-pipeline maintenance burden.

### Research Findings (2026-02-20 refinement session)

**Codebase (verified via LSP + codebase-investigator):**
- extraction_renderer.zig: SpanType::code fires in md4c callbacks, silently ignored (line 204)
- blob.zig: ScanBlobHeader v1 has 16 reserved bytes at offset 48 (line 37)
- scanner.rs: ScanBackend has 6 methods, no scan_code_spans (lines 104-130)
- types.rs: 18 entry types, no CodeSpanEntry (lines 1-266)
- extract.rs: 862 lines, 12 regex extractors, no code span extraction
- from_blob.rs: expects blob version 1 only (line 106-167)
- server.rs workspace_symbol: searches headings/tags/xml_tags only (lines 794-885)

### Approaches Considered

#### 1. Three-phase Zig consolidation (Chosen)
Code spans first via all 3 paths, then migrate all extractors to Zig, then trim extract.rs.
Chosen because: eliminates Rust/Zig extraction duplication, aligns with Option H, blob format becomes authoritative extraction transport, single maintenance surface.

#### 2. Code spans only, keep extract.rs as-is
Add code span extraction without migrating other extractors.
REJECTED BECAUSE: perpetuates dual-pipeline maintenance (Rust regex + Zig ExtractionRenderer). Each new extraction feature would need implementation in both places.
DO NOT REVISIT UNLESS: Zig ExtractionRenderer proves too complex for non-trivial extractors.

#### 3. Drop from_ast path, Zig-only extraction
Only extract code spans via Zig paths (from_scan, from_blob). Skip extract.rs entirely.
REJECTED BECAUSE: MCP batch indexing uses from_ast (tree-sitter). Generated docs added via add-root would silently lack code spans. User requires all 3 paths.
DO NOT REVISIT UNLESS: from_ast is removed from the codebase.

### Scope Boundaries
**In scope:** Tier 1 code spans, all 3 extraction paths, LSP+MCP surfaces, 11-extractor Zig migration, blob v2
**Out of scope:** Tier 2 (code fences), Tier 3 (prose heuristics), confidence scoring, rust-analyzer integration, goto-definition to Rust source, auto-completion, frontmatter migration

### Key Decisions (from brainstorming)
- All 3 construction paths must extract code spans from day one (no silent gaps)
- fgl8 is prerequisite (extract.rs split before any additions)
- Zig consolidation is in-scope for ix3 (not a separate epic)
- Everything except frontmatter migrates to Zig
- Blob v2 reserves generously (no v3 needed for Phase B)
- kind field is Optional (Tier 1 cannot determine struct/fn/trait)
- Confidence deferred to Tier 2/3

## Log

- [2026-03-23T13:20:50Z] [Seth] Implementation review APPROVED. All 14 success criteria verified with evidence. 1412/1412 tests pass, clippy clean, all 7 anti-patterns clear. fgl8 closed as superseded (Zig migration eliminated the split target). 20+ code span tests across 8 test modules, covering Zig FFI, ScanBackend, from_scan, from_blob (incl. v1 backward compat), RealmIndex dedup/cross-doc, MCP search. T3 observation: LSP integration tests lack explicit code span scenarios (low risk, mechanical dispatch).
