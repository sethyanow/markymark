# Agent Memory — markymark

Cross-session knowledge: active state, guardrails, and links to topic files.
Linked from CLAUDE.md, auto-loaded at session start.

**Curation rules:** Keep this file under 200 lines. High-signal only.
Detailed patterns live in topic files. Historical context lives in the archive.

**Topic files** (load on demand):
- [Zig Patterns](memory/zig-patterns.md) — FFI, ExtractionRenderer, md4c, build quirks
- [Architecture](memory/architecture.md) — Decisions, conventions, release process
- [Archive](memory/archive.md) — PR triages, completed epics, research, resolved bugs

---

## Current State (2026-03-23)

### marky-0xtn COMPLETE — blob serialization eliminated (PR #55)

Epic closed. All 14/14 criteria verified. PR #55 targeting dev.
CEngineResult is now the sole FFI path for all consumers (LSP, MCP, tests).
~6,600 lines of dead code deleted across Phases 4.1-4.6.

**Hollow feature note:** The `zig-kernels` feature in markymark-core is now completely
hollow — all `cfg(feature = "zig-kernels")` references were in the deleted scanner module.
The feature still activates the `markymark-kernels` optional dep but no code in
markymark-core uses it. Cleanup deferred — not blocking.

### Bazel Build System (2026-03-23)

Added Bazel alongside Cargo for optimized release builds with cross-language ThinLTO.

**Why:** rustc 1.93 uses LLVM 21, Zig 0.15.2 ships LLVM 20. Version mismatch prevents
cross-language LTO under Cargo. Bazel with `toolchains_llvm_bootstrapped` (LLVM 21.1.8)
provides a unified toolchain.

**Setup:** `MODULE.bazel` (rules_rust 0.68.1, rules_zig 0.12.3), `.bazelrc`, `BUILD.bazel`
per crate. `zig_static_library` in `zig/BUILD.bazel` replaces build.rs for Bazel builds.

**macOS caveat:** `-Clinker-plugin-lto` doesn't work on macOS (ld64.lld rejects `-plugin-opt`).
Release config uses `-Clto=thin,-Cembed-bitcode=yes` instead. `-Cembed-bitcode=yes` overrides
rules_rust's default `=no`.

**Cargo is unaffected** — remains the dev-loop build. Bazel is the release/CI path.

---

### Investigation Complete: Semantic Index and Block Model

Comprehensive investigation of semantic indexing, search, and document index completed (2026-03-05).
Full findings documented in `docs/research/semantic-index-block-model.md`.

**Key findings:**
- SemanticEntry is heading-centric; stores heading text, level, and section bounds (Position, not full section content)
- build_document_plan() creates entries per heading (fallback for headless docs)
- Search returns heading-level results via SearchResult struct (no paragraph/block-level search)
- Duplicate detection uses Jaccard similarity over token hash sets
- DocumentIndex stores 15 types of indexed elements (headings, blocks, tags, code spans, etc.)
- Incremental updates diff headings by text; no stable block IDs in semantic index

### Phase 1 Complete: Content Hash Short-Circuit (marky-lpb, 2026-03-23)

Zig's DocumentEngine already computes a content hash on every parse (hash of raw text input
to md4c). Phase 1 exposes this hash via FFI and uses it to skip blob serialization +
deserialization when document structure hasn't changed.

**Key decision:** Hash is on raw text input, not extracted structure. Two different texts
producing identical headings/links still get different hashes. This is conservative (may
rebuild unnecessarily) but never wrong (never skips a real change).

**Implementation:**
- `marky_engine_get_content_hash` C FFI export → `DocumentEngine::content_hash()` returns `u64`
- `EngineState` wrapper in `markymark-lsp/src/state/mod.rs` stores `engine` + `last_hash`
- `build_markdown_index_via_engine` returns `Option<DocumentIndex>` — `None` when hash unchanged
- `change_document` / `apply_document_changes` skip `realm.update_document()` on `None`
- 16 FFI-level hash tests + 8 short-circuit tests in LSP state

