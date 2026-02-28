# Architecture Patterns — markymark

Cross-cutting architectural decisions and conventions.
Linked from [MEMORY.md](../MEMORY.md). Load when making structural changes.

---

## Crate Structure

Seven-crate workspace (core, parser, index, lsp, mcp, cli, kernels) is well-partitioned.
Arena allocation (bumpalo) lives in parser layer, not crossing into transport (lsp/mcp).
This keeps Send/Sync constraints manageable.

## Arena Allocation

- **ArenaHashMap (!Send) restricted to parser types only; index types use std HashMap**
  (dec-arena-send-001). Bump:!Sync → &Bump:!Send → ArenaHashMap:!Send. tower-lsp requires
  Send+'static for async handlers.
- **DocumentIndex uses bare DocumentArena + unsafe impl Sync instead of Mutex** (marky-f9vv).
  The previous Mutex wrapper led to UB via raw-pointer MutexGuard escape. Bare arena +
  `unsafe impl Sync` is sound because the arena is only mutated during single-threaded
  self_cell construction; post-construction access is read-only.
- **Adopted DocumentArena wrapper in Ast and DocumentIndex** (dec-docarena-adopt-001).
  Provides Debug, capacity hints, semantic boundary vs raw Bump.
- **Reorder Ast struct fields so root_elements before arena** (dec-031). Rust drops in
  declaration order. Arena must outlive elements.
- **Arena reuse via reset is NOT worth implementing** (dec-041). Arena lifecycle = 0.07%
  of reparse cost.
- Avoid cloning ArenaHashMap with bumpalo — SIGSEGV. Return `Vec<&T>` instead.
- `bumpalo Vec::new_in(arena).into_bump_slice()` for empty arena slice, not `&[]` (UAF).
- When migrating wrapper types, trace all ptr::read/mem::forget — type must match.

## Self-Referential Types

- **self_cell owner/dependent for Ast internals on stable Rust** (dec-051).
- **Parameterize SymbolAtPosition over lifetime instead of 'static** (dec-049). LSP state
  clones index entries; 'static caused borrow escape errors.

## Semantic Index Concurrency (marky-ysv8)

- **SemanticIndex wrapped in Arc<tokio::sync::Mutex> inside RealmIndex** (dec-ysv8-001).
  Allows the MCP engine to clone the Arc handle, release the outer realm RwLock, and run
  async search without blocking realm-level write operations.
- **tokio::sync::Mutex (not std::sync::Mutex)** because `SemanticIndex::search()` is async.
- **No `blocking_lock()` in async call chains** (marky-wnjk). `RealmIndex::remove_document`
  and `detect_semantic_duplicates` are async and must use `lock().await`; calling
  `blocking_lock()` from Tokio runtime paths panics.

## Feature Flag Strategy (2026-02-27)

- **Feature flags are distribution-target profiles, not end-user choices.** The `semantic-search`
  and `local-embeddings` flags exist so each distribution channel gets the right dependency set:
  - **Library reuse**: no flags, slim deps (33 crates, 4.8 MB)
  - **CLI/LSP power users**: default build, fast markdown tooling
  - **Editor plugins**: `semantic-search` + optionally `local-embeddings`, hardcoded in build config
- **Measured binary impact** (release, 2026-02-27):
  - `semantic-search` adds reqwest/TLS: +1.7 MB, +196 deps
  - `local-embeddings` adds fastembed/ONNX: +17.5 MB more, +236 deps, +100 MB runtime model
- **Do NOT fold `semantic-search` into default.** Core stays slim for library consumers.
- **Future: multi-provider support.** Voyage is the only embedding provider today. The
  `EmbeddingProvider` trait in markymark-core is the extension point.

## Engine Pipeline (Epic H)

- **End positions computed in Zig** — heading, link, and block-id end positions are calculated
  during document construction in `zig/src/engine/document.zig`.
- **Engine lifecycle: create → update → get_blob → from_blob → destroy** — per-document
  `DocumentEngine` in `ServerState.engines` HashMap. Same URI key as `documents` and `realm`.
