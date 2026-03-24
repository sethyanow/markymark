# Archive — markymark

Historical context: completed epics, PR triages, resolved bugs, research.
Linked from [MEMORY.md](../MEMORY.md). Load on demand when investigating past work.

---

## PR Triage History

### PR #46 (feature-embeddings) — Review Triage Round 2 (2026-02-26)

15 findings from 4 reviewers (Codex, Copilot, CodeRabbit, Greptile). **10 dismissed** (already
fixed by `ac3563b`), **1 already tracked** (marky-y4be), **4 new valid**:

| Bead | P | Finding |
|------|---|---------|
| marky-ysv8 | P2 | **FIXED** (`f06d591`) — Realm read-lock held across semantic search await |
| marky-2q2b | P2 | **FIXED** (`db61d5c`) — Voyage embed_batch response cardinality validation |
| marky-h7pp | P4 | `/dev/null` test not portable — needs `#[cfg(unix)]` (local.rs:221) |
| marky-le49 | P4 | Stale `voyage-3` in README.md, code default is `voyage-4` |

**Pattern:** Reviewers analyzed commit `b77c490` (pre-fix). 10/15 findings were already
addressed. Future triage rounds should note the reviewed commit vs HEAD to fast-dismiss stale findings.

### PR #46 Round 3 — Final-Validation Blocker (2026-02-26, marky-e5zl)

`marky-qfg1` could not be closed after child-task completion because final validation failed on
semantic-search builds:

- Repro: `cargo nextest run -p markymark-index -p markymark-mcp --features semantic-search`
- Failure: E0364 in `markymark-index/src/semantic/mod.rs`
- Cause: `mod.rs` re-exports helper functions that are private in `helpers.rs`

**Follow-up task:** `marky-e5zl` (P1) added under `marky-qfg1`, with SRE-refined design requiring
internal visibility cleanup plus nextest/clippy/semantic test validation.

### PR #48 v0.7.0 Release — Review Triage Round 5 (2026-02-28)

88 changed files. Cross-PR triage: PR #48 + PR #47 (overlapping code) + Codex note.
Reviewers: Semgrep (4), Copilot (8+4), CodeRabbit (6+6), Greptile (1), Codex (1).
20 raw → 9 tracked (table below), 8 dismissed, 3 batched into marky-pk7p (Round 6).

| Bead | P | Finding |
|------|---|---------|
| marky-a2m7 | P1 | Semantic writes before root_still_present check in AddRoot (+ batch optimization) |
| marky-qgg1 | P2 | Mutex serializes semantic search across embed I/O (realm + engine paths) |
| marky-6ri3 | P2 | Non-atomic add_document — embed failure loses old entries |
| marky-ce9o | P3 | ID collision in update_document during heading reorder + slug match |
| marky-6pap | P3 | All-blank headings skip fallback → zero semantic entries |
| marky-qmpo | P3 | u64→u32 truncation in compute_fetch_k under-fetches |
| marky-6igw | P4 | Stale `voyage` feature flag references in CLI doc + smoke script |
| marky-lkw9 | P4 | reqwest 0.12 → 0.13 dependency update |
| marky-u1ji | P4 | Test default_cache_dir_under_home assumes HOME env |

**Dismissed:** (8 findings)
- Semgrep temp_dir (main.rs:207,224) — test code only, no security risk
- Semgrep await-holding write lock in RemoveRoot — required for consistency
- Copilot scripts/README.md table `||` — false positive, tables are correct
- Copilot ~23MB model size — approximately correct for all-MiniLM-L6-v2
- Copilot smoke-embeddings.sh executable bit — scripts have shebangs, standard practice
- CodeRabbit references_tests.rs write lock — test code, no re-entrancy risk
- CodeRabbit realm_isolation.rs unused mut — already fixed (91bf466)
- Copilot lib.rs read_resource naming — trait-imposed name, style preference only

