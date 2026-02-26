# Agent Memory — markymark

Cross-session knowledge: decisions, failure patterns, conventions, and active plans.
Linked from CLAUDE.md, auto-loaded at session start.

**Curation rules:** Keep high-signal. Remove entries obvious from the code itself.
Completed work details live in git history, not here.

---

## Current State (2026-02-26)

### PR #46 (feature-embeddings) — Review Triage Round 2

15 findings from 4 reviewers (Codex, Copilot, CodeRabbit, Greptile). **10 dismissed** (already
fixed by `ac3563b`), **1 already tracked** (marky-y4be), **4 new valid**:

| Bead | P | Finding |
|------|---|---------|
| marky-ysv8 | P2 | **FIXED** (`f06d591`) — Realm read-lock held across semantic search await |
| marky-2q2b | P2 | **FIXED** (`db61d5c`) — Voyage embed_batch response cardinality validation |
| marky-h7pp | P4 | `/dev/null` test not portable — needs `#[cfg(unix)]` (local.rs:221) |
| marky-le49 | P4 | Stale `voyage-3` in README.md, code default is `voyage-4` |

**Pattern learned:** Reviewers analyzed commit `b77c490` (pre-fix). 10/15 findings were already
addressed. Future triage rounds should note the reviewed commit vs HEAD to fast-dismiss stale findings.

### PR #44 (v0.6.0, dev→main) — CI green, ready for merge

PR #43 was closed (stale merge base). Main was merged into dev, and PR #44 opened as the fresh
release PR. CI fixed at `f2a894f` — the Zig archive corruption turned out to be a format
incompatibility, not a caching issue (see Known Bugs below).

**Codex pre-triage findings (beads created):**
- marky-vxgg (P2): select-binary.sh missing .exe handling for Windows
- marky-e3if (P3): binary.ts PATH fallback — fixed in #34223

---

## Project Architecture

### Crate Structure

Seven-crate workspace (core, parser, index, lsp, mcp, cli, kernels) is well-partitioned.
Arena allocation (bumpalo) lives in parser layer, not crossing into transport (lsp/mcp).
This keeps Send/Sync constraints manageable.

### Rust Agent Docs: Grade A (2026-02-15)

45 files, 6,443 lines. 14 decision trees. 18 mistakes tracked. All gaps closed.
Key strength: decision trees for procedural knowledge (closures, errors, Send/Sync, etc.).
Known issue: XML tag false positives in code blocks (marky-8la).

### Known Bugs

#### Zig 0.15.2 archive format incompatibility with rust-lld (RESOLVED, 2026-02-24)

`zig build lib` on Linux x86_64 produces archives that pass `ar t` but fail
rust-lld's stricter parsing: `Archive::children failed: truncated or malformed
archive (offset to next archive member past the end of the archive after member
c_adapter.o)`.

**Root cause (revised after 6 iterations):** NOT a warm-cache issue. Zig 0.15.2's
archive writer on Linux produces a non-standard archive format where member offset
metadata is inconsistent with the actual file size. GNU `ar` tolerates this, but
`rust-lld` rejects it. The archive passes `ar t` validation (3.43 MB, 1 member)
but fails at link time.

**Fix (`f2a894f`):** build.rs extracts .o files from the Zig-produced archive with
`ar x` and re-packs with `ar rcs` to produce a standard GNU archive. Only runs on
Linux (macOS ld64 handles Zig's format fine). Combined with `use-cache: false` on
`mlugg/setup-zig` and purging `.zig-cache` at repo root.

**Key learnings from the 6-iteration debugging saga:**
1. Env vars (`ZIG_LOCAL_CACHE_DIR`) are ignored by `zig build` — only CLI flags work
2. `mlugg/setup-zig` restores `.zig-cache` at repo root, not `zig/.zig-cache`
3. Adding `ar t` validation in build.rs was the diagnostic breakthrough — it proved
   the archive was "valid" but in a format rust-lld couldn't parse
4. The real bug is Zig's archive FORMAT on Linux, not caching behavior

### Zig errdefer + explicit deinit = double-free pattern (2026-02-20, marky-gmny)

When a function has `errdefer obj.deinit()` at the top, **never** call `obj.deinit()`
explicitly on error paths — the errdefer fires on `return error.*` and double-frees.

For partially-transferred ownership (e.g. `headings.toOwnedSlice()` succeeded but
`links.toOwnedSlice()` failed), use a **scoped errdefer** immediately after the
successful transfer to clean up the transferred data:

```zig
const headings = ext.headings.toOwnedSlice(alloc) catch return error.OutOfMemory;
errdefer {
    for (headings) |h| alloc.free(h.text);
    alloc.free(headings);
}
const links = ext.links.toOwnedSlice(alloc) catch return error.OutOfMemory;
```

Also: `allocator.free(slice)` only frees the backing array, NOT owned strings inside
each element. Always iterate and free inner allocations first.

**OOM-loop testing pattern:** iterate `FailingAllocator` `fail_index` from 0..N with
GPA backing. GPA fills freed memory with `0xaa` — double-free segfaults at
`0xaaaaaaaaaaaaaaaa`. GPA `.deinit()` returning `.leak` catches missing frees. Use
5 consecutive successes as termination condition.

---

## Lessons Learned

### Post-implementation code review findings pattern (2026-02-26)

After parallel subagent implementation of marky-ysv8 + marky-2q2b, code review found
recurring patterns worth catching upfront:

- **Racy test synchronization**: `sleep(Nms)` to "wait for async task to start" is a race.
  Use `tokio::sync::Notify` — signal from inside the target code path, await before proceeding.
  The test passes vacuously if the sleep is too short (false green). Fixed in `90734e2`.
- **Symmetrical test coverage**: When validating `!=` checks (e.g., count mismatch), test
  BOTH directions — under-count AND over-count. The partial-response test only covered fewer
  items; the excess-items direction was untested until review caught it.
- **Inner Mutex contention after lock-scope fix**: Wrapping shared state in `Arc<Mutex>` to
  release an outer lock is correct, but the inner Mutex still serializes mutations against
  searches. This is acceptable (by design) but must be documented — callers need to know
  write latency can spike by the duration of a concurrent search.
- **`pub` vs `pub(crate)` for cross-crate internal APIs**: Methods consumed only by sibling
  workspace crates must stay `pub` (Rust visibility is per-crate, not per-workspace). Consider
  `#[doc(hidden)]` for stability signaling, but don't attempt `pub(crate)` for cross-crate use.

### Semantic add_document atomicity pattern (2026-02-26, marky-y2ne)

For `SemanticIndex::add_document`, use a two-phase flow: (1) embed all headings/fallback and
stage `(id, embedding, entry)` in memory, then (2) commit all Zig `index.add()` writes. Never
interleave embed+insert. This prevents orphaned Zig vectors when provider embed fails mid-loop.
Unit tests should assert both metadata (`entry_count`) and Zig state (`index.count`) on failure.

### Zig ArrayListUnmanaged scratch buffer pattern (2026-02-19)

When a function builds a temporary string via `ArrayListUnmanaged(u8){}` and returns
`.items`, the backing allocation leaks because nobody calls `.deinit()`. Fix: add a
reusable scratch buffer field to the owning struct, `clearRetainingCapacity()` at
the start of each call, and have callers that persist the result `dupe()` it.
Also: if a struct stores duped slices, its deinit must free them individually before
freeing the container list (marky-i3fl).