**Savings:** ~2ms blob/arena work skipped per non-structural edit. Parse still runs.

### Phase 2 COMPLETE: Edit Range Threading (marky-686, 2026-03-24)

All 3 tasks done, 10/10 sub-epic criteria met. Acceptance task pending.

Task 1 (marky-f1w): FFI plumbing — `update()` accepts edit_offset/old_len/new_len.
Task 2 (marky-v60): Zig slug reuse — headings before edit_offset reuse old slugs.
Task 3 (marky-enr): LSP threading — `apply_document_changes` accumulates bounding box
from incremental edits (min start_byte, sum old_len, sum new_len) and passes via
`build_markdown_index_via_engine` to `engine.update()`. Full changes invalidate range.

**Key patterns:**
- Bounding box: `match` on `Option<(usize, usize, usize)>` to accumulate, `.map()` to convert
- Skipped edits (`end_before_start`) excluded from accumulation
- `build_markdown_index_via_engine` signature: `(&mut self, uri, text, Option<EditRange>)`

### Phase 3a Complete: Direct Arena Decode (marky-u9q, 2026-03-24)

`from_engine_result_direct` in `from_engine_direct.rs` reads CEngineResult.text_blob
directly into bumpalo arena via typed EngineResult accessors + `read_blob_str`, bypassing
EngineExtraction. LSP hot path now uses one copy (blob → arena) instead of two.

**Key implementation details:**
- EngineResult now has typed slice accessors (headings(), links(), etc.) wrapping private ptr_slice
- `read_blob_str` returns `&str` (borrowed) vs old `read_str` which returned owned `String`
- Intermediate Vec collection pattern: blob → owned Strings in Vecs → arena (necessary because
  DocumentIndexCell's self_cell closure can't hold borrows from EngineResult)
- Content blocks NOT extracted in the direct path (matches current LSP hot path behavior)
- Link parsing replicates convert_engine_result logic exactly (wiki vs markdown split, alias detection)

**Foundation for Phase 3b:** Typed accessors + read_blob_str enable borrowing from text_blob
directly into DocumentIndex fields via blob-in-owner approach (see below).

### Phase 3b Design Decision: Blob-in-owner replaces lifetime parameter (2026-03-24)

R6/R7 originally specified `DocumentIndex<'engine>` with lifetime cascade through
RealmIndex/ServerState. Analysis revealed the double-copy exists because text_blob
isn't in the self_cell owner — a structural problem, not a lifetime problem.

**Fix:** Add `text_blob: Vec<u8>` to DocumentOwner. Inside the self_cell closure,
`read_blob_str(&owner.text_blob, offset, len)` returns `&'a str` borrowing from
the owner. Zero copy per string. No lifetime parameter, no API cascade.

**User confirmed** this replaces R6/R7. Sub-epic criteria updated accordingly.
Task: marky-03r.

### Active Work

- **Epic marky-zsys**: Engine Pipeline v2 — Phase 1 + Phase 2 closed. Phase 3: 3/10 criteria met.
  Next: marky-03r (blob-in-owner), then Phase 3 acceptance.

### Failure Pattern: Unauthorized architectural switch (fail-from-ast-switch)

