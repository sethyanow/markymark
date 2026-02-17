# Agent Memory — Cross-Session Assessments

**Purpose:** Track quality assessments, improvement decisions, and lessons learned
across sessions. This file is linked from CLAUDE.md and auto-loaded at session start.

**Rules:**
- Append-only — never delete entries, only add new ones
- Each entry is timestamped and categorized
- Assessments include evidence, not just opinions
- Link to beads issues for traceability

---

## Rust Agent Docs Quality Tracker

### Assessment: 2026-02-15 (Session: feature/mark-rustdocs)

**Grade: A-** (up from B+ after 8 improvements)

**Evidence-based rating:**
- Evaluated 42 files (4,973 lines) against official Rust sources (book, reference, nomicon, cargo, clippy, rbe)
- Cross-referenced with 5 actual project failures from harness memory
- Dogfooded markymark MCP tools on the corpus (realm indexing, search, export-index)

**What works well:**
- Decision trees are the strongest feature — 11 total covering smart pointers, errors, conversions, atomics, patterns, macros, closures, Send/Sync, cancellation safety
- Three-level progressive disclosure (L0 index → L1 overview → L2 detail) maps well to agent retrieval patterns
- Mistake tables with severity ratings catch the most dangerous errors
- The `COMMON MISTAKE` callouts in individual files are high-signal

**What caused 2-3x passes before improvements:**
1. No closure/Fn trait coverage at all — involved in ~40% of Rust code
2. Send/Sync table existed but no propagation rules or diagnostic flow
3. No borrow splitting or mem::take patterns — agents fought the borrow checker
4. No async testing or cancellation safety guidance
5. AGENTS.md used backtick refs not clickable links

**What was fixed (marky-5n9):**
- Created `core/closures.md` (156 lines) — FnOnce/FnMut/Fn hierarchy, capture semantics, move, decision tree
- Expanded Send/Sync with auto-derivation rules and real project failure example (Bump !Sync chain)
- Added borrow splitting + mem::take/replace/swap to ownership.md
- Added custom Iterator implementation + IntoIterator triple to collections.md
- Added async testing, JoinSet, async traits, cancellation safety to async.md
- Added HRTB, lifetime subtyping, self-referential struct solutions to ownership.md
- Added prerelease version semantics + feature unification to cargo.md
- Converted all AGENTS.md nav to clickable Markdown links