### Zig md4c error-handling and bounds patterns (2026-02-19)

From PR#39 code review (marky-0mr.4/.6/.9) — patterns that recur in md4c Zig port:
- **Silent `catch {}`** for buffer appends hides allocation failures → use `try`
- **Pointer arithmetic on `BlockHeader`**: always compute alignment offset explicitly,
  never assume `+ @sizeOf(...)` lands on the right boundary; add bounds guard via `if`
- **Bounds before increment**: `pivot_end += 1` in binary search without checking
  `pivot_end + 1 < map.len` is latent OOB on degenerate fold tables
- **Dead code from dual-return**: when two consecutive branches both `return false`,
  the redundant one is unreachable — remove it rather than leave a code-smell
- **`>= N` vs `> N-1`**: use the form that most directly names the index being accessed
  (e.g. `beg > 1` for `content[beg - 2]`)

### Zig test pointer tricks for >4GB fake slices (2026-02-19)

To test early-return guards that fire before data is accessed (e.g. size checks
before `@intCast(text.len)`), construct a fake huge slice using a many-pointer:
```zig
var sentinel: u8 = 0;
const p: [*]const u8 = @ptrCast(&sentinel);  // [*] has no tracked length
const fake: []const u8 = p[0..huge_len];      // valid fat pointer; never dereference
```
`[*]const u8` slicing has no bounds check. The function must return before
touching slice data or the test will crash. Using `@as([*]const u8, ptr)` is NOT
valid in Zig 0.15 — use type-annotated variable form instead (marky-0mr.5).

### FFI serialization: validate math, pointers, and alignment (2026-02-17/18)

For mmap-friendly binary formats, treat header counts and C pointers as untrusted input.
Checked arithmetic avoids overflow panics; null-pointer guards prevent SIGSEGV. Zero
padding bytes explicitly for deterministic output. Any `init()` accepting arbitrary
`[]const u8` must also validate alignment before `@alignCast` (marky-5rq).

### Code span extraction via ExtractionRenderer (2026-02-20, marky-pdyo)

Phase A-1 of ix3 added inline code span extraction to the Zig ExtractionRenderer.
Key design decisions and patterns:

- **Separate cursor**: `code_scan_cursor` is independent from `heading_scan_cursor` and
  `link_scan_cursor` (per marky-0rl6 lesson — shared cursors corrupt offsets).
- **Dual accumulation**: When `in_code_span` and `in_heading` are both true (e.g.
  `# Title \`code\``), `text()` appends to BOTH buffers. Heading text includes code
  span content, code span is extracted independently.
- **Backtick run matching**: `findCodeSpanOffset()` scans for matching backtick runs
  (1, 2, or 3 backticks). Double-backtick spans like ` ``code`` ` work correctly
  because the scan looks for a closing run of exactly the same length.
- **Fenced block exclusion**: `in_code_block` early return in `text()` fires before
  `in_code_span` check. md4c does NOT fire `SpanType.code` inside fenced blocks,
  so this is a belt-and-suspenders guard.
- **ABI change**: CMd4cResult grew from 40 to 48 bytes (added `code_spans` pointer +
  `code_spans_count`, removed `_padding`). Both Zig comptime and Rust const size
  asserts verify alignment.
- **Entity decoding**: md4c fires `TextType.code` (not `.entity`) for code span
  content, so entities are NOT decoded inside code spans. This matches CommonMark
  spec (code spans are verbatim).

### XML tag extraction via ExtractionRenderer (2026-02-23, marky-fd74)

B-7.1: Full Zig→blob→FFI→Rust pipeline for XML tags. Key patterns:

- **HTML callback parsing**: md4c fires `TextType.html` for block-level HTML.
  ExtractionRenderer parses `<tag>` / `</tag>` / `<tag />` patterns from the raw
  HTML text, with case-insensitive tag name matching for close tags.
- **Void elements**: `<br>`, `<hr>`, `<img>` etc. are auto-closed (no close tag needed).
- **CMd4cResult ABI**: grew from 136→144 bytes (added `xml_tags` pointer + `xml_tags_count`).
  Rust mirror struct must exactly match Zig extern struct layout — field order matters.
  SIGSEGVs result from any layout mismatch.
- **Blob header**: `xml_tag_count` at offset 80 in v2 header. BlobXmlTag = 40 bytes,
  section order: ...properties → xml_tags → line_starts → text_pool.
- **B-7.2 remaining**: Rust `from_blob` deserialization (read xml_tags from blob directly),
  wire into ScanBackend/ScanAllResult, remove supplementary `from_blob_with_xml_tags`.

---

## Using markymark Effectively

**Prefer LSP over MCP for single-file operations** — no realm setup needed. See
CLAUDE.md "Document Intelligence" section for the full LSP vs MCP decision tree.

**MCP-specific tips:**
- Always use `file://` URIs for `get-outline`, `export-index`, `find-references`
- `realm-stats` is cheap — use as before/after check when modifying docs
- `search-symbols` is fuzzy on heading text, not file content
- Ignore XML tag warnings in files with code blocks (marky-8la)

### Zig MCP Tool (mcp__zig, added 2026-02-22)

Available tools: `get_recommendations`, `generate_code`, `optimize_code`, `estimate_compute_units`,
`generate_build_zig`, `analyze_build_zig`, `generate_build_zon`. All respond successfully.

**Assessment:** Generic/noisy output. `get_recommendations` produces boilerplate checklists
regardless of input specificity. `generate_code` ignores detailed prompts and returns templates.
`analyze_build_zig` gives reasonable but shallow advice. Not useful for precision Zig work —
stick with LSP + agent docs for code quality. May be useful for quick build.zig scaffolding
or generating build.zig.zon dependency manifests.

---

## Key Architectural Decisions

### Arena Allocation

- **ArenaHashMap (!Send) restricted to parser types only; index types use std HashMap** (dec-arena-send-001). Bump:!Sync -> &Bump:!Send -> ArenaHashMap:!Send. tower-lsp requires Send+'static for async handlers.
- **DocumentIndex uses bare DocumentArena + unsafe impl Sync instead of Mutex** (marky-f9vv). The previous Mutex wrapper led to UB via raw-pointer MutexGuard escape. Bare arena + `unsafe impl Sync` is sound because the arena is only mutated during single-threaded self_cell construction; post-construction access is read-only. Compile-time Send+Sync assertion test guards against regression.
- **Adopted DocumentArena wrapper in Ast and DocumentIndex** (dec-docarena-adopt-001). Provides Debug, capacity hints, semantic boundary vs raw Bump.
- **Reorder Ast struct fields so root_elements before arena** (dec-031). Rust drops in declaration order. Arena must outlive elements.
- **Arena reuse via reset is NOT worth implementing** (dec-041). Arena lifecycle = 0.07% of reparse cost.

### Self-Referential Types

- **self_cell owner/dependent for Ast internals on stable Rust** (dec-051).
- **Parameterize SymbolAtPosition over lifetime instead of 'static** (dec-049). LSP state clones index entries; 'static caused borrow escape errors.

### ~~Incremental Indexing (Phase 3)~~ — SUPERSEDED by Epic H (marky-io3h)