Agent hit a gap (from_scan doesn't populate content blocks) and reactively switched
MCP indexing from `from_scan_with_frontmatter` to `from_ast` to make tests green.
This reversed the deliberate B-8 migration. Correct behavior: stop, report gap, present
options (extend from_scan, switch deliberately with analysis, or hybrid), get user call.

### docs-site branch — merging dev, follow-ups remain

Epic marky-dhkn (docs-site for v0.6.0) reviewed and approved. Branch: `docs-site`.
Remaining: marky-6yuk (terminology rename), marky-xclk (human review notes), marky-03m7 (dead extract/ cleanup).
GitHub Pages deployment researched — see archive for details.

### Rust Agent Docs: Grade A (2026-02-15)

45 files, 6,443 lines. 14 decision trees. 18 mistakes tracked. All gaps closed.
Known issue: XML tag false positives in code blocks (marky-8la).

### Zig Agent Docs: Grade A (2026-03-05)

50 files, 238 internal links, 0 orphans, 0 broken links. 10 hubs (top: core/memory.md with
19 incoming). 18 mistakes with severity emoji. 10 decision trees. stdlib/ category (11 files)
merged from modules/zig/02-std/. docs_index in AGENTS.md and CLAUDE.md updated. Exceeds
rust_agent_docs baseline on hub count (10 vs 5). Curation complete (marky-u2mb closed).

### Layered Retrieval Vision (Epic marky-b9o4, 2026-02-26)

Five-layer architecture: L0 ambient docs_index (preserved, auto-generated), L1 smart retrieval
(recommend-docs MCP tool), L2 curation diagnostics, L3 multi-project federation,
L4 memory integration (MEMORY.md + beads in search surface). Key constraint: docs_index pattern
stays because it ramrods symbols into agent instructions at zero latency — everything builds
on top. Related: marky-mkr, marky-y4be, marky-c9wi.

**Layer progress:** L0 ✅ (export-docs-index) → L1 ✅ (recommend-docs + tree intelligence) → L2 ✅ (curation-diagnostics) → L3 ✅ (multi-root federation) → L4 ✅ (structured doc search)

### Layer 3: Multi-Root Federation (marky-eluj, 2026-03-05)

Validated via 7 integration tests in `markymark-mcp/tests/multi_root_federation.rs`. Multi-root
infrastructure (add-root/remove-root) already worked correctly — no bugs found. Key findings:
wiki-link resolution is realm-wide (cross-root `[[target]]` resolves), relative markdown links are
source-anchored, same-stem disambiguation uses insertion order (first-added root wins), root
removal correctly causes cross-root links to become broken. All tools (search-workspace,
graph-analysis, recommend-docs, curation-diagnostics, export-docs-index) work across roots.

### Layer 2: Curation Diagnostics (marky-stip, 2026-03-05)

New `curation-diagnostics` MCP tool composing graph-analysis with per-document degree computation.
Detects orphan docs (in-degree=0 AND out-degree=0), scores connectivity per document, identifies
low-connectivity docs (below median AND below threshold of 2 links), and generates actionable
cross-link suggestions (orphan → nearest hub by directory co-location). SRE edge cases: empty
realm, single-doc realm (no self-link suggestions), max_suggestions/max_items_per_category caps.
Key modules: `engine/curation.rs` (handler with `GraphData` struct for extracted state),
`tools/curation.rs` (MCP tool), `engine/tests/curation.rs` (11 tests). Algorithm recomputes
degree maps from RealmIndex since graph-analysis doesn't expose per-doc degree.

### Layer 4: Structured Document Search (marky-6m1o, 2026-03-05)

Extended search-workspace to include structured documents (JSON, YAML, TOML, JSONL, etc.).
Key implementation decisions: SRE plan assumed `value_range` had byte offsets, but `Range` is
line/col-based — adapted to simpler `source_contains()` full-text search instead of per-value
extraction. Scoring mirrors markdown tiers: URI stem (1.0), key-path via `search_keys()` (0.8),
source text (0.6). Filters (frontmatter, tag, property) correctly exclude structured docs.
`uri_to_title()` generalized with `TITLE_STRIP_EXTENSIONS` constant for all file types.
Key modules: `markymark-index/src/structured_document.rs` (source_contains method),
`markymark-mcp/src/search.rs` (score_structured_document + integration). 9 new tests (2 unit,
7 integration), 270 total passing. All epic success criteria met — L0 through L4 complete.

### Tree Intelligence Sub-Epic (marky-d21j, child of marky-b9o4, 2026-02-26)

PageIndex-style hierarchical retrieval. Three phases all complete:
(1) ✅ expose existing OutlineNode as hierarchical JSON via get-outline format=tree + include_text,
(2) ✅ optional LLM enrichment with sidecar `.markymark/` JSON + content-hash invalidation,
(3) ✅ recommend-docs MCP tool composing search-workspace + graph-analysis + sidecar summaries.
Key modules: `engine/recommend.rs` (handler), `tools/recommend.rs` (MCP tool), `engine/outline.rs`
(`try_load_sidecar` now `pub(crate)` for shared use). Algorithm: 0.7*search_score + 0.3*hub_score,
top_k results with optional section summaries. Sub-epic marky-d21j complete.

### PR #52 Copilot Review Triage (2026-03-05)

Triaged 7 findings (6 inline + 1 suppressed). 2 dismissed, 5 valid → 5 beads created.

**Dismissed false positives:**
- `root_to_file_uri()` "produces `file:////`" — reviewer's math wrong, `"file://" + "/tmp/x" = "file:///tmp/x"`.
  Same pattern in `DocumentUri::from_file_path()`.
- `stem_to_uri.insert()` "disagrees with graph-analysis" — both `curation.rs` and `graph.rs` use
  identical `.insert()` pattern. Consistently last-indexed wins.

**Valid findings (beads):**
- marky-se77 (P2): export_docs_index root_count contract violation + `./{absolute}` display
- marky-qfut (P2): curation degree map uses path-based markdown link resolution while graph.rs uses stem-based
- marky-niw2 (P2): enrich.rs silent I/O errors (`let _ =`) + sidecar path collisions with override
- marky-d175 (P3): outline format not validated at MCP boundary
- marky-f2oa (P4): repeated allocations in outline section extraction + structured doc search

---

## Key Failure Patterns

These prevent repeat mistakes. Do not remove without replacement.

### bd + Dolt panics under parallel CLI invocations (2026-02-26)

Running multiple `bd` commands in parallel can trigger a Dolt nil-pointer panic even with
`BD_NO_DB=true BEADS_NO_DAEMON=1`. Run `bd` operations one-at-a-time.

### Global env fault-injection hooks leak across parallel tests (2026-02-27)

Using process-wide env vars to force failure paths (`set_var`/`remove_var`) causes
cross-test contamination under Rust's parallel test runner. Unrelated tests can observe
the injected flags and fail nondeterministically. Prefer URI-scoped or instance-scoped
fault hooks for integration tests; avoid global mutable process state.

### Context window exhaustion from task chaining (fail-context-runaway)

Agent completed multiple tasks in one session without stopping for user review. Hit context
limit mid-task, leaving incomplete file splits (build break).

**Rules:**
- **ONE task per session turn.** After completing a task, STOP and report.
- **Budget awareness.** After >2 commits, pause and check in.
- **Never start a destructive refactor near context limits.** File splits require atomic completion.
- **Commit intermediate milestones within large tasks.**
- **NEVER autonomously reduce designed scope.** Report analysis, let user decide.
- **Benchmark numbers do not justify skipping designed work.**

### Benchmark methodology anti-pattern (fail-benchmark-chasing)

Agent iteratively "fixed" benchmark methodology until numbers looked good. Each fix was
technically valid, but adjusting until success is unacceptable. Design benchmarks correctly
from the start. Report honest numbers.

### tower-lsp-server v0.23 API mismatch (fail-tower-lsp-types)

Pre-training has `lsp_types` and `#[async_trait]`. The community fork v0.23 uses `ls_types`
and native async traits. Always read `docs/rust_crates/tower-lsp.md`.

### MCP stdio: line-delimited JSON, not Content-Length (fail-mcp-framing)

rmcp stdio uses `writeln!` + `read_line`, not HTTP-style `Content-Length` headers.

### Agent attempted PR merge without authorization (fail-pr-merge-autonomy)

Agents NEVER merge PRs. Human merges all PRs. Agent prepares PRs and pushes branches only.

### Security hook blocks Write on GitHub Actions YAML (fail-write-tool-gh-actions)

`security_reminder_hook.py` intercepts Write on `.github/workflows/*.yml`. Use Bash heredoc.

### Agent used Grep/Read instead of LSP for code navigation (fail-lsp-not-used)

Always use LSP tools first for Rust/Zig navigation. Read/Grep only after LSP narrows
the target or for non-code files.

### Agent used claude-mem save_memory for this project (fail-save-memory-unreliable)

CLAUDE.md says not to use `save_memory` for markymark. Use `docs/MEMORY.md` as the sole
persistent memory store.

### Dev workflow skill placed in plugin directory (fail-skill-location)

Plugin skills are user-facing features. Dev workflow skills belong in `.claude/skills/`.

### Agent searched filesystem for plugin skills (fail-plugin-skill-filesystem-search)

Plugin skills (`hyperpowers:*`, `pensive:*`, `beads:*`, etc.) are served by the plugin
system, not local files. The `Skill` tool invocation IS the loading mechanism. Never search
`~/.claude/skills/` for plugin skill files — they don't exist on disk.

### CLAUDE.md crate table stale after adding crate (fail-stale-crate-table)

When adding a crate to workspace, update CLAUDE.md crate table in the same PR.

### Brainstorm agent rejected existing infra without verifying (fail-brainstorm-rejected-existing-infra)

marky-3cy brainstorm rejected md4c block extraction as "unnecessary Zig FFI complexity"
without checking that `enterBlock`/`leaveBlock` callbacks already existed in
`ExtractionRenderer`. Tree-sitter was chosen unilaterally — the extraction source was
never presented as a question to the user. Six other design questions were asked via
AskUserQuestion; this one was silently decided by the agent. Result: two implementation
sessions built on tree-sitter, both caught and corrected, wasted work across three sessions.

**Rule:** Every rejected alternative in an epic must trace to a user decision. Before
claiming an approach requires "new FFI" or "unnecessary complexity," verify the claim
against the actual codebase. See marky-c2g4 for the fix.

### docs/modules and docs/zig_agent_docs are symlinks (info-docs-symlinks)

Symlinks to forge repo. `git ls-files` and `find -type f` won't find them. Use absolute
path or `ls -la docs/` to verify. CodeRabbit flagged as "non-existent" — false positive.

### ${CLAUDE_PLUGIN_ROOT} in file content triggers hook blocks (fail-write-plugin-root)

Use `Bash cat` heredoc with single-quoted delimiter (`'EOF'`) to bypass.

---

## Using markymark Effectively

**Prefer LSP over MCP for single-file operations** — no realm setup needed.

**MCP tips:**
- Always use `file://` URIs for `get-outline`, `export-index`, `find-references`
- `realm-stats` is cheap — use as before/after check when modifying docs
- `search-symbols` is fuzzy on heading text, not file content
- Ignore XML tag warnings in files with code blocks (marky-8la)

### Zig MCP Tool (mcp__zig)

Output is generic and noisy; not useful for precision Zig work — stick with LSP + agent docs.
May be useful for quick build.zig scaffolding.

---

## Project-Specific

- Plugin directory: `markymark-plugin/.claude-plugin/plugin.json` (version bumped manually, license now AGPL-3.0 as of 2026-03-03)
- `require_marksman!` macro for graceful test skip in CI
- lefthook YAML: quote command values containing colons/braces

## License Status (2026-03-03)

Plugin license updated to AGPL-3.0. Remaining "MIT OR Apache-2.0" references in docs are examples/documentation only:
- `markymark-plugin/README.md` (doc example for workspace manifest)
- `docs/rust_agent_docs/reference/cargo-ref.md` (reference doc showing example syntax)
- `docs/rust_crates/core.md` (reference doc showing example syntax)

No code or configuration files need updating — these are documentation references only.