**Patterns learned:**
- Semgrep temp_dir rule fires on ALL temp_dir usage regardless of context — dismiss in test code
- CodeRabbit re-flagged the mut issue that was already fixed — verify HEAD before triaging
- Codex produced highest-signal finding (P1 race condition) that automated tools missed

### PR #46 Round 4 (2026-02-27)

3 findings from Codex + CodeRabbit pre-PR notes. **1 dismissed**, **2 valid**:

| Bead | P | Finding |
|------|---|---------|
| marky-mgfh | P1 | AddRoot Phase 4 race: inserts docs after concurrent RemoveRoot removed the root |
| marky-nhi0 | P4 | Stale `feature-embeddings` branch ref in scripts/README.md:70 |
| — | — | **DISMISSED**: Zig index partial add — already tracked/risk-accepted in marky-y2ne |

**Pattern:** CodeRabbit re-flagged the Zig partial add issue that was already analyzed and
risk-accepted in marky-y2ne. Cross-reference closed beads before creating new ones.

### PR #44 (v0.6.0) — Review Triage (2026-02-23)

100 changed files, +16.9k/-7.7k. Reviewers: Copilot, CodeRabbit, GHAS/Semgrep, Greptile.

**Dismissed:** (5 tracks)
- Semgrep unsafe-usage/unsafe-block (30 comments) — FFI module, already annotated
- Copilot: circular dependency claim (markymark-index↔kernels) — FALSE, dev-dep only
- CodeRabbit: scan_tests.rs:98 xml_tags empty assertion — CORRECT test, inline vs block HTML
- CodeRabbit: md4c/mod.rs u32 truncation — already dismissed in PR #40/#41
- CodeRabbit: from_blob XML tag attributes empty — accepted trade-off

**Valid findings (8 beads):**
- marky-whvn (P1): CI linker failure — Zig archive format on Linux
- marky-ab5g (P2): realm/tests.rs at 994 lines
- marky-e7i3 (P2): frontmatter.rs property scan + CRLF
- marky-mh1p (P2): LSP fallback scan drops frontmatter
- marky-a4k9 (P3): Loose `>= 2` test assertions
- marky-r5p3 (P3): from_blob magic numbers + empty list items
- marky-85ii (P4): Docs cleanup batch
- marky-2pyo (P4): Code quality batch

**Patterns:** Copilot misidentifies dev-deps as circular. CodeRabbit doesn't understand
md4c inline vs block-level HTML distinction.

### PR #42 (v0.5.1) — Review Triage (2026-02-20)

7 findings — 3 valid (all fixed immediately), 4 dismissed.

**Dismissed:**
- docs/semver.md style findings — verbatim SemVer 2.0.0 spec
- Step numbering duplicate — CodeRabbit flagged same as Copilot
- cargo-mcp vs raw cargo in quality gates — intentional for release automation

**Fixed — marky-lj58 (P2, CLOSED):** Three correctness gaps in prepare-release Phase 2.

**Pattern (info-verbatim-spec-docs):** `docs/semver.md` is official SemVer spec verbatim.
Style findings on this file are always false positives.

### PR #41 — Review Triage (2026-02-20)

14 inline + 1 outside-diff + 13 nitpicks from CodeRabbit, 22 from Semgrep/GHAS, 0 from Copilot.

**Dismissed:** (3 items)
- did_open generation ordering — INVALID, design is correct
- u32 truncation in md4c.rs — already dismissed in PR #40
- Semgrep nosemgrep alignment — platform limitation

**Accepted — Round 1 (5 beads, ALL CLOSED):**
- marky-0rl6 (P1): ExtractionRenderer cursor split
- marky-c44x (P2): Debounce flush flattened
- marky-pk33 (P3): FFI safety — intCast guard + fallible read/write
- marky-i873 (P4): autolinks.zig improvements
- marky-4atp (P4): Code quality batch