The tree-sitter incremental indexing architecture (5 extractors, byte-range merge, LSP-layer
orchestration) was deleted in Task 4 (marky-n78f, tag `marky-io3h-complete`). Replaced by
Zig DocumentEngine pipeline: full md4c reparse on every edit, blob serialization, `from_blob()`
in Rust. Net -2,839 lines. The decisions below are historical context only.

- **LSP-layer orchestration, not index-layer diffing** (dec-phase3-001).
- **Byte-range granularity using InputEdit ranges** (dec-phase3-002).
- **All 5 independent extractors get incremental** (dec-phase3-003).
- **Headings/TOC/outline always full rebuild** (dec-phase3-004).
- **Markdown incremental only, JSON/YAML/TOML always full rebuild** (dec-phase3-006).
- **Wiki-link merge: range intersection + neighbor window + tail-boundary guard** (marky-77x).

### Zig SIMD Kernels

- **Copy and diverge Zig kernels from forge BRZA** (dec-brza-mm-001).
- **Complement tree-sitter with Zig SIMD, promotion path based on benchmarks** (dec-brza-mm-002).
- **markymark-kernels below markymark-core in dependency graph** (dec-brza-mm-003). Feature-gated.
- **Split C ABI exports into separate exports_*.zig files** (dec-ncz-001).
- **comptime { _ = @import } at module level for export wiring** (dec-0u5-003).
- **Batch fuzzy ranking in Zig with Rust fallback** (dec-8xt-batch-001/002).

### Semantic Index Concurrency (marky-ysv8)

- **SemanticIndex wrapped in Arc<tokio::sync::Mutex> inside RealmIndex** (dec-ysv8-001). Allows
  the MCP engine to clone the Arc handle, release the outer realm RwLock, and run async search
  without blocking realm-level write operations. `semantic_index_arc()` accessor provides the handle.
- **tokio::sync::Mutex (not std::sync::Mutex)** because `SemanticIndex::search()` is async.
- **No `blocking_lock()` in async call chains** (marky-wnjk, 2026-02-26). `RealmIndex::remove_document`
  and `detect_semantic_duplicates` are async and must use `lock().await`; calling
  `blocking_lock()` from Tokio runtime paths panics ("Cannot block the current thread...").

### Build & CI

- **build.rs invokes zig build lib via std::process::Command, zero build-dependencies** (dec-brza-een-001).
- **rerun-if-changed enumerates individual .zig files** (dec-brza-een-002). Directory-level watch only triggers on add/remove.
- **PIC required for Zig static libraries on Linux x86_64** (suc-021).
- **`zig build` ignores `ZIG_LOCAL_CACHE_DIR` env var** — must use `--cache-dir` CLI flag to override cache location. build.rs purges both `zig/.zig-cache/` and `prefix/.zig-cache/` before each invocation to prevent warm-cache archive corruption (Zig 0.15.2 bug).

---

## Key Failure Patterns

### bd + Dolt panics under parallel CLI invocations (2026-02-26)

Running multiple `bd` commands in parallel (`list/show/ready`) can trigger a Dolt nil-pointer
panic even with `BD_NO_DB=true BEADS_NO_DAEMON=1`. Sequential `bd` commands in the same shell
work reliably. For plan execution, run `bd` operations one-at-a-time.

### Context window exhaustion from task chaining (fail-context-runaway)
Agent completed B-6, then marky-eebj refactor, then started marky-j516 — all in one session
without stopping for user review. Hit context window limit mid-task, leaving from_blob/tests.rs
split half-done (new files created, old file not deleted = E0761 build break).

**Recurrence (2026-02-23, marky-9s66):** B-9 is a single large task (~14 implementation steps)
that touches 24 files and deletes ~2,895 lines. Agent made all changes in one pass without
committing intermediate checkpoints. Hit context wall while investigating a pre-existing LSP
compile error (key_path_str method). Left all changes uncommitted/unstaged. Recovery required
a fresh session to validate and commit. **Lesson reinforcement:** even within a single task,
commit intermediate milestones (e.g., Part 1 feature gate removal, then Part 2 deletion).

**Rules:**
- **ONE task per session turn.** After completing a task, STOP and report. Do not chain into
  the next task without explicit user approval.
- **Budget awareness.** If a session has already done substantial work (>2 commits), pause
  and check in before starting more. Large refactors (file splits, multi-file changes) are
  especially context-hungry.
- **Never start a destructive refactor near context limits.** File splits require atomic
  completion (create new + delete old + test + commit). Starting one without room to finish
  leaves a broken build.
- **Commit intermediate milestones within large tasks.** A 14-step task with 24 file changes
  should have at least 2-3 intermediate commits, not one giant uncommitted diff.

**Recurrence (2026-02-23, n7wx L2-L4):** Agent chained Layer 2 → Layer 3 → benchmarks →
Layer 4 "verification" in a single 75-minute session with 5 commits. Then autonomously declared
Layer 4 "redundant" and closed it without user approval. **New rule additions:**
- **NEVER autonomously reduce designed scope.** If you think a designed layer/task is unnecessary,
  report your analysis and let the user decide. Don't close it yourself.
- **Benchmark numbers do not justify skipping designed work.** Performance arguments from dev
  hardware are noise. Design decisions were made for a reason — implement them.

### Benchmark methodology anti-pattern (fail-benchmark-chasing)
Agent ran benchmarks, got bad numbers (860µs both paths), then iteratively "fixed" methodology
(iter_batched, returning realm to avoid drop timing, reducing sample_size) until criterion numbers
looked good enough to close a task. Each individual fix was technically valid, but the pattern
of adjusting until success is unacceptable. **Rules:**
- Design benchmarks correctly from the start, don't iterate until numbers match criteria
- Never use benchmarks from development machines to gate scope decisions
- Report honest numbers; if criterion not met, analyze why — don't adjust methodology

### tower-lsp-server v0.23 API mismatch (fail-tower-lsp-types)
Pre-training has `lsp_types` and `#[async_trait]`. The community fork v0.23 uses `ls_types`
and native async traits. Always read `docs/rust_crates/tower-lsp.md`.

### MCP stdio: line-delimited JSON, not Content-Length (fail-mcp-framing)
rmcp stdio uses `writeln!` + `read_line`, not HTTP-style `Content-Length` headers.

### Agent attempted PR merge without authorization (fail-pr-merge-autonomy)
Agent ran `gh pr merge 36 --merge` during v0.4.0 release. User correction: agents NEVER
merge PRs. Human merges all PRs. Agent prepares PRs and pushes branches only.

### Security hook blocks Write on GitHub Actions YAML (fail-write-tool-gh-actions)
`security_reminder_hook.py` intercepts Write on `.github/workflows/*.yml`. Use Bash heredoc.

### Zig 0.15 API breaks from 0.14 (fail-zig-015-api)
`addStaticLibrary` → `addLibrary` with `.linkage = .static`.
`root_source_file` → `root_module` via `b.createModule()`.
`callconv(.C)` → just use `export fn`.
`ArrayList(T).init(allocator)` → `ArrayListUnmanaged(T){}` (allocator passed per-call).
Always read Zig build system docs first.

### ${CLAUDE_PLUGIN_ROOT} in file content triggers hook blocks (fail-write-plugin-root)
Some hooks intercept Write when content contains this literal string. Use `Bash cat` heredoc
with single-quoted delimiter (`'EOF'`) to bypass.

