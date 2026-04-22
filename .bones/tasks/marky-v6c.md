---
id: marky-v6c
title: Add ignore-filter to collect_documents (workspace scan hygiene)
status: active
type: task
priority: 2
depends_on: [marky-lpz]
parent: marky-p88
---







## Context

Finding from the 2026-04-20 debugging session (epic marky-p88).

`markymark_mcp::engine::helpers::collect_documents` at `markymark-mcp/src/engine/helpers.rs:115` walks a workspace root using a plain `std::fs::read_dir` stack and pushes every file whose extension matches `DocumentKind::from_path`. There is **no** filtering for:

- `.git/`
- `target/`
- `bazel-bin/`, `bazel-out/`, `bazel-testlogs/`
- `node_modules/`
- `.venv/`, `venv/`, `__pycache__/`
- Any other build artefact or vendored-deps directory

When `markymark --mcp` is pointed at a real development worktree, it indexes the entire checkout including Bazel outputs and Cargo targets. In the 2026-04-20 session, indexing `/Volumes/code/markymark_worktrees/optimize` did not complete in 60+ seconds — silently consuming CPU walking generated artefacts.

This is a hygiene issue in its own right, AND it amplifies the blast radius of any parser bug (e.g. marky-prs would have been triggered by more files than strictly necessary).

## Requirements

1. Respect `.gitignore` / `.ignore` / `.markymarkignore` style rules during workspace scan.
2. Hard-ignore a baseline set regardless of user config: `.git/`, `target/`, `bazel-*/`, `node_modules/`, `__pycache__/`, `.venv/`, `venv/`.
3. Preserve symlink-cycle safety (currently implicit — verify not regressed).
4. Maintain determinism: output order must still be sorted by path (existing contract at helpers.rs:143).
5. Behaviour must be opt-in-safe: a workspace without a `.gitignore` still gets the hard-ignore baseline.

## Investigation notes

