# claude-harness Memory Archive — 2026-02-18

**Source:** `.claude-harness/memory/` from `feature/incremental-indexing` branch before harness removal.
**Purpose:** Reference for refining the coding client project. Contains learned rules, patterns, and
decisions accumulated across all markymark development sessions (Feb 13–18 2026).

---

## Learned Rules (rules.json)

These rules were extracted from user corrections and direct experience. Each maps to a real failure.

| ID | Rule | Scope | Source |
|----|------|-------|--------|
| rule-001 | Write tool is blocked by harness PreToolUse hook when file content contains `${CLAUDE_PLUGIN_ROOT}`. Use Bash heredoc with single-quoted delimiter. | project | direct-experience |
| rule-002 | claude-harness v3+ tracks memory in git — do NOT gitignore `.claude-harness/memory/`, `features/`, `agents/`, `impact/`, `prd/`, `config.json`, `claude-progress.json`. Only `sessions/`, `memory/compaction-backups/`, `memory/working/` are ephemeral. | general | user-correction |
| rule-003 | Use built-in LSP tool for Rust, not Serena MCP. Serena has no Rust language server — its symbolic tools return garbage for `.rs` files. | tooling | user-correction |
| rule-004 | Hard stop at 1000 lines — cut P0 refactor bead immediately and escalate. The 500-line threshold is a suggestion; 1000 is a block. | code-quality | user-correction |
| rule-005 | Create documentation blockers before complex implementations with unfamiliar crates. Prefer retrieval-led reasoning over pre-training. | workflow | user-guidance |
| rule-006 | Bump `markymark-plugin/.claude-plugin/plugin.json` version alongside Cargo.toml. The plugin manifest version is NOT derived from Cargo.toml — must be updated manually. | project | user-correction |
| rule-007 | `epstein_20250227_all_in_one.md` is a local benchmark fixture — never include it in checkpoint/PR commits unless explicitly requested. | project | user-correction |
| rule-008 | Never squash merge — preserve full commit history. Squash destroys context, makes bisect harder, loses narrative. | general | user-correction |
| rule-009 | Use cargo-mcp tools for Rust build/test commands when available, rather than raw `cargo` CLI. | tooling | user-correction |
| rule-010 | Exclude generated benchmark artifacts from the input corpora used to compute those same metrics (prevents self-referential drift). | project | user-correction |

---

## Procedural Patterns (patterns.json — condensed)

### Code Patterns (selected high-value entries)

**Arena / Memory Safety**
- `ArenaHashMap` is `!Send` (Bump:!Sync); parser types can use it, index types must use `std::HashMap`
- `DocumentArena` wrapper preferred over raw `Bump` for per-document arenas (provides Debug, capacity)
- Arena empty slice: `bumpalo::collections::Vec::new_in(arena).into_bump_slice()` — not `&[]` (stack-local, causes UAF)
- Avoid cloning `ArenaHashMap` with bumpalo — causes SIGSEGV. Return `Vec<&T>` of arena refs instead.
- Use `arena_ref()` helper to deduplicate unsafe transmute instead of per-method unsafe blocks
- Self-referential arena migration: use `self_cell` owner/dependent internals on stable Rust first; avoid nightly

**Rust Drop Order**
- Rust drops struct fields in declaration order. Arena must be declared AFTER any fields that contain ArenaHashMap references (or the arena frees memory before element drop impls run → UB).

**LSP Incremental Editing**
- Centralize byte-range clamping in one helper; emit warning telemetry on clamp events for observability
- Incremental wiki-link update needs explicit tail-boundary check: edit starting at/after last existing link must force recomputation
- Selective merge: collect old → extract new → intersect ranges → preserve unaffected → replace affected neighborhood

**Benchmark Discipline**
- Tiered benchmark controls via env vars (`MARKYMARK_BENCH_SAMPLES`, `MARKYMARK_BENCH_DOC_TIER`) — separate fast dev iteration from scale validation
- Exclude report output files from the input corpus that drives those same metrics
- Automate run + parse + report with a dedicated binary; manual collection is error-prone

**FFI Bridging**
- Generic `call_scan_ffi<T>` helper with buffer retry eliminates per-function boilerplate
- Buffer retry: start 64, double on `-2`, max 3 retries
- `repr(C)` mirror structs at FFI boundary; idiomatic Rust types in public API
- `PhantomData<*mut ()>` for `!Send`/`!Sync` on stable Rust (impl !Trait requires nightly)
- Drop impl: set handle to null after destroy for idempotent double-free protection
- Gate non-zero section count/size with null-pointer check before memcpy in C ABI serializers

**Zig Kernel Conventions**
- SIMD for sparse pattern search: `@Vector` to find candidate positions, scalar for validation
- Share parsing logic between SIMD and scalar via `pub import` from reference module
- `exports_*.zig` + `comptime { _ = @import(...) }` in `c_adapter.zig` for composable ABI
- `EmbeddingIndex` uses `page_allocator` for persistent FFI-owned memory
- FFI functions must initialize all output parameters before error returns (Zig undefined = garbage)
- Padding bytes in caller-provided output buffers must be explicitly zeroed for deterministic output