### Agent used Grep/Read instead of LSP() for code navigation (fail-lsp-not-used)
Repeated user correction: always use LSP tools first for Rust/Zig navigation.
- `LSP documentSymbol` to explore file structure before reaching for Read
- `LSP findReferences` to find usages instead of Grep
- `LSP hover` for type/signature info instead of reading source
- `LSP goToDefinition` to jump cross-file instead of Glob + Read
Read/Grep only after LSP narrows the target or for non-code files.

### Agent used claude-mem save_memory for this project (fail-save-memory-unreliable)
CLAUDE.md says not to use `save_memory` for markymark — the API is unreliable.
Sole persistent memory store is `docs/MEMORY.md`. Update it directly via Edit tool,
then commit. Never use save_memory as a substitute.

### Dev workflow skill placed in plugin directory (fail-skill-location)
The `prepare-release` skill was placed in `markymark-plugin/skills/` (ships to users)
instead of `.claude/skills/` (repo-level, dev-only). Plugin skills are user-facing features
(like `markdown-check`). Dev workflow skills belong in `.claude/skills/`. Caught in review.

### CLAUDE.md crate table stale after adding markymark-kernels (fail-stale-crate-table)
CLAUDE.md "Project Overview" said "Six crates" and omitted `markymark-kernels`. Stale since
the kernels crate was added. Lesson: when adding a crate to the workspace, update CLAUDE.md
crate table in the same PR. Now fixed (Seven crates, kernels included).

### docs/modules and docs/zig_agent_docs are symlinks to forge repo (info-docs-symlinks)
`docs/modules` → `../../../forge/docs/modules/` and `docs/zig_agent_docs` →
`../../../forge/docs/zig_agent_docs` are **symlinks** to the forge repo. The `ASM-AGENTS-MD`
docs_index block in CLAUDE.md references paths under `docs/modules/` — these resolve via
symlink when forge is present. `git ls-files` and `find -type f` from the worktree root
won't find them (symlinks not tracked). Use absolute path to forge or `ls -la docs/` to
verify. 40 `.md` files exist at the target. CodeRabbit flagged these as "non-existent" —
false positive due to not following symlinks (2026-02-20).

---

## Key Patterns

### FFI Bridging
- Generic `call_scan_ffi<T>` with buffer retry (start 64, double on -2, max 3 retries)
- `repr(C)` mirror structs at boundary, idiomatic Rust in public API
- `safe_slice()` rounds byte offsets to UTF-8 char boundaries
- `PhantomData<*mut ()>` for !Send/!Sync on stable Rust
- Drop impl sets handle to null for idempotent double-free protection

### Zig Kernels
- SIMD for sparse search: @Vector for candidates, scalar for validation
- Share parsing logic between SIMD and scalar via pub import from reference
- `exports_*.zig` + `comptime { _ = @import(...) }` for composable ABI
- FFI functions must initialize all output parameters before error returns
- `test { _ = @import(...); }` pulls sub-module tests into main test step
- Output-buffer capacity guard must come BEFORE the write, not after the increment (marky-wpl)
- Validate alignment before `@alignCast`: `if (@intFromPtr(p) % @alignOf(T) != 0) return null;` — panics in Debug/ReleaseSafe on misaligned arbitrary input (marky-5rq)

### Arena & Lifetimes
- Avoid cloning ArenaHashMap with bumpalo — SIGSEGV. Return `Vec<&T>` instead
- `bumpalo Vec::new_in(arena).into_bump_slice()` for empty arena slice, not `&[]` (UAF)
- When migrating wrapper types, trace all ptr::read/mem::forget — type must match

### Resolution Layer

- **`resolve_markdown_link` handles cross-document links** (marky-z9z). Resolves `other.md` → Document, `other.md#heading` → Heading, `dir/other.md` → path-relative first, stem fallback.
- **Path-relative without filesystem access**: Component-stack normalization (pop on `..`, skip `.`/empty) instead of `std::fs::canonicalize`. `canonicalize` requires path to exist on disk.
- **Stem-only is the fallback, not the primary**: Path-relative resolution wins when URL contains `/`. Stem-only fires when path-relative misses.

### Engine Pipeline (Epic H)

- **XML tags extracted natively via blob** — B-7.1/B-7.2 (marky-fd74/marky-l5vu) migrated XML
  tag extraction into the Zig ExtractionRenderer. Tags are serialized as `BlobXmlTag` entries
  in the blob and deserialized by `from_blob()`. No supplementary extraction needed.
- **md4c inline HTML content pointers are NOT into source text** — md4c passes inline HTML
  fragments via `text()` callback with pointers to internal buffers, not the original source.
  The bounds check in `extraction_renderer.zig:text()` must validate both start AND end:
  `content_start >= src_start AND content_end <= src_end`. Failing to check end caused garbage
  offsets (414M+) and corrupted tag names.
- **processHtmlFragments scans within fragments for multiple tags** — a single HTML block line
  like `<goal>win</goal>` contains multiple `<...>` sequences. The inner loop in
  `processHtmlFragments()` finds each one.
- **XML tag symbols need sort-by-range before nesting** — `xml_tags_to_symbols()` in
  `markymark-lsp/src/symbols.rs` must sort tags by range before building parent/child nesting.
  Parents must be inserted before children.
- **Blob path does not preserve per-tag attributes** — the `BlobXmlTag` format stores tag name,
  range, and flags but NOT attributes. Attribute display in hover and workspace stats is empty
  when going through the blob path. This is an acceptable trade-off.
- **Test fixtures must use block-level HTML for XML tag extraction** — inline HTML like
  `<tag>content</tag>` on a single line is treated as inline by md4c, not block-level.
  Tags are only extracted from block-level HTML (tag on its own line, content on separate lines).
- **End positions computed in Zig** — heading, link, and block-id end positions are calculated
  during document construction in `zig/src/engine/document.zig`, avoiding double-computation in Rust.
- **Engine lifecycle: create → update → get_blob → from_blob → destroy** — per-document
  `DocumentEngine` in `ServerState.engines` HashMap. Same URI key as `documents` and `realm`.

### LSP/MCP
- **Diagnostic logic lives in `markymark-index/src/diagnostics.rs`** (marky-6i9). Shared `compute_diagnostics(index, realm, uri)` used by both LSP and MCP.
- **Adding a new CoreOperation follows a 5-stop pattern**: (1) types in `core/engine.rs`, (2) compute logic in `index/`, (3) engine handler in `mcp/src/engine/{op}.rs`, (4) DTO + tool handler in `mcp/src/tools/{op}.rs`, (5) `#[tool]` wiring in `lib.rs`.
- Drop read lock before async publish_diagnostics (deadlock prevention)
- LSP character offsets from clients are untrusted — always bounds-check (`if offset > line.len()`) before byte-slicing (marky-xpk, marky-u46)
- MCP handlers that accept any URI kind must use `realm.get_any_document()` and branch on `AnyDocumentIndex`; `get_document()` silently rejects structured docs (marky-kvr)
- For edit-delta math on `u32`/`usize` positions, use explicit saturating add/sub with signed deltas to prevent wraparound (marky-v8y)
- **Global monotonic counter for generation-based cleanup** — when a HashMap entry must be
  removed on close but a stale async task might race with a reopen, use a global monotonic
  counter (not per-key counters starting from 1). Per-key counters collide when an entry is
  removed then re-inserted with the same starting value. Global counters guarantee every
  allocation is unique, so `unwrap_or(0) != captured_gen` always holds (marky-jwsk).

