# Core Rust Patterns (language, std, Cargo)

<agent>
<goal>Provide a compact “Rust fundamentals” reference tuned for agent-assisted implementation work.</goal>
<when_to_use>When you’re unsure about idiomatic Rust structure, error handling boundaries, lifetimes/borrowing, or Cargo workspace conventions.</when_to_use>
<contains>Project layout, modules, errors, lifetimes, ownership, async basics, logging, testing layout, Cargo workspace patterns</contains>
<see_also>error-handling.md, testing.md, bumpalo.md, tower-lsp.md</see_also>
</agent>

**TL;DR:** Keep crates small, APIs typed, errors explicit at library boundaries, and workspaces predictable.

**Checklist:**
- Use a Cargo workspace with clear crate roles (`*-core`, `*-parser`, `*-index`, `*-lsp`).
- Libraries expose typed errors (`thiserror`); apps/bins can use `anyhow`.
- Prefer owned outputs at public boundaries; use borrows internally for perf.
- Keep async boundaries explicit; don’t hold locks across `.await`.
- Add tests next to behavior (unit) and alongside fixtures (integration/snapshots).

---

## Cargo workspace defaults

### Suggested crate split (for LSP-style systems)
- `*-parser`: parsing + AST/CST extraction (tree-sitter)
- `*-index`: semantic indexing + cross-doc resolution
- `*-graph`: connection graph + algorithms
- `*-lsp`: LSP server + request handlers (tower-lsp)
- `*-cli` (optional): dev tooling / debugging

### Workspace `Cargo.toml` conventions

```toml
[workspace]
members = [
  "crates/*",
]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
thiserror = "1"
anyhow = "1"
tracing = "0.1"
```

### Feature policy (high-level)
- Prefer additive features; avoid “removing API surface” via features.
- If you need std/no-std splits, prefer a `std` feature (default-on) rather than a `no-std` feature.

---

## Ownership & lifetimes (practical rules)

### Boundaries: prefer owned
At public API boundaries (especially cross-crate), prefer returning owned values (`String`, `Vec<T>`, `Arc<T>`) rather than references. Borrowing is great internally, but tends to leak lifetime complexity outward.

### Internals: borrow for hot paths
Inside a crate/module, borrowed data (`&str`, `&[T]`) is often best for perf. If you need many small allocations with a bulk lifetime, use an arena (see `bumpalo.md`).

### Prefer slices and iterators
Accept `impl AsRef<str>` / `impl AsRef<Path>` for ergonomic inputs where it doesn’t reduce clarity. Prefer returning iterators for potentially large collections.

---

## Error handling boundaries

### Libraries: typed, structured
- Define a small number of error enums (or structs) with clear variants.
- Keep variant payloads minimal and meaningful; attach sources with `#[source]`.

### Applications/binaries: contextual
- Use `anyhow::Result` and attach context at I/O / boundary points.

See: `error-handling.md`.

---

## Concurrency + async (baseline)

### Don’t hold locks across `.await`
When using `tokio::sync::Mutex/RwLock`, acquire, copy what you need, then drop the guard before awaiting.

### Prefer message passing for high contention
If many async tasks need to mutate shared state, consider channels (`tokio::sync::mpsc`) to serialize mutations, especially for “index update” style work.

### Cancellation posture
Treat cancellation as normal in async: make functions idempotent, and avoid partial shared-state writes without a plan (transaction-like approach or staged updates).

---

## Logging / tracing (house defaults)

### Prefer structured logs
Use `tracing` with structured fields. Avoid interpolated “stringly” logs in hot paths.

```rust
use tracing::{info, warn};

info!(uri = %uri, version = %version, "initialized");
warn!(path = %path.display(), error = %err, "failed to read file");
```

---

## Testing layout

### Unit vs integration
- Unit tests: next to code, fast, small.
- Integration tests: `tests/` directory, use real I/O boundaries where possible.

### Snapshot + property tests
- Use `insta` for “complex output” comparisons (AST, formatted text, diagnostics).
- Use `proptest` for invariants (“never panics”, “roundtrip holds”, “no invalid indices”).

See: `testing.md`.

---

## Quick routing
- LSP server patterns: `tower-lsp.md`
- Incremental parsing patterns: `tree-sitter.md`
- Connection graphs: `petgraph.md`
- Arena allocation: `bumpalo.md`
- Typed vs contextual errors: `error-handling.md`
- Snapshots + property tests: `testing.md`