**Testing**
- TDD for bash hooks: write all tests first (RED), then create files
- Use exact count assertions (`assert_eq!`) not `>=` in rename/edit tests — `>=` masked a closing-tag rename bug
- `require_marksman!` macro for graceful test skip when marksman unavailable in CI
- ChildGuard RAII for process cleanup in integration tests

**Workflow**
- When harness active/archive state drifts from beads reality, reconcile before continuing
- For cross-agent handoff, attach full CodeRabbit review (including codegen prompts) as beads comments
- Checkpoint updates: persist claude-progress, working-context, and memory layer files together

### Rust Patterns (structured entries)

| Pattern ID | Pattern | When to Apply |
|-----------|---------|---------------|
| pat-core-contract-first | Core-contract-first transport layering | Adding/modifying LSP/MCP transport behavior |
| pat-arena-hybrid | Hybrid arena: per-document Bump + realm-level owned String for cross-doc | Arena ownership in dual-transport |
| pat-lock-safety | Drop read lock before async publish_diagnostics | Any RwLock + async call combination |
| pat-startup-index-mcp | Validate roots once, index at startup, keep execute() read-only | MCP RuntimeEngine initialization |
| pat-binary-smoke-testing | ChildGuard + thread+channel for timeout, Content-Length for LSP, line-delimited for MCP | Integration smoke tests |
| pat-dual-process-alignment | Normalize URIs to filenames, sort by uri+range, classify Match/Superset/Mismatch/ServerOnly | LSP alignment testing |
| pat-arena-ref-helper | Single `arena_ref()` helper with clear SAFETY comment | Self-referential arena-backed AST |
| pat-tiered-benchmark-reporting | MARKYMARK_BENCH_SAMPLES + MARKYMARK_BENCH_DOC_TIER + dedicated runner binary | Performance validation |
| pat-release-workflow | Quality gates → version bump (Cargo + plugin.json) → exact = deps for pre-release → branch+PR → tag | Any alpha/pre-release |
| pat-readme-audit | Explore agent to verify features exist in source, cross-ref CLI --help and CI | Pre-release README accuracy |

---

## Procedural Successes (successes.json — selected high-value entries)

These represent approaches that worked and should be reused.

### TDD Workflows

**suc-001 / suc-010 / suc-017:** Bash hook + FFI function TDD.
- Write all tests first (all RED)
- Create implementation → GREEN
- **Caught** 3 tautological `expect(true)` tests in Zig scaffold via `review-implementation` skill

**suc-012:** Arena migration regressions resolved via placeholder empty slices replaced by `arena_ref()` + `into_bump_slice()`.

**suc-030:** Miri arena validation: 14 tests in markymark-core (no FFI deps) replicating production arena patterns. Caught UAF with literal `&[]` before it hit CI.

### Release Engineering

**suc-009 / suc-011:** Alpha release workflow — parallel quality gates, version bump across Cargo.toml + plugin.json, cliff.toml changelog, exact `=` pinning for pre-release cross-crate deps.

### Zig FFI Integration

**suc-016:** `markymark-kernels` scaffold — `build.rs` invokes `zig build lib` via `std::process::Command`, zero build-dependencies. `rerun-if-changed` via `walkdir` for individual `.zig` files.

**suc-021:** PIC fix: `.pic = true` on Zig module in `build.zig` resolves `R_X86_64_32` relocation errors on Linux x86_64.

**suc-022 / suc-033:** Rust FFI wrappers — 4 scan functions with buffer retry, 2 simple functions (token estimate, content hash), opaque handle pattern for EmbeddingIndex.

**suc-031:** Benchmark-driven investigation: targeted arena lifecycle benches showed arena = 0.07% of cost, tree-sitter dominates 100-150x → arena reuse not worth implementing.

**suc-040:** Zig-kernels CI job with `dorny/paths-filter` for step-level path gating. Matrix: ubuntu-22 + macos-14 (NEON). Confirmed `.pic = true` required for Linux.

### Incremental Indexing

**suc-025:** FULL→INCREMENTAL text sync with UTF-16 byte offset conversion. `DocumentChanges::Incremental` handler.

**suc-026:** Incremental tree-sitter parsing wired end-to-end: `Parser::parse_with_old_tree` passes `MarkdownTree` cursor + `InputEdit` for ~1.3x speedup on edits.

**suc-042 (marky-77x):** `DocumentIndex::from_ast_with_wiki_links` + `ServerState` selective merge. Affected range + neighbor window + tail-boundary guard. Parity + edge case coverage in state_tests.

**suc-039:** Fixed `mem::forget` leak in `DocumentIndex::from_ast` via `Ast::take_arena()` — destructures Ast without running drop, transfers arena ownership cleanly.

### Multi-Agent Patterns

**suc-015:** SRE task refinement via background Opus subagent: dispatched to review 26 tasks, returned prioritized breakdown. Effective for large task lists.