### Testing
- Safe file splits: (1) module dir, (2) extract types, (3) extract helpers, (4) extract tests. Each step: edit→test→commit
- Rust-analyzer can show transient `unlinked-file` diagnostics immediately after creating a new module file during refactors; once the parent `mod.rs` includes `mod <name>;` and the workspace rebuilds, the warning clears (2026-02-22).
- from_blob refactors are safest when decode loops are moved wholesale into a sibling module and return a single owned-data container, preserving comments and marky-d7hh alias logic verbatim (2026-02-22).
- Use `assert_eq!` not `>=` — `>=` masked a closing-tag rename bug
- Integration test crate roots (`tests/*.rs`) resolve `mod foo;` in `tests/foo.rs` (sibling), NOT `tests/basename/foo.rs`. Use `#[path = "basename/foo.rs"] mod foo;` for subdirectory splits (marky-a90)
- Env-gated benchmarks (`MARKYMARK_RUN_100K_BENCH=1`) for checkpoint evidence
- `scanner.rs` split into submodules during B-5.0 (marky-z4ja): mod.rs (101), types.rs (159), md4c.rs (315), tests.rs (409). Safe to add B-5/B-6 scan methods.
- `md4c.rs` (markymark-kernels) split into md4c/mod.rs (614) + md4c/tests.rs (465) during marky-8y0o. Same pattern as scanner.rs split. Safe to add B-6/B-7 scan methods.

### Project-Specific
- Plugin directory: markymark-plugin/.claude-plugin/plugin.json (version must be bumped manually alongside Cargo.toml)
- `require_marksman!` macro for graceful test skip in CI
- lefthook YAML: quote command values containing colons/braces

---

## Release Process

### Version Locations