**Remaining gaps (all closed in marky-9ya):**
~~1. Compiler error walkthroughs~~ → Added 4 step-by-step walkthroughs to compiler-errors.md
~~2. Cookbook/recipes~~ → Created patterns/cookbook.md with 5 complete recipes
~~3. Cross-cutting guides~~ → Created patterns/async-ready.md (Send/Sync/Pin/'static combined)
~~4. Language migration bridges~~ → Created reference/migration-bridges.md (Python/TS → Rust)
~~5. Real failure mining~~ → Added 6 real failures from harness memory to anti-patterns.md

### Assessment Update: 2026-02-15 (Session 2: marky-9ya)

**Grade: A** (up from A- after closing all 5 gaps)

**What was added:**
- `reference/compiler-errors.md`: 4 detailed walkthroughs (E0382, E0502, E0277/Send, E0716) + error-reading protocol
- `patterns/cookbook.md`: 5 recipes — config parsing, newtype suite, trait object service, iterator chain, Arc+Mutex shared state
- `patterns/async-ready.md`: Cross-cutting checklist combining Send, Sync, Pin, 'static with real markymark arena examples
- `reference/migration-bridges.md`: Python/TS → Rust side-by-side translations covering 9 concept categories + 7 common traps
- `patterns/anti-patterns.md`: 6 real-world failures from harness memory (stale API trust, MCP framing, arena SIGSEGV, stack temporary, key overwrite, semver mismatch)
- 3 new mistakes in MISTAKES.md (arena clone SIGSEGV, arena empty slice, stale docs trust)
- All index files updated (AGENTS.md, README.md, _index.md files, CLAUDE.md docs_index)

**Stats:** 45 files, 6,432 lines (up from 5,033). 14 decision trees. 16 mistakes tracked.

### Assessment Update: 2026-02-15 (Session 3: source vault final pass)

**Grade: A** (confirmed — gap analysis found no remaining structural gaps)

**Method:** Indexed 633 source vault docs via markymark realm (`source-vault`). Searched
headings systematically for 15+ topic areas. Cross-referenced vault coverage against our docs.

**Gaps found and filled (+172 lines, 6,271 → 6,443 total):**
- `core/ownership.md`: Added "RefCell in Practice" section — borrow()/borrow_mut() mechanics, panic risk, Rc<RefCell<T>> pattern, agent pitfall callout
- `patterns/idioms.md`: Added "Drop Order Rules" subsection — variable vs struct vs tuple drop order, early drop with mem::drop(), recursive drop behavior
- `patterns/idioms.md`: Expanded "Deref Polymorphism" with 3 automatic coercion rules and 6 common auto-coercion examples agents should know
- `core/types.md`: Added `matches!` macro and match guards to pattern matching section
- `reference/syntax-ref.md`: Added "Panic & Stub Macros" table — todo!/unimplemented!/unreachable!/panic! with usage guidance
- `patterns/anti-patterns.md`: Added derived Clone with generic Arc fields gotcha (from nomicon dot-operator.md)
- `MISTAKES.md`: Added mistakes #17 (RefCell double borrow panic) and #18 (struct field drop order assumption)
- `AGENTS.md`: Updated mistake quick-ref table with new entries

**Topics confirmed adequate (no changes needed):**
- Pin/Unpin: Well covered in async-ready.md + async.md
- Iterators: Solid cheatsheet + IntoIterator triple + custom Iterator in collections.md
- Object safety: 5 rules in traits.md
- From/Into: Decision tree in traits.md + examples in cookbook.md
- HRTB: Covered in ownership.md
- Builder pattern: Covered in idioms.md
- cfg/conditional compilation: Covered in rules.md, syntax-ref.md, modules.md
- Turbofish syntax: Covered in syntax-ref.md
- Option/Result combinators: Full table in errors.md

**Topics assessed as too niche to add:**
- Variance/covariance: Nomicon-level; PhantomData table in unsafe.md is sufficient
- ?Sized/DST: Deref coercion handles most agent scenarios transparently
- Full type coercion list: Reference-level detail beyond agent needs

**markymark dogfooding findings:**
- XML tag parsing inside fenced code blocks produces false positives (filed marky-8la)
- 198 false positive "unclosed XML tag" warnings from Rust generics like `<T>`, `<Mutex<T>>`
- Realm indexing + search-symbols + export-index all work correctly
- Wiki-link detection shows 2 (as expected for this doc type)

---

## Project Architecture Assessments

### Crate Structure: 2026-02-15

The six-crate workspace (core, parser, index, lsp, mcp, cli) is well-partitioned.
Arena allocation (bumpalo) lives in parser layer, not crossing into transport (lsp/mcp).
This was a good architectural decision — keeps Send/Sync constraints manageable.

**Watch:** markymark-index at 600+ lines, approaching the 500-line refactor threshold.
The arena conformance closeout (marky-luy) should monitor this.

---

## Lessons Learned

### 2026-02-15: Documentation for Agents != Documentation for Humans

**Key insight:** Agent docs need PROCEDURAL knowledge (how to work through problems)
alongside DECLARATIVE knowledge (what things are). Decision trees bridge this gap —
they answer "I need to choose" with "here's the answer." We need more of that energy
applied to: error diagnosis workflows, multi-concept patterns, and borrow checker
resolution strategies.

**Evidence:** The closures gap was invisible when reading the docs as a human (you know
what Fn traits are). But an agent hitting `expected FnMut, found FnOnce` for the first
time has no doc to reach for. The decision tree format ("How will you call the closure?")
directly maps to the agent's situation.

### 2026-02-15: Dogfooding Reveals Tool Gaps

**Key insight:** Running markymark diagnostics on our own documentation revealed a real
bug (XML parsing in code blocks) that wouldn't have been caught by user reports for a
long time — most markdown files don't have Rust generics in them. Eating our own dog
food is the fastest path to quality.

---

## Using markymark Effectively

### MCP Tools Reference

markymark exposes MCP tools for markdown intelligence. Use them instead of grep/read
when working with markdown files in this project or any workspace markymark indexes.

**Realm management (workspace isolation):**

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `create-realm` | Create isolated index namespace | Start of analysis — keep different workspaces separate |
| `add-root` | Index a directory's markdown files | Point at a docs/ folder to make it searchable |
| `remove-root` | Un-index a directory | Swap out one doc set for another |
| `destroy-realm` | Delete realm entirely | Cleanup when done |
| `realm-stats` | Counts: documents, headings, links, tags | Quick health check of a doc corpus |

**Workflow:** Create realm → add-root → do work → destroy-realm (or leave for reuse).

**Symbol intelligence:**

| Tool | Purpose | When to Use |
|------|---------|-------------|
| `search-symbols` | Fuzzy search across all indexed headings | "Where is lifetime discussed?" — faster than grep |
| `get-outline` | Heading tree for a single file (requires `file://` URI) | Understand a file's structure before reading it |
| `export-index` | Full index dump: headings, links, wiki-links, XML tags | Audit link health, find broken refs, check structure |
| `find-references` | All references to a heading/tag at a position | Impact analysis — who links to this heading? |

**Tips learned from dogfooding (2026-02-15):**

1. **Always use `file://` URIs** — `get-outline`, `export-index`, and `find-references`
   require full `file:///path/to/doc.md` URIs, not relative paths.

2. **`realm-stats` is cheap** — use it as a before/after check when modifying docs.
   Compare heading_count and markdown_link_count to verify you didn't break structure.

3. **`search-symbols` is fuzzy** — it matches against heading text, not file content.
   Use it for "where is concept X documented?" not "find this exact string."

4. **`export-index` reveals link health** — the `markdown_links` array shows every
   link target. Cross-reference against actual file paths to find broken links.

5. **XML tag detection has false positives in code blocks** (marky-8la) — any
   `<T>`, `<Mutex>`, `<dyn Trait>` in fenced code blocks will show as XML tags
   in export-index and trigger diagnostics. Ignore these until the bug is fixed.

6. **Wiki-link count is a quality signal** — for code docs, 0-2 wiki-links is normal.
   For knowledge bases, wiki-links should be the primary nav method. A low count in
   a knowledge base means it's not well-connected.

### LSP Tools (Preferred — Use First)

Claude Code has a built-in `LSP` tool that talks directly to markymark's LSP server.
**Prefer LSP over MCP for single-file operations** — it's more context-efficient
because you get exactly what you asked for at a specific location, no realm setup needed.

| LSP Operation | What It Does | Use Instead Of |
|---------------|-------------|----------------|
| `documentSymbol` | Heading outline for a file | MCP `get-outline` |
| `workspaceSymbol` | Search headings across all indexed files | MCP `search-symbols` |
| `goToDefinition` | Jump to wiki-link or heading link target | Manual link following |
| `findReferences` | All references to a heading or wiki-link | MCP `find-references` |
| `hover` | Info about a link or heading at cursor position | Reading the file |

**When to use LSP vs MCP:**

```text
What do you need?
├─ Single file outline or structure?
│   └─ LSP documentSymbol — no setup, instant
├─ Find where a concept is documented?
│   └─ LSP workspaceSymbol — searches indexed headings
├─ Jump to a link target?
│   └─ LSP goToDefinition — resolves wiki-links and md links
├─ Who references this heading?
│   └─ LSP findReferences — precise, position-based
├─ Aggregate stats across a doc corpus?
│   └─ MCP realm-stats — LSP doesn't aggregate
├─ Full link audit (all broken links)?
│   └─ MCP export-index — dumps everything for bulk analysis
└─ Index a new directory not in the workspace?
    └─ MCP create-realm + add-root — LSP only indexes configured roots
```

**LSP is cheaper because:**
- No realm creation/teardown overhead
- Returns exactly what you asked for (one outline, one definition)
- Already running if markymark is configured as the markdown LSP
- Position-based queries (line + character) are precise, not fuzzy

### Diagnostic Categories

markymark reports three categories of issues:

| Category | Examples | Severity |
|----------|---------|----------|
| Broken links | `[[MissingPage]]`, `[text](#bad-anchor)` | Error — will confuse readers |
| Duplicate headings | Two `## Details` in same file → same slug | Warning — anchor conflicts |
| XML tag issues | Unclosed `<tag>`, malformed attributes | Warning — may indicate formatting errors |

**Ignore XML tag warnings in files with code blocks** until marky-8la is fixed.
Focus on broken links and duplicate headings — those are real quality issues.

### Effective Dogfooding Workflow

When auditing a doc corpus with markymark:

```
1. create-realm "audit-name"
2. add-root with the docs directory
3. realm-stats → baseline counts
4. For each file of interest:
   a. get-outline → verify heading hierarchy
   b. export-index → check links, find broken refs
5. search-symbols for key concepts → verify discoverability
6. Make improvements
7. remove-root + add-root → re-index
8. realm-stats → compare with baseline
9. destroy-realm
```

---

## Session Notes

### 2026-02-16: PR #21 follow-up triage execution (marky-3l6)

- Implemented two accepted follow-ups from review triage:
  - Added CRLF incremental edit parity regression test in `markymark-lsp/tests/state_tests.rs`.
  - Added incremental edit clamp observability in `markymark-lsp/src/state.rs` with helper-level unit coverage.
- Verified with `cargo test -p markymark-lsp` and `cargo fmt --check`.
- Posted PR response summary comment: https://github.com/sethyanow/markymark/pull/21#issuecomment-3910812072
- Closed beads issue `marky-3l6` after code, tests, and PR response loop were complete.

### 2026-02-17: Extraction parity suite + corpus report (marky-0aw)

- Added `markymark-index/tests/extraction_parity.rs` (feature-gated on `zig-kernels`) with:
  - 11 handcrafted AST-vs-scan parity fixtures (including known-gap coverage for setext/frontmatter and code-heavy docs),
  - corpus scan over `docs/**/*.md`,
  - per-element FP/FN aggregation (headings, wiki links, markdown links, tags, block IDs),
  - report generation to `docs/benchmarks/extraction-parity.md`.
- Latest corpus run metrics:
  - parsed files: 90,
  - raw FP: 4.49%,
  - adjusted FP (excluding setext/frontmatter known gaps): 4.26%,
  - raw FN: 2.81%,
  - adjusted FN: 2.52%.
- Validation command that passed:
  - `cargo test -p markymark-index --features zig-kernels -- --test-threads=1 parity`

### 2026-02-17: SemanticIndex integration for BRZA epic (marky-bcv)

- Added `embeddings` feature flag to `markymark-index` (`embeddings = ["zig-kernels"]`) and gated semantic exports behind it.
- Implemented `markymark-index/src/semantic.rs`:
  - `SemanticIndex::new`, `add_document`, `remove_document`, `search`, `detect_duplicates`,
  - entry metadata (`SemanticEntry`) and result models (`SearchResult`, `DuplicateMatch`),
  - stale-ID filtering strategy to handle document removal/replacement even though current Zig embedding API has no delete primitive.
- Wired semantic support into `RealmIndex` under `embeddings`:
  - `RealmIndex::new_with_embeddings`,
  - automatic semantic indexing on markdown document add,
  - `semantic_search` and `detect_semantic_duplicates` query APIs.
- Added integration tests in `markymark-index/tests/semantic_index.rs` covering:
  - empty search behavior,
  - add + search relevance,
  - no-heading fallback,
  - provider failure propagation,
  - duplicate detection threshold,
  - realm-level semantic search wiring.
- Verification commands that passed:
  - `cargo test -p markymark-index`
  - `cargo test -p markymark-index --features embeddings --test semantic_index`
  - `cargo clippy -p markymark-index -- -D warnings`
  - `cargo clippy -p markymark-index --features embeddings -- -D warnings`

### 2026-02-17: MCP semantic-search tool + realm-stats enhancements (marky-9ui)

- Added semantic-search support across core and MCP layers:
  - `markymark-core/src/engine.rs`: new `CoreOperation::SemanticSearch`, `CoreOperationResult::SemanticMatches`, and `SemanticSearchMatch` payload.
  - `markymark-index/src/semantic.rs`: `SearchResult` now carries `heading_level`.
  - `markymark-mcp/src/dto.rs`: new `SemanticSearchRequest/Response` + result DTO; `RealmStatsRequest` now includes optional `check_duplicates`/`include_token_counts` flags; `RealmStatsResponse` now includes optional `duplicate_pairs`/`total_tokens`.
  - `markymark-mcp/src/lib.rs`: added `semantic-search` tool handler and wired expanded realm-stats request/response fields.
- Added runtime execution paths in `markymark-mcp/src/runtime_engine.rs`:
  - feature-aware semantic search operation handling with section preview generation (200-char max),
  - optional duplicate pair calculation via `RealmIndex::detect_semantic_duplicates`,
  - optional token estimation aggregation via `markymark_kernels::tokens::estimate_tokens`,
  - semantic-enabled realm initialization using a deterministic hash-based embedding provider when `semantic-search` feature is enabled.
- Added `semantic-search` Cargo feature in `markymark-mcp/Cargo.toml` (`semantic-search = ["markymark-index/embeddings"]`).
- Added/updated tests:
  - `markymark-mcp/tests/tool_handler_tests.rs`: semantic-search tool registration/response tests and realm-stats option assertions.
  - `markymark-mcp/tests/runtime_engine_tests.rs`: semantic-search runtime coverage and token-estimate realm-stats coverage.
  - `markymark-mcp/tests/runtime_tools.rs`: end-to-end semantic-search tool test with real runtime indexing.
- Verification commands that passed:
  - `cargo fmt --all --check`
  - `cargo test -p markymark-core`
  - `cargo test -p markymark-index --features embeddings --test semantic_index`
  - `cargo test -p markymark-mcp`
  - `cargo test -p markymark-mcp --features semantic-search`
  - `cargo clippy -p markymark-mcp -- -D warnings`
  - `cargo clippy -p markymark-mcp --features semantic-search -- -D warnings`

### 2026-02-17: BRZA Benchmark Reality Check (marky-e59)

Implemented `markymark-kernels/benches/brza_kernels.rs` and produced
`docs/benchmarks/brza-markymark-benchmarks.md` from Criterion artifacts.

**Measured outcomes:**
- Heading scan: 107x-255x faster than tree-sitter extraction (target met/exceeded)
- Bulk re-index (600 docs): ~40x faster with Zig scan backend (strong win)
- Embedding search: 1K/10K latency targets met, 100K misses target (~41ms vs <10ms)
- Link scan vs regex: SIMD path slower than regex baseline on all tested sets
- content_hash vs md5: FNV-1a path slower than md5 baseline on tested sizes

**Lesson:** BRZA gains are highly workload-dependent. SIMD extraction + scan-path indexing wins are clear, but hash/link kernels and large-scale embedding search need targeted optimization instead of assumed blanket speedups.