**Accepted — Round 2/3 (3 beads):**
- marky-lzd5 (P2): ExtractionRenderer offset scan hardening (4 sub-issues)
- marky-nwoz (P3): LSP state/mod.rs robustness (2 sub-issues)
- marky-wdnc (P4): Zig engine doc/guard nitpick bundle

### PR #40 — Review Triage (2026-02-20)

8 findings — 4 valid, 1 already known, 2 dismissed.

**Dismissed:**
- Fixed buffer caps (tags 1024, block-ids 1024, fences 256) — intentional performance tradeoff
- u32 truncation in extract_md4c and call_scan_ffi — theoretical only, not UB
- Debounce edit loss — INVALID finding, design is correct

**Fixed:**
- marky-5vnt (P3, CLOSED): Slug truncation + processLeafBlock silent catch
- marky-9m7o (P4, CLOSED): parseAll errdefer leaks text on late-stage OOM

**Post-merge findings (cursor + codex):**
- marky-d7hh (P1): from_blob wiki link alias parity
- marky-8nzt (P2): parseAll toOwnedSlice cascade leak
- marky-ta07 (P2): convert_result blob slice without bounds checks

---

## Completed Epics

### Cross-Language Symbol Bridging (marky-ix3) — COMPLETE

All 11 markdown-content extractors migrated from extract.rs regex to Zig ExtractionRenderer.
Only frontmatter stays in Rust. Blob v2 header (128 bytes, 8 new count fields). MCP batch
path uses from_blob instead of from_ast. Code spans surfaced via LSP and MCP.

Key decisions:
- **Separate cursors per extraction type** — per marky-0rl6
- **from_blob backward-compatible** — reads both v1 and v2 blobs
- **FFI path is md4c/exports.zig** — NOT engine/exports.zig
- **Each B-task pattern:** Zig extraction → blob struct + header count → from_blob →
  DocumentDependent field → tests → remove extract.rs regex

### Documentation Overhaul (marky-y1gm)

- **Separate Starlight (Astro) docs site** in `docs-site/` — not in existing `docs/`
- **README rewritten as concise landing page** (~80 lines) linking to docs site
- **Bun exclusively** for docs tooling
- **23 content pages** across 8 sections
- **Agent tutorial** (`guides/agents.md`) — key differentiator
- **About page must be layperson-friendly** — no LSP/MCP jargon upfront
- First task: **marky-wvqy** — scaffold Starlight site

### Option H: Zig Document Engine (marky-io3h) — COMPLETE

Stateful Zig engine replacing N+4 FFI calls with exactly 2 (update + get_blob).
Tagged `marky-io3h-complete`. Net -2,839 lines.

Key decisions:
- Stateful (not stateless) — enables slug caching, lazy blob, future incremental
- Full md4c reparse always — fast enough with debounce
- from_ast()/from_scan() retained for MCP batch and backward compat

### RealmIndex v2 (marky-n7wx) — COMPLETE

lasso Rodeo interner in RealmIndex. Cross-doc HashMaps keyed by Spur (u32).
`update_document()` diffs `DocContribution` — fast path skips all cross-doc ops when
structure unchanged. Lazy `tag_to_docs` via `tags_dirty` flag.

Key decisions:
- Rodeo not ThreadedRodeo — single-threaded, simpler API
- Don't intern URIs — unique per document, no dedup benefit
- key_path_to_docs stays String — structured doc paths have low repetition

---

## Performance History

### Baseline Benchmarks (2026-02-19, marky-jpot)

| Size | md4c extract | md4c from_scan | tree-sitter from_ast | Pipeline speedup |
|------|-------------|----------------|---------------------|-----------------|
| 1KB | 0.115ms | 0.229ms | 0.490ms | 2.1x |
| 10KB | 0.850ms | 1.836ms | 4.573ms | 2.5x |
| 50KB | 4.686ms | 9.436ms | 26.662ms | 2.8x |
| 100KB | 9.882ms | 20.692ms | 66.962ms | 3.2x |

### Performance Optimization Roadmap