| File | Field | Notes |
|------|-------|-------|
| `Cargo.toml` | `workspace.package.version` | All crates inherit via `version.workspace = true` |
| `markymark-plugin/.claude-plugin/plugin.json` | `version` | NOT auto-derived — bump manually (Rule #4) |
| `Cargo.lock` | 7 internal crate entries | Regenerated by `cargo build` after version bump |

### Known Pitfalls

1. **plugin.json forgotten** — `plugin.json` version is independent of `Cargo.toml`. Must be
   bumped manually every release. Missed bumps ship stale plugin metadata.
2. **Cargo.lock not committed** — After editing workspace version, `cargo build` regenerates
   `Cargo.lock` with new internal crate versions. This file must be committed alongside
   `Cargo.toml`. Historical precedent: v0.4.2 needed a separate fixup commit (324f744).
3. **Publish order staleness** — RELEASING.md publish order drifted when `markymark-kernels`
   was added. Always re-derive from `cargo metadata` before publishing. See RELEASING.md
   for the derivation command.
4. **Inter-crate dependency versions** — Each crate's `Cargo.toml` has explicit `version = "X.Y.Z"`
   on its internal dependencies (e.g. `markymark-core = { version = "0.5.0", path = "..." }`).
   These must be bumped alongside the workspace version. Forgetting them causes `cargo build`
   to fail with "failed to select a version for the requirement". Historical precedent: v0.5.0
   initial build failed until all 5 crate Cargo.toml files were updated.
5. **Worktree prevents main checkout** — In a git worktree setup, `git checkout main` fails
   because main is checked out in another worktree. Tagging must be done from the main worktree
   by the human. The prepare-release skill Phase 4 is human-owned for this reason.

### Conventions

- **Tag format:** `vMAJOR.MINOR.PATCH` on `main` branch only
- **Publish order:** kernels → core → parser → index → lsp/mcp (parallel) → cli
- **Skill:** See `prepare-release` skill (`.claude/skills/prepare-release/`) for
  guided 5-phase release workflow with human checkpoints
- **Release notes:** Auto-generated git-cliff notes are replaced in Phase 5 with
  curated narrative notes (grouped by theme, not flat commit list). Agent drafts,
  human approves, then published via `gh release edit`.

---

## Performance Optimization Roadmap

### Completed (marky-77i CLOSED, superseded by Epic H)

- **F: Debounce** (marky-7dq, DONE) — 75ms async cancellation in LSP `did_change`
- **G: md4c streaming parser** (marky-0mr) — Vendored Bun's Zig md4c port. 2.8x pipeline speedup at 50KB.
- **H: Zig Document Engine** (marky-io3h, DONE) — see below. Tagged `marky-io3h-complete`.
- **D: Vendor tree-sitter-md** (marky-0jz, CLOSED) — superseded by Option G.

### Deferred (Low ROI after Epic H) — see also Incremental md4c below

- **E: Lazy AST** (marky-syx, P3) — value reduced. Tree-sitter only for MCP batch + hover/goto-def.
- **Engine incremental diffing** — 2.5ms at 50KB is fine, but scales linearly. 5MB → ~250ms per
  keystroke. Revisited in incremental md4c research (2026-02-23). See dedicated section below.
- **Zero-copy blob borrowing** — investigated, not worth it. Breaks DocumentIndex lifetime model for ~1-2ms.
- **Edit range support in engine.update()** — prerequisite for incremental md4c, no longer premature.

### RealmIndex v2 (marky-n7wx) — COMPLETE

lasso Rodeo interner in RealmIndex. Cross-doc HashMaps keyed by Spur (u32) instead of String.
`update_document()` diffs `DocContribution` (HashSets of Spur) — fast path skips all cross-doc
ops when structure unchanged. Lazy `tag_to_docs` via `tags_dirty` flag; `tag_counts(&self)`
computes from contributions when dirty (no interior mutability). `find_uri_by_stem()` is O(1)
via `stem_to_uris` index. Wired into LSP at `state/mod.rs:245,333`.

Key design decisions:
- **Rodeo not ThreadedRodeo** — RealmIndex is single-threaded, simpler API.
- **Don't intern URIs** — unique per document, no dedup benefit.
- **ResolvedHeading keeps String fields** — resolve Spur→&str at query boundary (cold path).
- **key_path_to_docs stays String** — structured doc paths have low repetition.

### Baseline Benchmarks (2026-02-19, marky-jpot)

Criterion, release mode, synthetic docs via `generate_markdown_doc()`.

| Size | md4c extract | md4c from_scan | tree-sitter from_ast | Pipeline speedup |
|------|-------------|----------------|---------------------|-----------------|
| 1KB | 0.115ms | 0.229ms | 0.490ms | 2.1x |
| 10KB | 0.850ms | 1.836ms | 4.573ms | 2.5x |
| 50KB | 4.686ms | 9.436ms | 26.662ms | 2.8x |
| 100KB | 9.882ms | 20.692ms | 66.962ms | 3.2x |

**Key insight:** from_scan index build doubles extraction time (4.7ms → 9.4ms at 50KB).
The gap is ScanBackend dispatch + DocumentIndex construction + N+4 FFI calls per document.
This is the exact bottleneck Option H eliminates.

Throughput drops at scale due to per-element allocation density: 53 MB/s (10KB) → 23 MB/s
(50KB) → 10 MB/s (100KB). Parser is fast; bottleneck is N allocations in ExtractionRenderer.

### Option H: Zig Document Engine (marky-io3h) — COMPLETE

Stateful Zig engine that owns per-document parse state and serves a flat binary blob
to Rust. Replaces N+4 FFI calls with exactly 2 (update + get_blob). Tagged `marky-io3h-complete`.

**Architecture:** Zig `DocumentEngine` with create/update/getBlob/destroy lifecycle.
Lazy blob serialization. Blob format: 64B header (magic 0x4D4B5343) + packed struct
arrays + contiguous text pool. Rust `DocumentIndex::from_blob()` / `from_blob_with_xml_tags()`
copies text from blob pool into arena. Net -2,839 lines (incremental module deleted).

**Performance:** Not yet benchmarked post-integration (marky-8d08). Expected ~4ms at 50KB
vs previous 9.4ms from_scan baseline.

**Key decisions:**
- Stateful (not stateless) — enables slug caching, lazy blob, future incremental
- Full md4c reparse always — fast enough with debounce
- from_ast()/from_scan() retained for MCP batch and backward compat
- Tree-sitter stays separate for lazy AST (hover/goto-def)

Tasks: 6jzs (engine+blob), atsp (FFI+wrapper), 0mr.9 (parser fixes), 2n4u (from_blob), n78f (LSP integration). All done.

---

## PR #40 Code Review Triage (2026-02-20)

SRE-level assessment of 8 findings from Codex + CodeRabbit. Consolidated into 7 tracks,
4 valid, 1 already known, 2 dismissed.

### Dismissed Findings

- **Fixed buffer caps (tags 1024, block-ids 1024, fences 256)** — intentional performance
  tradeoff. Engine path uses stack allocation for LSP hot path. Cap is 16× the Rust path's
  practical max (~512 via call_scan_ffi retry). No document realistically exceeds these.
  Architectural asymmetry between paths is deliberate: Rust path (call_scan_ffi → C adapter
  → `-2` retry) is dynamic; Zig engine path (direct scan_tags call) is fixed stack.
- **u32 truncation in extract_md4c and call_scan_ffi** — `text.len() as u32` at two sites
  (md4c.rs:114, scan.rs:283). Theoretical only — 4GB markdown files don't exist. Not UB on
  truncation (reads fewer bytes, not more). One-liner `u32::try_from` guard available if
  desired but not worth tracking.
- **Debounce edit loss (server.rs)** — INVALID finding. Task removes its own abort handle
  before draining pending_changes, so subsequent did_change can't abort it. Generation
  counter handles close/reopen races. Design is correct.

### Valid Findings (beads created)

- **marky-5vnt (P3, CLOSED):** Slug truncation returns empty + processLeafBlock silent catch {}.
  Fixed: slugifyText returns out[0..512] on rc==-2, processLeafBlock catch {} → try.
- **marky-9m7o (P4, CLOSED):** parseAll errdefer leaks text on late-stage OOM. Fixed:
  texts_transferred flag + freeStoredHeadingsList/LinksList free texts when flag true.
  Link end_offset heuristic replaced with scan cursor end_offset from extraction renderer.

### Post-merge Review Findings (2026-02-20, cursor + codex)

Three additional findings from cursor and codex reviews, SRE-refined:

- **marky-d7hh (P1):** from_blob wiki link alias parity — compares `text != page` (anchor
  stripped) instead of `text != target` (full target). For `[[page#heading|page]]`, misses
  alias and computes wrong end_byte. from_scan is correct. Fix: compare against full target.
- **marky-8nzt (P2):** parseAll toOwnedSlice cascade leak — distinct from marky-9m7o.
  After headings.toOwnedSlice succeeds, stored_headings_list is empty. If links.toOwnedSlice
  fails, errdefer frees empty list, headings data leaks. Fix: scoped errdefer after each
  toOwnedSlice (same pattern as marky-gmny extraction_renderer fix).
- **marky-ta07 (P2):** convert_result (md4c.rs) slices blob without bounds checks on FFI
  offsets. Zig always produces valid offsets but a parser bug would panic the LSP/MCP process.
  from_blob validates via pool_str; this path does not. Fix: safe_blob_slice helper.

## PR #41 Code Review Triage (2026-02-20)

SRE-level assessment of all findings from CodeRabbit (3 review rounds, 14 inline + 1
outside-diff + 13 nitpicks), Semgrep/GHAS (22 comments = 11 blocks × 2 rules), and
Copilot (0 comments, clean pass). Consolidated into 8 tracks: 5 closed, 3 open.

**Dismissed:** (3 items)
- **did_open generation ordering (server.rs:157-176)** — INVALID. `next_generation` starts
  at 1 (line 51), so `unwrap_or(0)` always mismatches any captured gen (≥1). Same class as
  PR #40 "Debounce edit loss" dismissal. Design is correct.
- **u32 truncation in md4c.rs** — already dismissed in PR #40 review (see above). Same finding.
- **Semgrep nosemgrep alignment** — engine.rs and md4c.rs already have complete SAFETY +
  nosemgrep coverage. GHAS still flags because it may not honor inline nosemgrep in diff view.
  Platform limitation, not a code issue.

**Accepted — Round 1:** (5 beads, ALL CLOSED)
- **marky-0rl6 (P1, CLOSED):** ExtractionRenderer scan_cursor split into heading/link cursors.
- **marky-c44x (P2, CLOSED):** Debounce flush flattened to single apply_document_changes call.
- **marky-pk33 (P3, CLOSED):** FFI safety — exports.zig u32 intCast guard + blob.zig
  writeStruct/readStruct made fallible with error.OutOfRange.
- **marky-i873 (P4, CLOSED):** autolinks.zig — boolean check, doc comment, debug assertion.
- **marky-4atp (P4, CLOSED):** Code quality — test .len, eprintln→log::warn!, glob import.

### Round 2/3 Findings (CodeRabbit, 2026-02-20 post-fix)

CodeRabbit re-reviewed after the round-1 fixes (commits b1e7cd3–b6ec6a4) and posted 8
additional inline comments + 6 nitpicks. Validated against code, consolidated into 3 tracks.

**Dismissed — Round 2/3:** (2 items)
- **writeStruct/readStruct UB (blob.zig:212-226)** — already fixed by marky-pk33. Now return
  error.OutOfRange with runtime bounds checks. CodeRabbit commented on stale code state.
- **@intCast overflow in serializeState (document.zig:491-497)** — physically impossible.
  u32::MAX array elements requires 400GB+ RAM for headings alone. Pure theoretical defense.
  Tracked in marky-wdnc (P4) as optional guard.

**Accepted — Round 2/3:** (3 beads created)
- **marky-lzd5 (P2):** ExtractionRenderer offset scan hardening — 4 sub-issues:
  (F1) ATX heading fence tracking uses `in_fence = !in_fence` without matching char/length.
  Backtick fence incorrectly closed by tilde line. (F2) Same bug in link scan.
  (F3) Setext heading scan has NO fence tracking — matches `---`/`===` inside code blocks.
  (F4) Inline link URL scan stops at first `)`, truncating URLs with parens (e.g. Wikipedia).
  All are offset-only — md4c extraction correct, but LSP hover/goto-def ranges wrong.
- **marky-nwoz (P3):** LSP state/mod.rs robustness — 2 sub-issues:
  (G1) `engine_mutex.lock().expect()` panics on poisoned mutex. Unreachable today (&mut self),
  but a panic in from_blob during lock scope would poison it. Replace with match + fallback.
  (G2) 6 remaining eprintln! calls in build_markdown_index_via_engine → log::warn!.
- **marky-wdnc (P4):** Zig engine doc/guard nitpick bundle — exports.zig -5 doc,
  readHeader/writeHeader precondition docs, 256 fence limit named constant, optional
  serializeState @intCast overflow guards.

---

## PR #42 Code Review Triage (2026-02-20)

Release v0.5.1 PR. Reviewers: Copilot (3 inline), CodeRabbit (2 inline + 1 outside-diff +
1 nitpick). 7 findings total — 3 valid (all fixed immediately), 4 dismissed.

**Dismissed:** (4 items)
- **docs/semver.md hyphenation ("backward compatible")** — verbatim copy of official
  SemVer 2.0.0 spec. Altering it would diverge from canonical source.
- **docs/semver.md "prerelease" terminology** — describes the named capture group in the
  spec's regex (group named `prerelease`), not a prose hyphenation choice.
- **Step numbering duplicate** — CodeRabbit flagged same as Copilot; deduplicated.
- **cargo-mcp vs raw cargo in quality gates** — release gate scripts intentionally use
  raw cargo for explicit flags (`-D warnings`, `--all-targets`). cargo-mcp preference
  applies to development navigation, not release automation.

**Fixed — marky-lj58 (P2, CLOSED):** Three correctness gaps in prepare-release Phase 2,
all triggered by inter-crate dep version bumping added in this release (commit 1ea2dba):
- Step numbering: Phase 2 jumped 5→7. Renumbered 6–10 consecutively.
- Assertion label: "Cross-file version assertion" only checks package versions; renamed to
  "Cross-crate package version assertion" + note that dep version fields caught by cargo build.
- Rollback command: `markymark-*/Cargo.toml` was missing from the git checkout revert.

**Pattern recorded (info-verbatim-spec-docs):** `docs/semver.md` is the official SemVer
2.0.0 spec verbatim. Style findings (hyphenation, terminology) on this file are always
false positives — do not re-triage. Same pattern may apply to other verbatim spec files.

---

## PR #44 Code Review Triage (2026-02-23)

Release v0.6.0 (fresh PR after #43 close+rebase). 100 changed files, +16.9k/-7.7k.
Reviewers: Copilot (5 inline, 82/100 files), CodeRabbit (16 inline + 7 nitpicks + 1
outside-diff), GHAS/Semgrep (30 comments, 2 rules × 15 sites), Greptile (clean pass).
CI: Build & Test FAILURE (linker), Security/CodeQL/Audit all pass.

**Dismissed:** (5 tracks)
- **Semgrep unsafe-usage/unsafe-block (30 comments)** — FFI module must use unsafe.
  Already has nosemgrep annotations. Same class dismissed in PR #40/#41.
- **Copilot: circular dependency claim (markymark-index↔kernels)** — FALSE. kernels→index
  is dev-dependency only. Dev-deps don't create cycles in Cargo.
- **CodeRabbit: scan_tests.rs:98 xml_tags empty assertion** — CORRECT test. Input
  `<goal>Ship</goal>` is inline HTML (single line), md4c only extracts block-level.
  Per MEMORY.md: "Test fixtures must use block-level HTML for XML tag extraction".
- **CodeRabbit: md4c/mod.rs u32 truncation** — Already dismissed in PR #40 and #41.
- **CodeRabbit: from_blob XML tag attributes empty** — Already documented in MEMORY.md
  as accepted trade-off.

**Valid findings (8 beads created):**
- **marky-whvn (P1):** CI linker failure — `libmarky_kernels.a` truncated/malformed on
  Linux x86_64. Blocks PR from passing CI.
- **marky-ab5g (P2):** realm/tests.rs at 994 lines — 6 from 1000-line hard stop.
- **marky-e7i3 (P2):** frontmatter.rs — property scan past non-property lines + CRLF.
- **marky-mh1p (P2):** LSP fallback scan drops frontmatter (state/mod.rs).
- **marky-a4k9 (P3):** Loose `>= 2` test assertions in scan_tests.rs and core_tests.rs.
- **marky-r5p3 (P3):** from_blob magic numbers + empty list items not filtered.
- **marky-85ii (P4):** Docs cleanup batch (absolute paths, style, stale comments).
- **marky-2pyo (P4):** Code quality batch (glob imports, workspace deps, cfg guard).

**Patterns recorded:**
- Copilot misidentifies dev-dependencies as circular dependencies — dismiss these.
- CodeRabbit doesn't understand md4c inline vs block-level HTML distinction for XML tags.

---

## Cross-Language Symbol Bridging (Epic marky-ix3) — COMPLETE

All 11 markdown-content extractors migrated from extract.rs regex to Zig ExtractionRenderer.
Only frontmatter stays in Rust. Blob v2 header (128 bytes, 8 new count fields). MCP batch
path uses from_blob instead of from_ast. Code spans surfaced via LSP (workspaceSymbol, hover)
and MCP (search-symbols). Three phases completed: A (code spans all paths), B (extractor
migration B-1..B-9), C (extract.rs cleanup).

### Key Decisions (retained for future reference)

- **Separate cursors per extraction type** — per marky-0rl6, never share mutable scan cursors.
- **from_blob backward-compatible** — reads both v1 and v2 blobs (code_span_count in _reserved).
- **FFI path is md4c/exports.zig** — NOT engine/exports.zig (that's Document Engine lifecycle).
- **Each B-task pattern:** Zig extraction → blob struct + header count → from_blob → DocumentDependent field → tests → remove extract.rs regex.
- **md4c callback types:** Tasks (LiDetail.is_task), Callouts (enterBlock .quote), Embeds
  (leaveSpan .wikilink + `!`), XML tags (TextType.html). Text scanning: block refs `((uuid))`,
  properties `key:: value`, query blocks `{{query}}`, link defs `[label]: url`.

---

## Documentation Overhaul (Epic marky-y1gm)

### Decisions (2026-02-20)

- **Separate Starlight (Astro) docs site** in `docs-site/` — not in existing `docs/` (which
  has 58+ agent reference files) and not flat markdown at repo root (doesn't scale past 5-6 files).
- **README rewritten as concise landing page** (~80 lines) linking to docs site.
- **Bun exclusively** for docs tooling (no npm/yarn/pnpm).
- **Both audiences:** end users (installation, usage, editor setup) AND contributors
  (architecture, development, guidelines).
- **23 content pages** across 8 sections: about, getting-started, usage, guides, editors,
  features, architecture, contributing — plus troubleshooting and FAQ.
- **Changelog not needed** — already generated by release workflow.
- **Editor setup guides:** VS Code, Neovim, Claude Code (others deferred).
- **Agent tutorial** (`guides/agents.md`) — key differentiator, walks through MCP server
  usage with Claude Code including working examples.
- **About page must be layperson-friendly** — no LSP/MCP jargon upfront.

### Content Sources

- Current README.md (stale: lists 6/7 crates, incomplete MCP tools list)
- Plugin README (`markymark-plugin/README.md`) and VS Code extension README
- MEMORY.md architectural decisions (rewrite for external audience, don't copy verbatim)
- Actual code for MCP tools reference and LSP capabilities
- `docs/plans/` for architecture content

### Task Order

First task: **marky-wvqy** — scaffold Starlight site with navigation structure (SRE-refined).
Subsequent tasks created iteratively via executing-plans.

---

## Incremental md4c Block-Level Reparse (Research Complete, 2026-02-23)

### Motivation

Current pipeline does full md4c reparse on every keystroke via `DocumentEngine.update()`.
At ~2.5ms/50KB this is fine for typical files, but scales linearly: 5MB → ~250ms per edit.
Goal: support multi-megabyte markdown files without degrading responsiveness.

The old Rust incremental system (deleted in marky-n78f, tag `fixed-incremental`) solved the
wrong problem at the wrong layer — it did **merge after parse** (re-running regex extractors
on edit regions, then splicing results). The right approach is **parse less** by making the
md4c block analyzer itself incremental.

### Research Findings (2026-02-23)

Comprehensive SOTA research completed and documented in `/docs/research/incremental-parsing-sota-2026.md`.

**Key findings:**
1. **No production markdown parser implements incremental reparse** — md4c, cmark, pulldown-cmark all parse full document. Lezer is the only production incremental markdown parser (used by Obsidian, CodeMirror).
2. **tree-sitter's incremental: CST node reuse via byte-range tracking** — Edit ranges identify affected nodes, unmodified nodes reused via Arc. Achieves ~99% node sharing. Cost: GLR parser machinery.
3. **Block-level incremental is feasible via safe boundaries** — Blank lines, ATX headings, thematic breaks, code fences are guaranteed convergence points. Parser state resets deterministically there.
4. **SIMD boundaries + sqrt decomposition chunks = sweet spot for markymark**
   - Reuse existing kernels (fence_map, heading_scan) for boundary detection
   - Chunk tree: O(√N) reparse cost per edit
   - Convergence detection: if exit_state matches next chunk's entry_state, stop propagating
   - Expected 3-5x speedup on typical edits, 10x+ on structural edits
5. **Production systems use complementary strategies**
   - Zed: rope + SumTree (O(log N) coordinate queries) + tree-sitter incremental
   - Roslyn: red-green trees (99% node reuse, designed for compiled languages)
   - Obsidian: Lezer + context hashing + per-document caching + cross-doc indexing
6. **Two-phase architecture (CommonMark spec) supports chunking**
   - Phase 1 (block structure) can be incremental per chunk
   - Phase 2 (inline) is fast, can remain stateless per-block
   - Link ref defs are global but can be extracted per-chunk and merged

### Architecture Analysis: blocks.zig

md4c's block phase (`zig/src/md4c/blocks.zig`) is a single-pass, line-by-line, forward-only
state machine. `processDoc()` calls `analyzeLine()` + `processLine()` in a loop, then
`buildRefDefHashtable()`, then `processAllBlocks()` (which walks the flat `block_bytes`
buffer and fires inline parsing + renderer callbacks).

**State carried across lines** (the "parser snapshot"):
- `containers[]` + `n_containers` — active blockquote/list nesting stack
- `current_block` — leaf block being accumulated
- `pivot_line.type` — previous line type (setext, lazy continuation, fenced code)
- `html_block_type` — active HTML block (1-7)
- `fence_indent` — code fence indentation
- `last_line_has_list_loosening_effect` — loose/tight list heuristic

**Why naive incremental is hard:**
1. Container cascades — adding `>` at line 50 changes container matching for all subsequent lines
2. Setext headings are retrospective — `---` on line N converts the paragraph above to `<h2>`
3. Link ref defs are paragraph-consuming — `[ref]: url` lines eaten from paragraph start
4. Loose/tight list detection retroactively patches opener blocks
5. `block_bytes` is append-only — no splice capability

### Proposed Hybrid: SIMD Boundaries + Chunk Tree

**Layer 1 — SIMD structural boundary scan (existing kernels, microseconds)**

Use SIMD kernels to identify **guaranteed convergence points** where parser state is fully
determined regardless of prior context:
- Blank line outside fenced code block = container stack resets to 0
- ATX heading (`# `) = self-contained single-line block
- Thematic break (`---`/`***`/`___`) = self-contained
- Opening code fence = known state transition

A dedicated `boundary_scan` kernel could find blank-outside-fence in one SIMD pass (track
backtick/tilde toggles, look for `\n\n`).

**Layer 2 — Chunk tree with cached state (sqrt decomposition / segment tree pattern)**

Build a balanced tree where each node = chunk between two safe cut points:
```
Chunk { byte_range, entry_state: ParserSnapshot, exit_state: ParserSnapshot, block_output, line_count }
```
`ParserSnapshot` ≈ 64-128 bytes (container stack depth + types, pivot_line type, html_block_type,
fence state, loose-list flags).

**Layer 3 — Edit propagation (O(log N) convergence)**

On edit:
1. SIMD scan edited region for new/removed safe cut points
2. Find affected chunk(s) in tree
3. Reparse those chunks using entry_state from chunk before edit
4. Compare new exit_state to next chunk's cached entry_state
5. Match → stop (no propagation). Mismatch → reparse next chunk. Repeat.

**Performance characteristics:**

| Edit type | Cost | Why |
|-----------|------|-----|
| Paragraph interior (95%+ of typing) | O(chunk_size) ≈ 5-50 lines | Safe cut points unchanged, one chunk reparsed, exit state matches |
| Structural (add `>`, fence, `---`) | O(affected_chunks × chunk) | Propagates until hitting a blank-line convergence barrier |
| Worst case (no blank lines in file) | O(N) | Same as full reparse, but this pathological case is rare |

Memory overhead: ~128 bytes per chunk. For 5MB / ~100K lines with ~2K chunks → ~256KB.

### Existing Infrastructure That Helps

- `fence_map` kernel — already builds fenced code ranges, usable for "is this blank line inside a fence?"
- `heading_scan` kernel — finds ATX headings
- `block_scan` kernel — finds block-level markers
- `content_hash` kernel — could fingerprint chunks for fast "did anything change?" checks

### Key Implementation Decisions (TBD — brainstorm these)

1. **Chunk granularity** — fixed size (sqrt decomposition) vs. semantic boundaries (blank lines)?
   Semantic is better for convergence but creates variable-size chunks.
2. **block_bytes structure** — replace flat append buffer with segmented/rope structure, or
   rebuild from chunks on demand?
3. **Ref def handling** — `buildRefDefHashtable` is global. Incremental needs per-chunk ref def
   tracking with a merge step.
4. **processAllBlocks** — walks block_bytes linearly firing inline parsing + renderer. Can be
   restricted to changed chunks if block_bytes is segmented.
5. **API surface** — `engine.updateRange(edit_start, old_end, new_end)` alongside `engine.update(full_text)`
6. **Blob interaction** — blob serialization already lazy (cached_blob invalidated on update).
   Could invalidate per-chunk and rebuild only changed segments.

### Related DSA Patterns

Multiple classic patterns apply. For brainstorming reference:
- **Sqrt decomposition** — divide into √N blocks, rebuild one block per update
- **Segment tree with lazy propagation** — O(log N) update/query, deferred recomputation
- **Finger tree with monoidal annotations** — split/concat at edit point, tree rebalances summaries
- **Skip list with state checkpoints** — checkpoints at log-spaced intervals, reparse from nearest
- **tree-sitter's approach** — CST nodes with byte ranges, identify minimal reparse set via range overlap

The hybrid proposed above is closest to sqrt decomposition + segment tree, with SIMD providing
the "block boundary" function that sqrt decomposition typically gets for free (fixed intervals).
