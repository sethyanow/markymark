---
id: marky-8d08
title: 'Cross-environment benchmark validation: md4c vs tree-sitter performance'
status: open
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


<context>
## What This Is

This task validates md4c streaming parser benchmark results across different environments (OS, hardware, cloud). The benchmarks were built in **marky-jpot** (now closed) as the final step of **epic marky-0mr** (Zig md4c streaming parser as fast-path replacement for tree-sitter).

### Why Cross-Environment Testing Matters

Our baseline results come from a single macOS ARM64 machine. Before closing the epic and committing to md4c as the production fast path, we need confidence the speedup holds across:
- Different OS (Linux, Windows)
- Different CPU architectures (x86_64, ARM64)
- Cloud VMs (potentially slower single-thread, different memory subsystems)
- Different Zig/Rust toolchain versions

### Related Issues
- **marky-0mr**: Parent epic — "G: Zig md4c streaming parser as fast-path replacement for tree-sitter"
- **marky-jpot**: Benchmark task that created the benchmarks (CLOSED)
- **marky-77i**: Parent epic for incremental indexing (10x speedup target)
</context>

<baseline_results>
## Baseline Results (macOS ARM64, Apple Silicon)

These are the numbers to compare against. Measured with criterion (Rust) and std.time.Timer (Zig).

### Criterion Benchmarks (Rust, release mode)

| Size | md4c extract only | md4c from_scan | tree-sitter from_ast | Pipeline Speedup |
|------|------------------|----------------|---------------------|-----------------|
| 1KB  | 0.115ms          | 0.229ms        | 0.490ms             | **2.1x**        |
| 10KB | 0.850ms          | 1.836ms        | 4.573ms             | **2.5x**        |
| 50KB | 4.686ms          | 9.436ms        | 26.662ms            | **2.8x**        |
| 100KB| 9.882ms          | 20.692ms       | 66.962ms            | **3.2x**        |

### Zig-Only Benchmarks (ReleaseFast, GPA allocator, 1000 iterations)

| Size  | Time/iter | Throughput | Headings | Links |
|-------|-----------|------------|----------|-------|
| 1KB   | 0.075ms   | 15.1 MB/s  | 5        | 16    |
| 10KB  | 0.189ms   | 53.1 MB/s  | 37       | 144   |
| 50KB  | 2.129ms   | 23.0 MB/s  | 177      | 704   |
| 100KB | 9.568ms   | 10.2 MB/s  | 352      | 1404  |

### Key Finding
- **Sub-1ms at 50KB was NOT achieved** (4.7ms extract-only via FFI)
- **md4c is 2.8x faster** than tree-sitter for full pipeline at 50KB
- **Extraction allocation overhead** dominates at scale (not raw parse speed)
</baseline_results>

<instructions>
## How To Run The Benchmarks

<important>
You MUST build in **release mode**. Debug mode benchmarks are meaningless.
Do NOT add println/logging inside benchmark closures.
Do NOT modify any production code — benchmarks only.
</important>

### Prerequisites
- Rust toolchain (stable)
- Zig 0.15.2+
- The `marky-0mr` branch checked out

### Step 1: Detect Your Environment

Run this and save the output — you will need it for your results comment:

```bash
echo "=== Environment ==="
uname -a
echo "CPU: $(uname -m)"
echo "OS: $(uname -s) $(uname -r)"
# Linux: cat /proc/cpuinfo | grep "model name" | head -1
# macOS: sysctl -n machdep.cpu.brand_string
# Windows (PowerShell): Get-CimInstance Win32_Processor | Select Name
rustc --version
zig version
echo "==================="
```

If you are an AI agent, also include any environment info from your system prompt (cloud provider, VM type, region, etc.).

### Step 2: Run Rust Criterion Benchmarks

```bash
# From project root
cargo bench --bench brza_kernels -- md4c_vs_tree_sitter
```

This runs 12 benchmarks (3 approaches x 4 sizes). Each prints time and throughput estimates. Look for the `md4c_vs_tree_sitter` group results. Typical runtime: 3-5 minutes.

