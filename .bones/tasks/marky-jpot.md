---
id: marky-jpot
title: 'Benchmark: validate md4c sub-1ms on 50KB doc vs tree-sitter 12.8ms baseline'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


## Design

## Goal

Validate the md4c streaming parser performance claim: sub-1ms for a full 50KB+ document, compared to tree-sitter's measured 12.8ms baseline (5.7ms block + 7.1ms inline FFI). Also verify extraction correctness (md4c and tree-sitter produce equivalent results).

## Context

All md4c infrastructure is complete (marky-s02r through marky-yfh7). The tree-sitter baseline was measured in release mode on a synthetic 57KB doc with 20 iterations (MEMORY.md 'Incremental Indexing Performance Deep-Dive').

Existing benchmark infrastructure:
- `markymark-kernels/benches/brza_kernels.rs` has criterion benchmarks comparing zig_heading_scan vs tree-sitter and zig_link_scan vs regex
- `generate_markdown_doc(target_bytes)` creates synthetic docs with headings, links, wiki links, code blocks
- `markymark-kernels/src/scan.rs` has env-gated benchmark pattern (`MARKYMARK_RUN_100K_BENCH`)
- Zig `build.zig` has `native-bench` step for Zig-level benchmarks

## Effort Estimate

4-6 hours

## Implementation Checklist

### Step 1: Add md4c criterion benchmark group to brza_kernels.rs

Add new criterion group `bench_md4c_vs_tree_sitter` in `markymark-kernels/benches/brza_kernels.rs`:

- Import `Md4cScanBackend` from `markymark_core::scanner`
- Use `generate_markdown_doc()` for 1KB, 10KB, 50KB, 100KB sizes
- Benchmark `Md4cScanBackend.scan_headings(doc)` vs `count_tree_sitter_headings(doc)`
- Benchmark `Md4cScanBackend.scan_links(doc)` (no existing tree-sitter link scan to compare)
- Use `Throughput::Bytes` for throughput reporting
- Use `black_box()` for all measured calls
- Sample size: 20 for <50KB, 12 for >=50KB (consistent with existing groups)

### Step 2: Add Zig-level md4c extraction benchmark

Add new build step `bench-md4c` in `zig/build.zig`:

- Create `zig/bench/md4c_bench.zig` benchmark file
- Call `extractFromMarkdown()` directly (no FFI overhead)
- Use same synthetic doc generation (port `generate_markdown_doc` logic or hardcode a representative doc)
- Measure with `std.time.Timer` over 1000 iterations
- Print: total time, per-iteration time, throughput in MB/s

### Step 3: Add correctness verification

In the Rust benchmark, add a one-time correctness assertion:
- Parse with tree-sitter, count headings
- Parse with Md4cScanBackend, count headings
- Assert counts match for the same document
- This runs once before the benchmark loop, not in the hot path

### Step 4: Run benchmarks and record results

- Run: `cargo bench --bench brza_kernels -- md4c` (release mode by default)
- Run: `cd zig && zig build bench-md4c` (ReleaseFast by default)
- Record results in MEMORY.md under new section 'md4c vs tree-sitter Performance'
- Include: absolute times, speedup ratios, FFI overhead percentage

### Step 5: Commit

- Commit benchmark files with results summary in commit message
- Do NOT modify any production code in this task

## Success Criteria

- [ ] Criterion benchmark group `bench_md4c_vs_tree_sitter` added to brza_kernels.rs
- [ ] Zig-level `bench-md4c` build step added with extractFromMarkdown() benchmark
- [ ] md4c heading count matches tree-sitter heading count on same document (correctness)
- [ ] md4c processes 50KB+ doc in sub-1ms (release mode, criterion warm cache)
- [ ] Speedup ratio vs tree-sitter documented (target: >10x for heading extraction)
- [ ] FFI overhead measured: Zig-only time vs Rust round-trip time documented
- [ ] Results recorded in MEMORY.md 'md4c vs tree-sitter Performance' section
- [ ] `cargo bench` runs clean, no warnings
- [ ] All existing tests still pass (no regressions from benchmark additions)

## Key Considerations (SRE Review)

**Correctness Before Performance**:
- If md4c heading count != tree-sitter heading count, investigate BEFORE benchmarking
- Differences may exist for edge cases (e.g. headings in code blocks) — document any known divergences

**Benchmark Methodology**:
- criterion handles warm-up, statistical analysis, outlier detection automatically
- Use Throughput::Bytes for meaningful MB/s comparison with md4c's claimed 200MB/s
- Do NOT add println or logging inside measured closures (distorts measurements)
- Always use `black_box()` to prevent dead code elimination

**Synthetic vs Real Docs**:
- `generate_markdown_doc()` produces representative markdown (headings, links, wiki links, code blocks, paragraphs)
- Real-world docs may differ (more inline formatting, deeper nesting)
- Synthetic doc is consistent with the original tree-sitter baseline measurement, so comparison is apples-to-apples

**If Sub-1ms NOT Achieved**:
- Document actual time and investigate where time is spent
- Check if FFI overhead dominates (compare Zig-only vs Rust round-trip)
- Check if allocation overhead is significant (extraction renderer allocates per-heading/per-link)
- Create follow-up issue for optimization if needed — do NOT block epic closure on specific timing threshold if architecture is sound

**Run-to-Run Variance**:
- criterion's default statistical analysis handles this
- Run on quiet machine (no heavy background processes)
- If variance >20%, increase sample_size

## Anti-patterns

- Do NOT benchmark in debug mode (only release/ReleaseFast)
- Do NOT include println/eprintln inside measured closures
- Do NOT forget black_box() — optimizer can eliminate entire benchmark
- Do NOT modify production code in this task — benchmark only
- Do NOT use wall-clock `Instant::now()` loops when criterion is available (Rust side)
- Do NOT hardcode iteration counts on Rust side — let criterion manage statistical sampling
