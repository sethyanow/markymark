<handoff session="2026-02-18" branch="dev">

<summary>
PR #36 review fixes session: addressed 4 CodeRabbit findings (env-gated report write,
wiki-link slug comparison bug, symlink metadata check, stale doc comment). Triaged all
remaining PR #36 review comments, filed 5 new beads issues (3 P1 bugs, 2 P2). Produced
v0.4.0 release triage categorizing 35 open issues into release-blocking vs post-release.
</summary>

<completed>
<task id="marky-u6p" status="closed">
Report write in extraction_parity.rs gated behind WRITE_PARITY_REPORT env var.
</task>

<task status="done">
PR #36 review fix: rename.rs wiki link heading slugified before comparison (regression test added).
</task>

<task status="done">
PR #36 review fix: extraction_parity.rs symlink check uses fs::symlink_metadata.
</task>

<task status="done">
PR #36 review fix: incremental/mod.rs stale doc comment for range_within_neighbor_window updated.
</task>

<task status="done">
Triaged all PR #36 CodeRabbit/Copilot review comments. Filed 5 new issues for unaddressed findings.
</task>
</completed>

<created_issues>
<issue id="marky-e2j" priority="P1" type="bug" label="pr-36,safety">
completion.rs: UTF-16 position used as byte offset for line slicing. Panic on multi-byte chars.
File: markymark-lsp/src/state/completion.rs:76-82
</issue>

<issue id="marky-pj4" priority="P1" type="bug" label="pr-36,safety">
rename.rs: closing-tag Position::new can underflow on short tag names.
File: markymark-lsp/src/state/rename.rs:155-168
</issue>

<issue id="marky-v8y" priority="P1" type="bug" label="pr-36,safety">
incremental/mod.rs: signed arithmetic wraparound in adjust_range_after_edit/adjust_bytes_after_edit.
File: markymark-lsp/src/incremental/mod.rs:138-166
</issue>

<issue id="marky-d4v" priority="P2" type="bug" label="pr-36,docs">
README.md: markymark-vscode listed in crates table but not a workspace member.
</issue>

<issue id="marky-wjf" priority="P2" type="bug" label="pr-36,incremental">
incremental: neighbor-window (100 bytes) can miss insertions in large gaps between entries.
File: markymark-lsp/src/incremental/mod.rs:170-555
</issue>
</created_issues>

<v040_release_triage>

## RELEASE TRIAGE: v0.4.0 (PR #36: dev -> main)

### Philosophy
"Don't call it a release with lingering bugs -- establish standards early."

### VERDICT: 4 P1 bugs must be fixed before merge. All are small, scoped fixes.

---

### RELEASE-BLOCKING (P1 bugs -- fix before merge)

| ID | Title | Scope | Est. |
|----|-------|-------|------|
| marky-e2j | completion.rs UTF-16 as byte offset | 1 file, use existing converter | 15min |
| marky-pj4 | rename.rs closing-tag underflow | 1 file, saturating_sub | 15min |
| marky-v8y | incremental adjust_range wraparound | 1 file, saturating arithmetic | 20min |
| marky-kvr | find-references fails on structured docs | 2 files, branch on AnyDocumentIndex | 45min |

**Total estimated: ~1.5 hours of focused work.**

All four are correctness/safety bugs that can cause panics or wrong behavior.
marky-kvr has a detailed fix plan already written in the beads comments.

---

### SHOULD-FIX (P2 -- fix before or shortly after merge)

| ID | Title | Category |
|----|-------|----------|
| marky-d4v | README lists non-existent markymark-vscode crate | docs accuracy |
| marky-wjf | incremental gap detection misses in large gaps | correctness edge case |

---

### PR #36 REVIEW COMMENTS STATUS

#### Addressed this session (4):
1. extraction_parity.rs:491 -- report write env-gated (marky-u6p closed)
2. rename.rs:101 -- wiki link slug comparison fixed + regression test
3. extraction_parity.rs:222 -- symlink_metadata check
4. incremental/mod.rs:107 -- stale doc comment updated

#### Rejected with justification (1):
- rename.rs byte-offset-for-UTF16: NOT A BUG. Entire parser uses byte offsets consistently. LSP layer converts at boundary.

#### Filed as new issues (5):
- marky-e2j, marky-pj4, marky-v8y (P1 bugs)
- marky-d4v, marky-wjf (P2)

