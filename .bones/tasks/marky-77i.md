---
id: marky-77i
title: 'Phase 3: Incremental indexing for 10x speedup on markdown edits'
status: closed
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9b]
---












## Design

## Requirements (IMMUTABLE)

- Markdown documents with single-char edits in 50KB files reindex ≥10x faster than full rebuild (target: <1.3ms vs 13ms baseline)
- Incremental reindex produces identical DocumentIndex to full rebuild for same input (correctness parity)
- The 5 independent extractors (wiki_links, blocks, tags, markdown_links, xml_tags) use incremental update logic with range-based purge + neighbor validation
- Headings, TOC, and outline always rebuild fully (accept O(headings) cost for simplicity)
- JSON/YAML/TOML/JSONL/JSON5 documents always get full rebuild (Phase 4 will address)
- ServerState tracks pending InputEdit ranges between did_change and reindex
- All existing LSP features continue to work (document_symbol, hover, goto_definition, references, completion)

## Success Criteria (MUST ALL BE TRUE)

- [ ] Benchmark: single-char edit in 50KB markdown reindexes ≥10x faster than baseline (Phase 1 full rebuild)
- [ ] Correctness test: incremental_matches_full_rebuild() passes for all 5 independent extractors
- [ ] Stress test: 100 sequential single-char edits complete without errors
- [ ] Integration tests: document_symbol, hover, goto_definition work correctly after incremental update
- [ ] All existing workspace tests pass (468 tests across 54 suites)
- [ ] Zero clippy warnings, fmt clean
- [ ] Structured formats (JSON/YAML/TOML/etc.) still work with full rebuild

## Anti-Patterns (FORBIDDEN)

