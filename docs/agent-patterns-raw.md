# Agent Patterns — Raw Collection

Extracted from claude-harness memory during harness removal (2026-02-18).
These are patterns observed across ~170 agent sessions building a Rust/Zig workspace.
Curate into reusable agent guidance as patterns solidify.

---

## Arena & Ownership Debugging

- `DocumentIndex::from_ast` borrows from parser arena: use `arena_ptr` + `ptr::read` + `mem::forget` when borrow checker blocks `into_arena()`. Later replaced by destructuring approach (`take_arena`).
- When migrating wrapper types (e.g. Bump -> DocumentArena), trace all `ptr::read`/`mem::forget` usage — pointer type must match the owning type, not the inner type.
- `docs-vs-practice` audit: read doc, find all struct/field types in codebase, compare systematically. Key: test Send/Sync implications before changing types.

## Cross-Crate Architecture

- Core-contract-first transport layering: define traits in core, thin adapters in transport crates.
- `CoreEngine` trait in core, transport crates are thin adapters.
- Hybrid arena ownership for LSP/MCP dual-transport — arena lives in parser, doesn't cross into transport layer.
- `convert`/`state`/`server` module separation in markymark-lsp for testability.
- Startup-index runtime adapter for MCP transport.

## MCP/LSP Protocol

- `rmcp` structured error envelope: `CallToolResult::structured` and `structured_error` for MCP payloads.
- Optional PromptArgument in rmcp: `PromptArgument { required: Some(false) }`, extract with `.get(key).and_then(|v| v.as_str())`.
- Full-text document sync (not INCREMENTAL) was the v1 LSP approach. Incremental sync landed later.
- Deterministic sort by URI then range for stable output in LSP responses.

## Tree-Sitter & Parsing

- Tree-sitter cursor traversal for nested Markdown lists.
- Single-pass stack tokenizer for XML extraction (not multi-regex).
- Range containment checks for nested XML document symbols.

## Benchmark & Performance

- Tiered benchmark controls: env-tiered (samples + doc counts), run synthetic_scale per tier, parse criterion estimates.json, persist markdown report artifact.
- Benchmark real corpus: use `MARKYMARK_BENCH_EPSTEIN` for epstein path from worktree; `MARKYMARK_BENCH_HEAVY=1` for 100 samples (RAM-heavy for index_docs_dir); keep baseline worktree for pre/post comparison.
- Extract duplicated unsafe arena transmute into a single helper (DRY for unsafe code).

## CI & Security Tooling

- Custom Semgrep rules verified with deterministic fixture scans + expected finding counts when semgrep test mode is constrained by CLI version.
- Keep Semgrep rule scope high-signal by excluding `tests/**` and validating with repo-wide scan excluding fixtures.
- Run local SAST baseline (cargo audit, cargo deny, semgrep) before enabling new CI workflow to establish signal baseline.
- Security CI for this repo is advisory-only with `continue-on-error` on each scan job.
- For gitleaks staged negative testing, use a synthetic token matching built-in regex (e.g., `ghp_` + 36 chars).
- When `cargo-deny` config fails, validate against installed tool version schema before assuming policy error.

## Agent Coordination & Workflow

- When teammate implementer crashes, reassign narrow-scope blocker to test-writer to preserve momentum.
- For cross-agent handoff, attach full CodeRabbit plain review (including codegen prompts) as beads comments on the owning task.
- When scaffold task proves flow but not target performance, close it explicitly and create immediate follow-up task linked to same epic.
- When writing `bd --design` content with markdown backticks, use heredoc file + `bd update` to avoid shell command substitution.

## Testing Conventions

- `ChildGuard` RAII for process cleanup in integration tests.
- Hook scripts receive JSON on stdin, output JSON to stdout, use exit 0 for no-op on non-matching files.
- Existing bash tests use `set -euo pipefail` — causes early abort on command failures in non-conditional contexts.
- Test files follow pass/fail counter pattern with colored output and `exit 1` on any failure.

## Release & Versioning

- Version sync across Cargo workspace and plugin manifest (plugin.json version != Cargo.toml version).
- Alpha/prerelease tagging workflow: quality gates in parallel -> bump versions -> branch + PR + merge -> tag main -> push tag triggers CI release.
- Cargo prerelease deps require `=X.Y.Z-pre` exact match (no semver range).
- Audit README claims against actual implementation before every release.
- Documentation blockers for unfamiliar crates: create blocking issue, document APIs and pitfalls first, then implement.