**Completed:**
- F: Debounce (marky-7dq) — 75ms async cancellation in LSP `did_change`
- G: md4c streaming parser (marky-0mr) — Vendored Bun's Zig md4c port. 2.8x pipeline speedup at 50KB.
- H: Zig Document Engine (marky-io3h) — tagged `marky-io3h-complete`
- D: Vendor tree-sitter-md (marky-0jz) — superseded by Option G

**Deferred:**
- E: Lazy AST (marky-syx, P3) — value reduced after Epic H
- Engine incremental diffing — 2.5ms at 50KB is fine. See incremental md4c research below.
- Zero-copy blob borrowing — not worth it, breaks DocumentIndex lifetime model

---

## Incremental md4c Block-Level Reparse (Research, 2026-02-23)

Full research documented in `/docs/research/incremental-parsing-sota-2026.md`.

### Key Findings

1. **No production markdown parser implements incremental reparse** — md4c, cmark, pulldown-cmark
   all parse full document. Lezer is the only production incremental markdown parser.
2. **Block-level incremental is feasible via safe boundaries** — Blank lines, ATX headings,
   thematic breaks, code fences are guaranteed convergence points.
3. **SIMD boundaries + sqrt decomposition chunks = sweet spot for markymark**

### Proposed Hybrid: SIMD Boundaries + Chunk Tree

**Layer 1** — SIMD structural boundary scan (existing kernels, microseconds)
**Layer 2** — Chunk tree with cached state (sqrt decomposition)
**Layer 3** — Edit propagation (O(log N) convergence)

Expected 3-5x speedup on typical edits, 10x+ on structural edits.
See research doc for full architecture analysis of md4c's blocks.zig state machine.

---

## Resolved Bugs

### Zig 0.15.2 Archive Format Incompatibility (RESOLVED, 2026-02-24)

`zig build lib` on Linux x86_64 produces archives that pass `ar t` but fail rust-lld.
Root cause: Zig's archive writer produces non-standard format where member offset metadata
is inconsistent with actual file size.

**Fix (`f2a894f`):** build.rs extracts .o files and re-packs with `ar rcs`. Only on Linux.

Key learnings:
1. Env vars (`ZIG_LOCAL_CACHE_DIR`) are ignored by `zig build`
2. `mlugg/setup-zig` restores `.zig-cache` at repo root
3. `ar t` validation in build.rs was the diagnostic breakthrough
4. The real bug is Zig's archive FORMAT, not caching

---

## Lessons Learned (Historical)

### Post-Implementation Code Review Findings (2026-02-26)

After parallel subagent implementation of marky-ysv8 + marky-2q2b:

- **Racy test synchronization**: `sleep(Nms)` to "wait for async task" is a race. Use
  `tokio::sync::Notify`. Fixed in `90734e2`.
- **Symmetrical test coverage**: When validating `!=` checks, test BOTH directions.
- **Inner Mutex contention after lock-scope fix**: Acceptable by design but must be documented.
- **`pub` vs `pub(crate)` for cross-crate internal APIs**: Methods consumed by sibling workspace
  crates must stay `pub`. Consider `#[doc(hidden)]` for stability signaling.

### Semantic add_document Atomicity (marky-y2ne)

Two-phase flow: (1) embed all headings/fallback and stage in memory, (2) commit all Zig
`index.add()` writes. Never interleave embed+insert. Unit tests should assert both metadata
and Zig state on failure.

### Semantic Startup Batching (marky-y4be, 2026-02-27)

`SemanticIndex::add_document` and `add_documents` stage entries, call `embed_batch` once,
then commit. On `embed_batch` error, fall back to sequential `embed` per text.
`RealmIndex::add_documents` batches semantic embedding first, then structural indexing.

### PR #44 — Codex Pre-Triage Findings

- marky-vxgg (P2): select-binary.sh missing .exe handling for Windows
- marky-e3if (P3): binary.ts PATH fallback — fixed in #34223
