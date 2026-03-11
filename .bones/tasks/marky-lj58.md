---
id: marky-lj58
title: 'Fix prepare-release Phase 2: step numbering, assertion scope, rollback completeness'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

Three correctness gaps in .claude/skills/prepare-release/SKILL.md Phase 2, exposed by PR #42 review (Copilot + CodeRabbit). All introduced because inter-crate dep version bumping was added this release but Phase 2 wasn't updated consistently.

**A. Step numbering skip (P4) — line 162**
Phase 2 steps go 5 → 7 (step 6 is absent). Should be renumbered to 6, with subsequent steps shifting down by one.

**B. Cross-file assertion scope misleading (P3) — lines 146-160**
The assertion labeled 'Cross-file version assertion (all files must match)' only checks crate *package* versions (via cargo metadata) and plugin.json. It does NOT check the `version = "X.Y.Z"` strings in each crate's `[dependencies]` section. The MEMORY.md Known Pitfall #4 records that forgetting these caused v0.5.0 to fail to build.
Options:
  - Extend the script to also validate inter-crate dep version fields
  - Or clarify the label: 'Cross-crate package version assertion' + add a note that dep requirements are caught by step 7 (cargo build)

**C. Rollback command incomplete (P2) — line 171**
`git checkout -- Cargo.toml markymark-plugin/.claude-plugin/plugin.json` omits `markymark-*/Cargo.toml` even though Phase 2 now edits those files too (inter-crate dep version bumping added in PR #42). Incomplete rollback leaves the project in an inconsistent state.
Fix: `git checkout -- Cargo.toml markymark-*/Cargo.toml markymark-plugin/.claude-plugin/plugin.json`