#### Intentionally deferred (nitpicks/improvements from CodeRabbit):
These are code quality suggestions, not bugs. File as post-release improvements if desired:
- DRY: extract duplicated sort closures in engine/search.rs and engine/references.rs
- Use tempfile crate instead of manual temp_dir in pattern/tests.rs
- Escape # in glob_to_regex (defensive, no current bug)
- Make semantic duplicate threshold configurable (0.85 hardcoded)
- Use Display instead of Debug for ValueKind in outline.rs
- Add SAFETY comments to FFI test unsafe blocks in index_serde.rs
- Use .len() instead of .chars().count() in ASCII comparison (scan.rs)
- Property value serialization loses list structure (dto.rs)
- Symlink loop protection in collect_documents (helpers.rs)
- Update lib.rs doc comment to mention diagnostics/incremental modules
- HashSet contains check before insert in completion.rs XmlTag
- Rename percent() to ratio() in extraction_parity.rs
- Various markdown formatting fixes in docs/corpus/legendary_handoff.md
- HTML entity escapes (&amp;) in handoff doc code snippets

---

### POST-RELEASE OPEN ISSUES (31 issues, all P2-P4)

#### Performance Epic (marky-77i): Incremental Indexing
- marky-7dq (P2): Debounce did_change -- 50-100ms async cancellation
- marky-0jz (P2): Vendor tree-sitter-md, selective inline skip
- marky-0mr (P2): Zig md4c streaming parser (potential 50x speedup)
- marky-syx (P3): BRZA-powered lazy AST
- marky-v8g (P3): TreeSitterScanBackend wrapper

#### Feature Epics
- marky-v8e (P1 epic): v1.0 Product Launch umbrella
- marky-mkr (P2 epic): Agent Tooling (skills, MCP expansion)
- marky-hwc (P2 epic): Knowledge Plugins (Obsidian, Logseq)
- marky-qyf (P2 epic): Editor Distribution (VSCode, Neovim, Zed)
- marky-7pw (P3 epic): Roadmap Research

#### Code Quality
- marky-itd (P2): Refactor markymark-mcp/lib.rs (919 lines)
- marky-n5w (P2): Eliminate eager alloc in SearchSymbols
- marky-agk (P2): Polish plugin hooks/skills/config
- marky-luy (P2): Arena conformance closeout
- marky-u3m (P4): Split incremental/mod.rs into per-extractor submodules
- marky-lkj.1 (P4): Extract runtime_engine unit tests

#### Features
- marky-6i9 (P2): markdown-check CLI / get_diagnostics MCP tool
- marky-efm (P3): JSON document LSP support
- marky-z9z (P3): Improve markdown link resolution beyond stem-only
- marky-agv (P3): Deduplicate link edges in graph analysis
- marky-pvr (P3): Deduplicate file reads in SemanticSearch

#### Research
- marky-f2c (P3): AI-Augmented Markdown Features
- marky-n7i (P3): Advanced Markdown Intelligence
- marky-vz6 (P3): Ecosystem Integration
- marky-ix3 (P3): Cross-language symbol bridging

#### Chores
- marky-lsl (P3): Remove pr-*.json snapshot artifacts
- marky-ejt (P3): Refactor zig similarity.zig tests
- marky-w85 (P3): Non-existent realm error path test coverage
- marky-a5w (P3): Refactor oversized c_adapter.zig

</v040_release_triage>

<next_steps>
1. Fix the 4 P1 bugs (marky-e2j, marky-pj4, marky-v8y, marky-kvr) on dev branch
2. Run full test suite: cargo nextest run --workspace && cargo clippy --workspace --all-targets
3. Fix marky-d4v (README) and marky-wjf (gap detection) if time permits
4. Push to dev, update PR #36
5. Human reviews and merges PR #36 (Rule 7: agent never merges)
6. Tag v0.4.0 after merge
</next_steps>

<rules>
- Rule 7: Agent NEVER merges PRs. Human merges all PRs.
- Rule 5: Never squash merge. Preserve full git history.
- All 4 P1 bugs are scoped, small fixes with clear locations. Total ~1.5 hours.
- PR #36 has passing CI. The 4 bugs are latent issues found by code review, not test failures.
</rules>

</handoff>