- ❌ NO incremental logic for headings/TOC/outline (simplicity: they're O(headings) not O(doc), cheap to rebuild. Complex delta algorithms for marginal gains violates YAGNI)
- ❌ NO incremental updates for structured formats in Phase 3 (scope: Phase 4 work, don't scope creep. JSON files are small, full rebuild is fast enough)
- ❌ NO AST tree diffing in index layer (complexity: range-based extraction is simpler and leverages existing InputEdit data. Tree diffing adds complexity without proven benefit)
- ❌ NO stable IDs or DocumentIndex redesign (breaking change: keep current structure, add update methods only. Stable IDs are Phase 5+ work if needed)
- ❌ NO skipping correctness tests (validation: incremental must match full rebuild. Optimization without correctness is a bug)

## Approach

Phase 3 implements selective re-extraction for markdown documents. When LSP receives did_change events, ServerState accumulates InputEdit ranges and passes them to build_markdown_index_incremental(). This method always rebuilds headings/TOC/outline (cheap O(headings)), then for each of the 5 independent extractors checks if InputEdit ranges intersect existing entries. If yes, it purges intersecting entries, re-extracts from changed ranges, validates neighbors, and merges. If no, it reuses old entries (zero cost). Structured formats bypass incremental logic and always use from_ast() full rebuild.

## Architecture

### LSP Layer (markymark-lsp/src/state.rs)
- ServerState gains pending_edits: Vec<InputEdit> field to track changes since last reindex
- build_markdown_index() checks if old index + old tree exist, routes to incremental path
- build_markdown_index_incremental() orchestrates: rebuild headings/TOC/outline, conditionally update 5 extractors, construct new DocumentIndex

### Index Layer (markymark-index/src/document.rs)
- Add helper methods: extractor_needs_update(), update_wiki_links(), update_blocks(), update_tags(), update_markdown_links(), update_xml_tags()
- Each update method: purge intersecting entries, extract from changed ranges, validate neighbors, merge and return
- DocumentIndex struct unchanged (no new fields)

### Parser Layer (markymark-parser)
- No changes (already has parse_with_old_tree and InputEdit support from Phase 1)

### Data Flow
```
did_change → apply_document_changes → accumulate InputEdit
           → build_markdown_index → route to incremental
           → build_markdown_index_incremental
              → rebuild headings/TOC/outline (always)
              → for each extractor:
                  if needs_update → purge + extract + validate + merge
                  else → reuse old entries
           → new DocumentIndex
```

## Design Rationale

### Problem
Phase 1 (marky-zan) achieved ~1.3x speedup via incremental tree-sitter parsing, not the 10x target. Benchmarks show tree-sitter-md's dual block+inline grammar limits parse-level gains. The bottleneck is DocumentIndex::from_ast() which rebuilds all 9 indexes from scratch even when only a few lines changed. For a 50KB document with single-char edit, 13ms is spent reindexing unchanged content.

### Research Findings

**Codebase:**
- markymark-index/src/document.rs:179-337 - DocumentIndex::from_ast() builds 9 indexes via full AST traversal
- 5 of 9 indexes are independent extractors (wiki_links, blocks, tags, markdown_links, xml_tags) - they don't depend on each other or headings
- Codebase investigation (Explore agent) found these 5 extractors are ~60% of total indexing cost
- Headings/TOC/outline are mandatory path but only ~10% of cost (O(headings) not O(doc))
- markymark-lsp/src/state.rs:208-228 - ServerState already tracks InputEdit from Phase 1 (marky-tzq)

**External:**
- tree-sitter incremental parsing best practices - range-based invalidation is standard approach
- Rust arena allocators (bumpalo) - reusing arena across updates is negligible benefit (<0.1% per marky-luy.2 benchmark)

### Approaches Considered

#### 1. LSP-Layer Orchestration ✓

**What it is:** ServerState tracks InputEdit ranges, calls build_markdown_index_incremental() which always rebuilds headings/TOC/outline and conditionally updates the 5 independent extractors using range-based purge + neighbor validation.

**Investigation:**
- Reviewed markymark-lsp/src/state.rs:208-228 - already has InputEdit tracking from marky-tzq
- Checked DocumentIndex structure - 5 extractors are independent, can be updated in isolation
- Benchmarked heading/TOC/outline rebuild cost - ~10% of total, cheap enough to always rebuild

**Pros:**
- Leverages existing InputEdit data from LSP layer (no new tracking infrastructure)
- Single orchestration point in build_markdown_index_incremental()
- Minimal cross-layer coupling (LSP knows about index structure, but already does via from_ast())
- Incremental adoption path (can start with one extractor, expand to others)

**Cons:**
- LSP layer has some knowledge of index internals (which extractors exist)
- Incremental logic not reusable outside LSP context (batch indexing tools would need separate path)

**Chosen because:** Leverages existing data, minimal new infrastructure, matches codebase pattern of LSP orchestrating index builds, supports incremental rollout.

#### 2. Index-Layer Diffing ❌

**What it is:** Pass both old_ast and new_ast to DocumentIndex. Index layer diffs the AST trees, identifies changed elements, patches indexes itself.

**Why we looked at this:** Index layer owning its update strategy seems cleaner encapsulation.

**Investigation:**
- Prototyped tree diffing logic - complex recursive walk to find added/removed/modified nodes
- Estimated memory overhead - keeping old_ast around doubles memory footprint during update
- Compared to range-based extraction - tree diff might be slower than just re-extracting from ranges

**Pros:**
- Clean encapsulation (index layer owns all update logic)
- Reusable outside LSP (batch tools, CLI)
- AST diffing centralized in one place

**Cons:**
- Index layer needs complex AST diffing implementation
- Memory overhead from keeping old_ast alive
- Tree diffing performance unclear, might be slower than re-extraction

**⚠️ REJECTED BECAUSE:** Adds significant complexity (tree diffing algorithm) without clear benefit over range-based extraction. Memory overhead (keeping old_ast) is wasteful when InputEdit ranges give us the same information cheaper.

**🚫 DO NOT REVISIT UNLESS:** We need incremental updates outside LSP context AND profiling shows range-based extraction is actually slower than tree diffing.

#### 3. Parser-Layer Change Tracking ❌

**What it is:** Parser returns (Ast, ChangeReport) with list of affected extractor types and ranges. Index layer reads change report and selectively updates.

**Why we looked at this:** Parser knows parse structure, might give more precise change hints than raw InputEdit ranges.

**Investigation:**
- Reviewed markymark-parser/src/lib.rs - Parser doesn't know about DocumentIndex extractors
- Would need to add ChangeReport struct and analysis logic to parser
- Couples parser to index concerns (which extractors exist, what needs updating)

**Pros:**
- Parser-level analysis could be more precise
- Index layer doesn't analyze ranges itself
- Change hints are explicit metadata

**Cons:**
- Violates separation of concerns (parser shouldn't know about index extractors)
- ChangeReport is extra metadata to create and maintain
- Parser would need to understand index structure (wrong layer)

**⚠️ REJECTED BECAUSE:** Wrong layer - parser shouldn't know about DocumentIndex structure. Range-based approach keeps layers properly separated.

**🚫 DO NOT REVISIT UNLESS:** We discover range-based invalidation is fundamentally flawed and need parse-time hints.

### Scope Boundaries

**In scope:**
- Incremental indexing for markdown documents
- The 5 independent extractors (wiki_links, blocks, tags, markdown_links, xml_tags)
- Range-based purge + neighbor validation merge strategy
- Always rebuild headings/TOC/outline (accept O(headings) cost)
- Correctness and performance testing
- JSON gets full rebuild (fast enough)

**Out of scope (deferred/never):**
- Incremental headings/TOC/outline (deferred - complex delta algorithms for marginal gains, violates YAGNI)
- Incremental for structured formats (deferred to Phase 4 - JSON/YAML/TOML/etc.)
- DocumentIndex redesign with stable IDs (deferred to Phase 5+ if needed - breaking change)
- Arena reuse optimization (never - marky-luy.2 showed <0.1% benefit, not worth complexity)
- Batch indexing incremental path (deferred - LSP is primary use case, batch tools can use full rebuild)

### Open Questions
- Should we parallelize the 5 extractor updates? (defer to implementation - profile first, parallelize if beneficial)
- What's the right neighbor validation distance? (defer to implementation - start with ±100 bytes, tune based on test failures)
- Should RealmIndex cross-doc aggregation be incremental too? (defer to Phase 4 - orthogonal concern)

## Design Discovery (Reference Context)

> Detailed context from brainstorming for task creation and obstacle handling.

### Key Decisions Made

| Question | User Answer | Implication |
|----------|-------------|-------------|
| Complexity budget? | Middle ground (keep structure + update methods) | No DocumentIndex redesign, add update_incremental() method only |
| Change granularity? | Byte-range (InputEdit ranges) | Leverage tree-sitter change tracking, simpler than element tracking |
| Which indexes incremental? | All 5 independent extractors | wiki_links, blocks, tags, markdown_links, xml_tags (60% of cost) |
| Headings/TOC/outline? | Always rebuild fully | No incremental logic for mandatory path (cheap O(headings)) |
| Merge strategy? | Hybrid (purge intersecting + validate neighbors) | More precise than range-only, less overhead than full dedup |
| Multi-format support? | Markdown + JSON in Phase 3, defer YAML/TOML/etc to Phase 4 | JSON always full rebuild, other formats deferred |
| JSON incremental? | Full rebuild for all structured formats in Phase 3 | Simplify scope, JSON files are small and fast anyway |

### Research Deep-Dives

#### DocumentIndex Structure Analysis
**Question explored:** What's the current indexing architecture and where's the bottleneck?

**Sources consulted:**
- Explore agent deep-dive of markymark-index crate
- DocumentIndex::from_ast() implementation (document.rs:202-337)
- Benchmark data from Phase 1 (marky-zan)

**Findings:**
- 9 total indexes: headings, slug_to_heading, blocks, toc, outline, wiki_links, tags, markdown_links, xml_tags
- 5 are independent extractors (no dependencies): wiki_links, blocks, tags, markdown_links, xml_tags
- These 5 account for ~60% of indexing time via regex/AST traversal
- Headings/TOC/outline are mandatory path but only ~10% of cost (O(headings) not O(doc))
- Each extractor does independent full tree walk - parallelizable

**Conclusion:** Target the 5 independent extractors for incremental, accept full rebuild for headings/TOC/outline (cheap). This gives most of the 10x speedup with minimal complexity.

#### Merge Strategy Options
**Question explored:** How should we merge re-extracted entries with old index data?

**Sources consulted:**
- Incremental data structure literature (LSM trees, delta indexing)
- Rust arena allocation patterns from Phase 1 work

**Findings:**
- Pure range-based purge: simple but conservative, may remove unchanged entries
- Content-based dedup: precise but requires hashing all entries (overhead)
- Hybrid purge + validation: purge intersecting, validate neighbors, good balance

**Conclusion:** Hybrid approach balances precision and performance. Validate neighbor entries (they might have shifted) without hashing everything.

### Dead-End Paths

#### Arena Reuse for Incremental Updates
**Why explored:** Thought reusing the arena across updates might save allocation overhead.

**Investigation:**
- Created benchmark in marky-luy.2
- Small doc arena lifecycle: 662ns vs 64µs full reparse (0.07%)
- Large doc: 22.5µs vs 3.36ms (0.02%)
- Tree-sitter parsing dominates by 100-150x

**Why abandoned:** Sub-0.1% improvement doesn't justify API changes across 4 crates. Incremental benefit is in skipping extraction, not arena reuse.

#### Incremental Headings/TOC/Outline
**Why explored:** User mentioned 10x speedup, thought headings might need incremental too.

**Investigation:**
- Analyzed build_toc() and build_outline() algorithms - both are stack-based sequential
- Measured cost: ~10% of total indexing time
- Delta algorithms would need to detect add/remove/move of headings, patch tree structure
- Complexity high, benefit low (O(headings) is already cheap)

**Why abandoned:** YAGNI violation - complex delta algorithms for 10% of cost. Full rebuild is simple and fast enough.

### Open Concerns Raised

- "What if an edit shifts all subsequent entry positions?" → Neighbor validation catches position shifts, and range intersection ensures we re-extract the affected region
- "Do we need to update RealmIndex cross-doc aggregation too?" → Deferred to Phase 4, orthogonal concern. RealmIndex gets updated via existing add_document() path
- "Should the 5 extractors run in parallel?" → Defer to implementation, profile first. They're independent but overhead might exceed benefit for typical docs
- "What about very large markdown files (>1MB)?" → Benchmark will validate. If incremental doesn't help, we can add heuristic to fall back to full rebuild for huge files
