# Performance Guidelines (progressive)

**TL;DR:** Identify hot paths early, benchmark/profile, design for throughput (not empty cycles), and add yield points to long-running tasks.

**Checklist:**
- Detect performance-sensitive crates early; benchmark hot paths.
- Profile regularly (CPU + allocations); document hotspots.
- Optimize for throughput: batch work, avoid hot spins, yield when idle.
- Insert `yield_now().await` in long-running tasks (especially CPU-bound).

## Identify, Profile, Optimize the Hot Path Early (M-HOTPATH) { #M-HOTPATH }

<why>To end up with high performance code.</why>
<version>0.1</version>

Early in development, if the crate is performance or COGS relevant:

- identify hot paths and create benchmarks,
- regularly run a profiler (CPU and allocation insights),
- document or communicate the most performance sensitive areas.

Benchmark recommendations: [criterion](https://crates.io/crates/criterion) or [divan](https://crates.io/crates/divan). For meaningful CPU insights, enable debug symbols for benchmarks:

```toml
[profile.bench]
debug = 1
```

Common issues: frequent reallocations (cloned/growing strings), short-lived allocations vs. bump allocators, copying collections, repeated rehashing, default hasher where collision resistance is not required.

## Optimize for Throughput, Avoid Empty Cycles (M-THROUGHPUT) { #M-THROUGHPUT }

<why>To ensure COGS savings at scale.</why>
<version>0.1</version>

Optimize for items per CPU cycle. Do not pay for latency with empty cycles.

Do:
- partition work into reasonable chunks,
- let threads/tasks handle their own slices,
- sleep or yield when idle,
- design and use batched APIs,
- yield within long items or between chunks (see [M-YIELD-POINTS](#M-YIELD-POINTS)),
- exploit CPU caches/locality.

Avoid:
- hot spinning for single items,
- per-item work when batching is possible,
- unnecessary work stealing or over-sharing state.

## Long-Running Tasks Should Have Yield Points. (M-YIELD-POINTS) { #M-YIELD-POINTS }

<why>To ensure you don't starve other tasks of CPU time.</why>
<version>0.2</version>

Long-running computations should include `yield_now().await` points.

- I/O-heavy tasks will naturally yield at awaits.
- CPU-bound tasks should cooperatively yield regularly to avoid starving others:

```rust, ignore
async fn process_items(zip_file: File) {
    let items = zip_file.read().async;
    for i in items {
        decompress(i);
        yield_now().await;
    }
}
```

Yield cadence: aim for ~10–100μs of CPU work between yields to keep switching overhead negligible.

### Related
- Safety posture: `safety.md`
- Throughput yield guidance: `libraries-resilience.md` (statics) and `libraries-ux.md` (API design)
- Original: `../rust_guidelines_full.md`