**suc-013:** Batch PR review triage: categorize all comments into (actionable / false-positive / design-level / deferred) before touching code. Prevents rabbit holes on low-priority feedback.

---

## Episodic Decisions (decisions.json — condensed)

Decisions with ongoing relevance are in MEMORY.md under "Key Architectural Decisions".
Additional one-time decisions recorded here for completeness:

**dec-020:** `extract_list_items` returns `Vec<&ListItem>` not `Vec<ListItem>` — ArenaHashMap clone causes SIGSEGV.

**dec-021:** Arena empty slice via `bumpalo Vec::new_in(arena).into_bump_slice()` — literal `&[]` is stack-local, causes UAF.

**dec-022:** Custom `CountingAllocator` in benchmark binary for heap allocation metrics (wraps `System` allocator with atomic counter).

**dec-023:** `BlockId` extended with source `Range` from regex full match — enables go-to-definition to jump to actual position rather than file start.

**dec-024:** Keep baseline worktree at `/tmp/markymark-baseline` for pre/post-benchmark comparison; PID cleanup only when explicitly requested.

**dec-025:** `MARKYMARK_BENCH_HEAVY=1` for 100-sample runs; default 10–20 to avoid RAM exhaustion on index_docs_dir (918 files, ~5.9MB per iteration).

**dec-028:** Preserve duplicate block IDs in RealmIndex — store all occurrences, return first match. Overwriting silently dropped cross-document references.

**dec-029:** Upgrade to bumpalo 3.19.1 + hashbrown 0.16.1 — CodeRabbit flagged outdated versions.

**dec-pr-review-001:** Use `LazyLock<Regex>` for all 14 regex patterns in `extract.rs` — CodeRabbit flagged per-call recompilation as Major. `LazyLock` is in std since Rust 1.80.

**dec-pr-review-002:** `MarkdownLinkEntry.url` stores base URL only (no fragment); anchor stored separately. Prevents duplication between `url` and `anchor` fields.

**dec-pr-review-003:** Accept `'static` lifetime pattern with documentation; defer `self_cell` migration to marky-5yt. Pragmatic given arena migration scope.

**dec-033:** Merge `origin/main` into feature branch (not rebase) to resolve PR squash merge conflicts. Rebase failed previously; merge preserves all history.

---

## Semantic Architecture Summary (architecture.json)

### Tech Stack

| Component | Version |
|-----------|---------|
| Language | Rust |
| LSP | tower-lsp-server 0.23 (community fork — uses `ls_types` not `lsp_types`) |
| MCP | rmcp 0.13 (official Rust SDK — requires `schemars 1.x`) |
| Parser | tree-sitter 0.26.5 + tree-sitter-md 0.5.2 |
| Graph | petgraph 0.7 (`StableGraph`) |
| Allocator | bumpalo 3.19.1 + hashbrown 0.16.1 |
| SIMD | Zig `@Vector(16, u8)` — NEON on Apple Silicon, SSE on x86 |
| Testing | cargo nextest + bash test scripts |

### Key Code Style Rules (from architecture.json)
- `#[expect(lint, reason="...")]` preferred over `#[allow(lint)]` per M-LINT-OVERRIDE-EXPECT
- `DocumentArena` wrapper preferred over raw `Bump` for per-document arenas
- `self_cell` owner/dependent as first stable migration step for self-referential types
- `RealmIndex` cross-doc maps remain `owned String`-backed until `self_cell`/ouroboros migration (marky-5yt)
- Compile-fail doctests to lock lifetime leakage invariants during migration
- For incremental edits: pair out-of-range clamping with deterministic warning for observability

---

## Notes for Coding Client Project

These patterns extracted from the harness memory are reusable beyond this project:

1. **The `call_scan_ffi<T>` + buffer retry pattern** (start 64, double on -2, max 3 retries) is a clean template for any Rust→Zig FFI with variable-length output.

2. **`LazyLock<Regex>` for static regex compilation** (std since Rust 1.80) is strictly better than `once_cell::Lazy` for new projects — no external dependency.

3. **SRE task refinement via background Opus subagent** for large task lists (26+ items) was highly effective. The subagent produces a prioritized breakdown without consuming the main context.

4. **Batch PR review triage** before touching code (actionable / false-positive / design-level / deferred) prevents low-priority feedback from derailing implementation.

5. **Tiered benchmark controls via env vars** (`LIGHT` / `MEDIUM` / `HEAVY` tiers) is cleaner than feature flags or hardcoded constants for CI vs local profiling.

6. **ChildGuard RAII + thread+channel timeout** (not tokio) for binary integration tests is reliable and doesn't require async infrastructure in test binaries.

7. **`dorny/paths-filter` for step-level CI gating** on matrix builds — only run Zig compilation on `.zig` file changes without needing separate workflows.

8. **Miri validation in a no-FFI crate** (replicated production patterns in isolation) is the right approach for verifying arena lifetime correctness before wiring up FFI.