- `ignore` crate (https://docs.rs/ignore) is the idiomatic choice — respects `.gitignore`, `.ignore`, global gitignore, and supports hard excludes. Already used by ripgrep and many Rust tools.
- **SRE verified 2026-04-21:** `ignore` is NOT yet in `Cargo.lock` — needs `cargo add ignore` in `markymark-mcp/Cargo.toml`. `walkdir` IS already present as a transitive dep (do not rely on that).
- Existing scan at `markymark-mcp/src/engine/helpers.rs:115-145`. Single call site: `markymark-mcp/src/engine/mod.rs:464`. Existing tests: `markymark-mcp/src/engine/tests/mod.rs:625,800`.
- **SRE verified 2026-04-21:** LSP has no independent filesystem scanner (`markymark-lsp/src/` contains only in-memory content helpers). `markymark-lsp/src/state/mod.rs:167`'s `fallback_scan_with_frontmatter` is a content parser, not a directory walker. No consolidation needed.

## Implementation

Perform in this order. Each step has a verification gate.

1. **Add `ignore` crate to `markymark-mcp/Cargo.toml`** (under `[dependencies]`). Commit `Cargo.lock` together with the toml edit per project rule #8. Also update `markymark-mcp/BUILD.bazel` deps list per project rule #13 — missing this will break the primary Bazel build.
2. **Add regression tests FIRST (TDD gate)** in `markymark-mcp/src/engine/tests/mod.rs` near the existing `collect_documents_*` tests (line 625, 800). Four scenarios:
   - a. Hard-ignore baseline: create a tempdir with `target/skip.md`, `.git/skip.md`, `node_modules/skip.md`, `bazel-bin/skip.md` and one `real.md` at root. Assert only `real.md` collected.
   - b. `.gitignore` respect: tempdir with a `.gitignore` containing `private.md`, plus `private.md` and `public.md`. Assert only `public.md` collected.
   - c. No-ignore baseline (opt-in-safe, req 5): tempdir with NO ignore files at all. Create one `target/artifact.md` + `real.md`. Assert only `real.md` collected — baseline applies without any `.gitignore` present.
   - d. Sort determinism (req 4): tempdir with `b.md`, `a.md`, `c.md` at root. Assert returned order is `[a.md, b.md, c.md]`.
   - e. Symlink-cycle safety (req 3): tempdir with a symlink loop `dir -> ..`. Wall-clock budget of 5s — fail the test if the walk exceeds it (catches accidental `follow_links(true)` regression). Also assert the expected file set is returned (catches the "passes for the wrong reason" case).
3. **Rewrite `collect_documents`** to use `ignore::WalkBuilder`:
   - Start with `WalkBuilder::new(root)`.
   - Configure hard-ignores via `.filter_entry(...)` OR by passing a `.add_custom_ignore_filename(".markymarkignore")` + baseline override file. Prefer `filter_entry` for dir-name blocklist: reject any entry whose file_name is in the hard-ignore set.
   - Baseline hard-ignore set: `[".git", "target", "node_modules", "__pycache__", ".venv", "venv"]` plus any directory starting with `bazel-` (glob prefix).
   - Keep `DocumentKind::from_path` filter for file selection.
   - Retain the existing sort at the end — `ignore::Walk` order is NOT deterministic, so the final `sort_by` is required.
4. **Regenerate the Bazel crate universe** — e.g. `CARGO_BAZEL_REPIN=1 bazel sync --only=crates` (grep the repo for the project's exact repin command first — `rg -g '*.md' -g '*.bazel' 'CARGO_BAZEL_REPIN'` is a starting point). Commit any lockfile or JSON the repin produces alongside `Cargo.lock`.
5. **Run `bazel test //markymark-mcp:markymark-mcp_test`** to validate.
6. **Run `bazel test //...`** for workspace-green.
7. **Record a measurable speedup** — time `collect_documents` on this worktree before (HEAD~1) and after in the `bn log` entry. Purpose: demonstrate the fix delivers real workload reduction; exact ratio not a target.

## Anti-Patterns

- **Do NOT** hand-roll a `.gitignore` parser. Use the `ignore` crate.
- **Do NOT** drop the final `files.sort_by` — `ignore::WalkBuilder` order is unspecified.
- **Do NOT** enable `follow_links(true)` — symlink cycles will hang the walk.
- **Do NOT** add the hard-ignore set via regex matching on full paths — use file_name comparisons in `filter_entry` for O(1) per-entry check.
- **Do NOT** forget `markymark-mcp/BUILD.bazel`. The Cargo build will pass but Bazel (primary) will fail.
- **Do NOT** read any `.env` / secret files even if encountered during the walk (CLAUDE.md rule).
- **Do NOT** widen the function's public surface. It stays `pub(crate)`.

## Key Considerations

### General

- `ignore::WalkBuilder` by default respects `.gitignore`, `.ignore`, global gitignore, and hidden-file exclusion. `.git/` is both hidden and ignored-by-convention — either gate suffices, but the explicit hard-ignore is belt-and-suspenders.
- `bazel-*` directories are NOT in a default `.gitignore` for most repos (they're typically in a top-level `.bazelignore` or global exclude). The hard-ignore baseline MUST cover them explicitly.
- `WalkBuilder::standard_filters(true)` is the default. Leave it on; do NOT disable.
- The `.markymarkignore` custom file (req 1) requires `.add_custom_ignore_filename(".markymarkignore")` — not automatic.
- Pin `ignore = "0.4"` specifically — the crate is pre-1.0 (0.4.x) and has had semver-sensitive changes in minor versions.

### Failure Catalog

**Dependency Treachery: `filter_entry` behaviour**
- Assumption: `filter_entry` closure only runs on directories (to prune subtrees)
- Betrayal: `ignore::WalkBuilder::filter_entry` docs confirm the closure runs on BOTH files and directories. Reject on file_name alone and you may also reject an unrelated regular file named `target` (rare, but real).
- Consequence: Silent missing documents.
- Mitigation: Inside the filter closure, check `DirEntry::file_type()` and ONLY apply the hard-ignore blocklist when the entry IS a directory. Regular files pass through to the `DocumentKind` filter.

**Dependency Treachery: `ignore::Walk` error items**
- Assumption: Walk iterator yields `DirEntry` values
- Betrayal: It yields `Result<DirEntry, ignore::Error>`. Errors include permission-denied, I/O failures, loop detection.
- Consequence: Unwrapping panics, propagation aborts the walk.
- Mitigation: Log-and-continue on errors to match existing semantics (`helpers.rs:122,128` uses `Err(_) => continue`). Use `.filter_map(Result::ok)` or explicit `match`.

**Input Hostility: paths with non-UTF-8 bytes**
- Assumption: Every path is UTF-8
- Betrayal: macOS/Linux filesystems accept arbitrary byte sequences in filenames
- Consequence: If we force `.to_str().unwrap()` anywhere, panic on the wrong file.
- Mitigation: `DocumentKind::from_path` already operates on `Path::extension` (`OsStr`). Avoid `.to_str().unwrap()` entirely in the walker.

**Input Hostility: legitimate user `target/` directories**
- Assumption: Any directory named `target` is a Cargo artefact
- Betrayal: A markdown-notes project could have `notes/target/` for archery notes.
- Consequence: User content silently excluded.
- Mitigation: Accept the trade-off for v1 — hard-ignore applies at all depths. Document this in commit message. If a user complains, add a depth-scoping option (`depth == 1` only) in a follow-up task. Escape hatch today: user's content gets indexed if they rename the directory. `.markymarkignore` cannot override a `filter_entry` hard-ignore.

**Temporal Betrayal: filesystem mutating mid-walk**
- Assumption: Filesystem is quiescent during the walk
- Betrayal: User runs `cargo build` simultaneously, `target/` populates mid-walk. Or a tmp file vanishes between readdir and stat.
- Consequence: Partial view. With hard-ignore of `target/`, impact is bounded. With `.gitignore` pickup, a newly-created `.gitignore` mid-walk is not respected for already-visited subtrees.
- Mitigation: Accept best-effort semantics — walk is a snapshot. Hard-ignore of `target/` reduces the main attack surface. Document in commit message, not a blocker.

**Resource Exhaustion: pathological workspaces**
- Assumption: Workspace fits in memory as `Vec<(PathBuf, DocumentKind)>`
- Betrayal: Monorepo with 1M+ markdown files survives the filter.
- Consequence: Memory bloat, slow scan.
- Mitigation: Not a correctness issue; still bounded by post-filter count. Document that huge workspaces need explicit `.gitignore` or `.markymarkignore` entries. Out of scope for this task.

**State Corruption: test contamination from global gitignore**
- Assumption: Tempdir tests are hermetic
- Betrayal: Developer has `~/.config/git/ignore` containing `*.md` (or `/tmp/*`). Walk reads it via `standard_filters(true)` default. Tests "pass" by excluding everything.
- Consequence: False-green CI; real regression ships.
- Mitigation: In test helpers, wrap with an env shield: `std::env::set_var("XDG_CONFIG_HOME", tmp.path())` before constructing the WalkBuilder. Or use `WalkBuilder::standard_filters(false)` + `add_custom_ignore_filename` for the precise set. Pick ONE and use consistently across all five scenarios.

**Temporal Betrayal: symlink-cycle test weak assertion**
- Assumption: Symlink cycle test asserts walk completes
- Betrayal: `ignore::Walk`'s default `follow_links(false)` means symlinks aren't traversed at all — the cycle is never even attempted. Test passes for the wrong reason. If someone later enables `follow_links(true)`, test still "passes" (returns quickly, asserting non-infinite-loop) because there's no positive check that cycles were detected.
- Consequence: Regression in follow-links configuration goes undetected.
- Mitigation: Instrument the test with a wall-clock budget (e.g., 5 seconds). If walk exceeds budget, fail test with explicit "symlink cycle leaked — are follow_links enabled?" message. Plus assert the expected file count is returned.

**Dependency Treachery: Bazel crate universe regeneration**
- Assumption: Adding `ignore` to `Cargo.toml` makes it available in Bazel via `@crates//:ignore`
- Betrayal: `rules_rust`'s `crates_repository` cache is stale until manually repinned. First Bazel build after the edit fails with "no such target".
- Consequence: CI red on the commit that adds `ignore` — confusing for reviewers.
- Mitigation: Run `CARGO_BAZEL_REPIN=1 bazel sync --only=crates` (or the project's equivalent command) before `bazel test //...`. Commit the resulting lockfile / JSON alongside Cargo.lock. Grep the repo for the project's exact repin command before guessing.

## Success Criteria

- [x] `ignore` crate added to `markymark-mcp/Cargo.toml` and `markymark-mcp/BUILD.bazel`, `Cargo.lock` committed (req 1)
- [x] Hard-ignore baseline rejects `.git/`, `target/`, `bazel-*/`, `node_modules/`, `__pycache__/`, `.venv/`, `venv/` (req 2)
- [x] `.gitignore` / `.ignore` / `.markymarkignore` rules honoured during scan (req 1)
- [x] Output still sorted by path — determinism preserved (req 4)
- [x] Workspace with NO ignore files of any kind still gets the baseline hard-ignore (req 5)
- [x] Symlink cycles terminate without hanging or panicking (req 3)
- [x] Regression test covers all five scenarios (2a–2e above) — plus 8 adversarial tests
- [x] `bazel test //markymark-mcp:markymark-mcp_test` green
- [x] `bazel test //...` green
- [x] Measurable speedup recorded in `bn log`: 628 docs in 29.4ms vs >60s prior (logged 2026-04-22)

## Log

- [2026-04-22T04:00:28Z] [Seth] Speedup measurement: v6c_speedup_probe (debug build) collects 628 docs from /Volumes/code/markymark_worktrees/optimize in 29.4ms. Prior session (2026-04-20) documented > 60s hang on same worktree without ignore-filter. > 2000x improvement. Probe lives in markymark-mcp/src/engine/tests/mod.rs as an #[ignore]d test.