- **Blob format**: 64B header (magic 0x4D4B5343) + packed struct arrays + contiguous text pool.
  Rust `DocumentIndex::from_blob()` copies text from blob pool into arena.
- **from_ast()/from_scan() retained** for MCP batch and backward compat.
- **Tree-sitter stays separate** for lazy AST (hover/goto-def).

## Resolution Layer

- **`resolve_markdown_link` handles cross-document links** (marky-z9z). Resolves `other.md` →
  Document, `other.md#heading` → Heading, `dir/other.md` → path-relative first, stem fallback.
- **Path-relative without filesystem access**: Component-stack normalization (pop on `..`,
  skip `.`/empty) instead of `std::fs::canonicalize`.
- **Stem-only is the fallback, not the primary**: Path-relative resolution wins when URL
  contains `/`.

## LSP/MCP Conventions

- **Diagnostic logic lives in `markymark-index/src/diagnostics.rs`** (marky-6i9). Shared
  `compute_diagnostics(index, realm, uri)` used by both LSP and MCP.
- **Adding a new CoreOperation follows a 5-stop pattern**: (1) types in `core/engine.rs`,
  (2) compute logic in `index/`, (3) engine handler in `mcp/src/engine/{op}.rs`,
  (4) DTO + tool handler in `mcp/src/tools/{op}.rs`, (5) `#[tool]` wiring in `lib.rs`.
- Drop read lock before async publish_diagnostics (deadlock prevention).
- LSP character offsets from clients are untrusted — always bounds-check before byte-slicing.
- MCP handlers that accept any URI kind must use `realm.get_any_document()` and branch on
  `AnyDocumentIndex`.
- For edit-delta math on `u32`/`usize` positions, use explicit saturating add/sub with signed
  deltas to prevent wraparound.
- **Global monotonic counter for generation-based cleanup** — when a HashMap entry must be
  removed on close but a stale async task might race with a reopen, use a global monotonic
  counter (not per-key counters starting from 1).

## Testing Conventions

- Safe file splits: (1) module dir, (2) extract types, (3) extract helpers, (4) extract tests.
  Each step: edit → test → commit.
- Rust-analyzer shows transient `unlinked-file` diagnostics after creating a new module file;
  clears after `mod <name>;` and workspace rebuild.
- Use `assert_eq!` not `>=` — `>=` masked a closing-tag rename bug.
- Integration test crate roots (`tests/*.rs`) resolve `mod foo;` in `tests/foo.rs` (sibling),
  NOT `tests/basename/foo.rs`. Use `#[path = "basename/foo.rs"] mod foo;` for subdirectory splits.
- Env-gated benchmarks (`MARKYMARK_RUN_100K_BENCH=1`) for checkpoint evidence.

## Release Process

### Version Locations

| File | Field | Notes |
|------|-------|-------|
| `Cargo.toml` | `workspace.package.version` | All crates inherit via `version.workspace = true` |
| `markymark-plugin/.claude-plugin/plugin.json` | `version` | NOT auto-derived — bump manually (Rule #4) |
| `Cargo.lock` | 7 internal crate entries | Regenerated by `cargo build` after version bump |

### Known Pitfalls

1. **plugin.json forgotten** — must be bumped manually every release.
2. **Cargo.lock not committed** — `cargo build` regenerates after version bump; commit both.
3. **Publish order staleness** — re-derive from `cargo metadata` before publishing.
4. **Inter-crate dependency versions** — each crate's `Cargo.toml` has explicit `version = "X.Y.Z"`
   on internal dependencies; must be bumped alongside workspace version.
5. **Worktree prevents main checkout** — tagging done from main worktree by human.

### Conventions

- **Tag format:** `vMAJOR.MINOR.PATCH` on `main` branch only
- **Publish order:** kernels → core → parser → index → lsp/mcp (parallel) → cli
- **Skill:** See `prepare-release` skill for guided 5-phase workflow
- **Release notes:** Auto-generated git-cliff notes replaced with curated narrative notes
