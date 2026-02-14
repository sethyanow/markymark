# Handoff: Bumpalo Arena Migration Evaluation

**Branch:** `feature/feature-001`  
**Worktree:** `feature-mark-bumpalo`  
**Epics:** marky-g9t (arena migration), marky-luy (g9t remediation closeout)  
**Date:** 2026-02-15

---

## Session Summary

Evaluated whether the bumpalo arena migration was worthwhile. Added memory (RSS, resident) and concurrency benchmarks. Ran before/after comparison against pre-arena baseline (476795e). **Verdict: marginal gains; sample too small to conclude.**

---

## Current Benchmark Setup

**Location:** `markymark-index/benches/memory.rs`

**Metrics:**
- `index_10_documents` / `index_100_documents` — sequential indexing time
- `reparse_single_document` — parse + index one doc
- `memory/memory_after_index_100` — resident MiB + peak RSS (KB) after indexing
- `memory/alloc_count_index_100` — heap allocation count (via CountingAllocator)
- `concurrent_index_4x100_docs` / `concurrent_index_8x100_docs` — N threads each indexing 100 docs
- **Real corpus:** `reparse_real_large_doc` (epstein ~492KB), `index_real_corpus` (343 sections), `index_docs_dir` (gigapowers .md files, incl. .rust_docs/.zig_docs)

**Dependencies (dev):** criterion, memory-stats, libc

**Run:** `cargo bench -p markymark-index`

---

## Sample Quality Issues (CRITICAL)

### Current Sample

```rust
fn sample_doc(n: usize) -> String {
    format!(
        r#"# Document {}

## Section A
Content with [[wiki link]] and #tag and [markdown](https://example.com) link.

A block ^block-{}

## Section B
More content here.
"#,
        n, n
    )
}
```

**Problems:**
1. **Too small** — ~100 docs max; arena benefits may only show at 1k–10k+ docs
2. **Synthetic** — trivial structure; real markdown has nesting, long content, varied frontmatter
3. **Uniform** — every doc identical structure; no size/complexity variance
4. **No stress** — few headings, one wiki link, one tag, one block; parser/extraction barely exercised
5. **Memory measurement unreliable** — baseline showed 24 MiB; current run showed 1671 MiB (likely criterion/process bloat); need isolated measurement

### Recommended Sample Improvements

1. **Larger N:** Add benchmarks for 1_000, 5_000, 10_000 documents
2. **Real corpus:** Use actual markdown files (e.g. docs/**, .md from repo or external) instead of `sample_doc`
3. **Varied sizes:** Mix short (~100 chars), medium (~2KB), long (~50KB) documents
4. **Rich structure:** Frontmatter, properties, nested lists, XML tags, multiple wiki links, blocks
5. **Memory isolation:** Run benchmark binary in subprocess, measure RSS before/after single index run to avoid criterion accumulation

---

## Baseline (Pre-Arena) Comparison

**Branch:** `baseline/pre-arena`  
**Commit:** `476795e` — chore: checkpoint — marky-cfj complete, v0.1.0-alpha.2 released (#5)  
**Parent of:** `47aada5` (arena Phase 1)

### Running Baseline

```bash
git checkout pre-arena              # tag at 476795e; or baseline/pre-arena (branch with benches)
cargo bench -p markymark-index -- --nocapture
```

See `docs/benchmarks/baseline-pre-arena.md` for full results and reproducibility.

### Results (100 docs, synthetic sample) — 2026-02-14

| Metric              | BEFORE (476795e) | NOW (arena) | Delta   |
|---------------------|------------------|-------------|---------|
| Heap allocations    | 216,806          | 215,837     | −969    |
| index_100            | ~33.8 ms         | ~33.3 ms    | ~same   |
| concurrent 4×100    | ~59 ms           | ~59 ms      | ~same   |
| concurrent 8×100    | ~104 ms          | ~101 ms     | −3%     |
| Resident / peak RSS | 23 MiB / 24 MB   | 638 MiB (criterion bloat) | —       |

### Real Corpus (epstein 480 KB, gigapowers 918 files / 5.9 MB)

| Benchmark              | Pre-arena | Arena   | Delta  |
|------------------------|-----------|---------|--------|
| reparse_real_large_doc | 166.7 ms  | 163.6 ms | −2%   |
| index_real_corpus (343 sections) | 251.4 ms | 247.8 ms | −1.5% |
| index_docs_dir         | 1.60 s    | 1.64 s   | +2.5% (noise) |

Use `MARKYMARK_BENCH_EPSTEIN` for epstein path; `MARKYMARK_BENCH_CORPUS_DIR` or default `/Volumes/code/gigapowers`.

---

## Key Decisions & Constraints

- **XmlTagEntry.attributes:** Cannot use ArenaHashMap in index layer — `&Bump` is !Sync; DocumentIndex lives in LSP ServerState which must be Sync. Parser uses ArenaHashMap; index uses default allocator for that field.
- **ArenaHashMap::clone:** Triggers SIGSEGV (dec-020); avoid cloning types containing it. LSP `SymbolAtPosition::XmlTag` holds owned XmlTagEntry (requires Clone).
- **RealmIndex:** Hybrid model — per-doc arena, owned cross-doc lookups (ResolvedHeading/ResolvedBlock) per dec-arena-001.

---

## Relevant Files

| File | Role |
|------|------|
| `markymark-index/benches/memory.rs` | All benchmarks |
| `markymark-index/Cargo.toml` | criterion, memory-stats, libc dev-deps |
| `markymark-index/src/document.rs` | DocumentIndex, XmlTagEntry, from_ast |
| `markymark-index/src/realm.rs` | RealmIndex, ResolvedHeading/Block |
| `markymark-core/src/arena.rs` | ArenaHashMap, new_arena_hashmap |
| `docs/rust_crates/bumpalo.md` | Arena patterns |

---

## Recommended Next Steps (New Session)

1. **Improve sample quality**
   - Add real-markdown benchmark: load `.md` files from `docs/` or a fixture dir
   - Add size tiers: small (1KB), medium (10KB), large (100KB) per doc
   - Add benchmarks for N ∈ {100, 1_000, 5_000, 10_000}

2. **Fix memory measurement**
   - Run `cargo run --release -p markymark-index --bin memory_bench` (or similar) that does one index run, reports RSS, exits — avoid criterion's accumulated state
   - Or: measure RSS in a subprocess

3. **Re-run before/after**
   - Baseline at 9578d85 with improved sample
   - Current with improved sample
   - Compare: allocations, time, RSS, concurrency at scale

4. **Re-evaluate "was it worthwhile"**
   - If 10k docs show meaningful allocation/RSS reduction and similar or better latency: yes
   - If gains remain marginal: document as "low ROI for this workload" and consider scope

---

## Beads / Epics

- **marky-g9t:** Arena allocation epic (parser + index)
- **marky-luy:** g9t remediation closeout
- **marky-g9t.4 / g9t.5:** Index migration tasks (verified implemented; conformance gaps reverted/skipped)

---

## Quick Commands

```bash
# Benchmarks (current)
cargo bench -p markymark-index

# Tests
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets
```
