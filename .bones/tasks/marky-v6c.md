---
id: marky-v6c
title: Add ignore-filter to collect_documents (workspace scan hygiene)
status: open
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

- `ignore` crate (https://docs.rs/ignore) is the idiomatic choice — respects `.gitignore`, `.ignore`, global gitignore, and supports hard excludes. Already used by ripgrep and many Rust tools. Check if it's already in `Cargo.lock`.
- Existing scan at `markymark-mcp/src/engine/helpers.rs:115-145`. Small function (~30 lines), low refactor risk.
- LSP / other entry points: confirm they don't have their own independent scanners. If they do, consolidate.

## Success Criteria

- [ ] `.gitignore` rules honoured in workspace scan
- [ ] Baseline hard-ignore prevents indexing of `.git/`, `target/`, `bazel-*/`, `node_modules/`
- [ ] `bazel test //...` still green
- [ ] Benchmark: indexing markymark worktree completes in < 5s (was hanging past 60s)
- [ ] Regression test covers hard-ignore baseline and `.gitignore` respect