**Save the full output.** You will paste it into the comment.

### Step 3: Run Zig-Only Benchmark

```bash
# From zig/ subdirectory
cd zig
zig build bench-md4c
```

Prints a table with per-size results showing us/iter, MB/s, and element counts.

**Save the full output.** You will paste it into the comment.

### Step 4: Run Tests (Verify Nothing Is Broken)

```bash
# From project root
cargo nextest run
cd zig && zig build test
```

All tests should pass. If any fail, note the failure — it may indicate a platform-specific issue with the md4c port or FFI bridge.

### Step 5: Post Results as a Comment on This Issue

<critical>
THIS IS THE MOST IMPORTANT STEP. Do not skip it.

Use `bd comment` to add your results to this issue so all runs are collected in one place.
</critical>

Use this exact template — fill in your values and run:

```bash
bd comment marky-8d08 "## Benchmark Run: [YOUR ENVIRONMENT NAME]

### Environment
- **OS**: [e.g. Ubuntu 24.04 x86_64, Windows 11 ARM64, macOS 15 ARM64]
- **CPU**: [e.g. AMD EPYC 7R13, Apple M3 Pro, Intel i7-13700K]
- **RAM**: [e.g. 16GB, 32GB]
- **VM/Cloud**: [e.g. GitHub Codespaces 4-core, EC2 c7g.xlarge, bare metal]
- **Rust**: [rustc version]
- **Zig**: [zig version]
- **Agent**: [model name if AI agent, or 'human']

### Criterion Results (50KB — the key comparison point)

| Benchmark | Time (ms) | vs Baseline |
|-----------|-----------|-------------|
| md4c_extract_only/50kb | [VALUE] | [baseline: 4.686ms] |
| md4c_from_scan/50kb | [VALUE] | [baseline: 9.436ms] |
| tree_sitter_from_ast/50kb | [VALUE] | [baseline: 26.662ms] |
| **Pipeline Speedup** | **[X.Xx]** | [baseline: 2.8x] |

### Zig-Only Results

| Size | us/iter | MB/s |
|------|---------|------|
| 1kb  | [VALUE] | [VALUE] |
| 10kb | [VALUE] | [VALUE] |
| 50kb | [VALUE] | [VALUE] |
| 100kb| [VALUE] | [VALUE] |

### Test Results
- Rust: [PASS/FAIL] ([N] tests)
- Zig: [PASS/FAIL]

### Anomalies
[Any unexpected results, failures, or observations. Write 'None' if clean run.]
"
```

<important>
If `bd comment` is not available or fails, append results to the issue notes instead:
```bash
bd update marky-8d08 --notes "... your results ..."
```
</important>
</instructions>

<what_we_care_about>
## What We Are Looking For

1. **Does the 2.5-3x speedup hold across environments?**
   - If yes: md4c is validated as the production fast path
   - If speedup drops below 1.5x somewhere: investigate why

2. **Does correctness hold?**
   - The benchmark includes an assertion that md4c heading count == tree-sitter heading count
   - If this assertion fails on any platform, that is a critical finding

3. **Are there platform-specific issues?**
   - Zig cross-compilation quirks
   - FFI alignment issues on different architectures
   - Memory allocator behavior differences
</what_we_care_about>

<files>
## Key Files

- `markymark-kernels/benches/brza_kernels.rs` — Criterion benchmark (bench_md4c_vs_tree_sitter function)
- `zig/bench/md4c_bench.zig` — Zig-only extraction benchmark
- `zig/build.zig` — Contains bench-md4c build step
- `zig/src/md4c/` — The vendored md4c parser (from Bun, MIT)
- `zig/src/md4c/extraction_renderer.zig` — ExtractionRenderer under test
- `zig/src/md4c/exports.zig` — C ABI exports for FFI
- `markymark-kernels/src/md4c.rs` — Rust FFI bindings
- `markymark-core/src/scanner.rs` — Md4cScanBackend implementation
- `docs/MEMORY.md` — Baseline results and analysis
</files>
