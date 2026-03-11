---
id: marky-qf4k
title: Create prepare-release SKILL.md with 4-phase conversational flow
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-uwg5]
parent: marky-t124
---



## Design

## Goal
Create the prepare-release skill at markymark-plugin/skills/prepare-release/SKILL.md following the existing markdown-check skill format. The skill guides agents through a 4-phase conversational release workflow with human checkpoints.

## Context
Completed marky-uwg5: RELEASING.md and MEMORY.md are now up to date. This task implements the core skill definition (R1-R13) and updates CLAUDE.md (R15).

## Implementation

### 1. Read existing skill format
Read markymark-plugin/skills/markdown-check/SKILL.md to understand format conventions.

### 2. Create SKILL.md
Create markymark-plugin/skills/prepare-release/SKILL.md with:

**YAML frontmatter:**
- name: prepare-release
- description: Prepare a markymark release (version bump, quality gates, PR, tag)

**Phases (conversational, human-in-the-loop):**

**Phase 1: Assessment**
- List commits since last tag using git log
- Classify by conventional commit type (feat/fix/refactor/docs/chore)
- Propose semver bump (major/minor/patch) based on commit types
- Show git-cliff changelog preview
- STOP: Ask human to confirm version number

**Phase 2: Version Bump**
- Edit Cargo.toml workspace.package.version
- Edit markymark-plugin/.claude-plugin/plugin.json version
- Run cargo build to regenerate Cargo.lock
- Run full quality gates: fmt, clippy, test, smoke tests
- Validate RELEASING.md publish order against cargo metadata
- Update RELEASING.md if publish order is stale
- Commit ALL bumped files in one commit
- STOP: Show diff for human review

**Phase 3: PR Prep**
- Create dev→main PR with changelog body from git-cliff
- Agent NEVER merges (Rule #7)
- STOP: Wait for human to merge

**Phase 4: Tag**
- After human confirms merge, push vX.Y.Z tag
- Verify tag triggers release workflow

### 3. Update CLAUDE.md
Add reference to prepare-release skill in the Quick Reference section. Document the Cargo.lock commit requirement.

## Success Criteria
- [ ] SKILL.md exists at markymark-plugin/skills/prepare-release/SKILL.md
- [ ] YAML frontmatter matches markdown-check conventions
- [ ] All 4 phases documented with clear STOP points
- [ ] Cargo.lock regeneration is explicit in Phase 2
- [ ] plugin.json bump is explicit in Phase 2
- [ ] Publish order validation uses cargo metadata (not hardcoded)
- [ ] CLAUDE.md references the skill
- [ ] Agent merge prohibition is explicit (Rule #7)

## Anti-Patterns
- NO automated PR merging
- NO hardcoded publish order
- NO skipping quality gates
- NO tagging before PR merge

---

## SRE Task Refinement (2026-02-20)

Analyzed edge cases, race conditions, rollback scenarios, tool reliability, and format correctness across all 4 phases.

### Finding 1: Version bump + cargo build failure (Phase 2 — Rollback gap)

**Scenario:** Agent edits Cargo.toml version, then `cargo build` fails (e.g., dependency resolution issue, Zig build.rs failure, network error fetching registry).

**Risk:** Cargo.toml is dirty with the new version, Cargo.lock is partially regenerated or missing, and the workspace is in a broken state. If the agent commits anyway or proceeds to quality gates, the version bump commit contains a broken build.

**Mitigation the skill MUST specify:**
1. The version bump commit MUST NOT be created until `cargo build` succeeds. The skill should describe the sequence as: edit files → build → quality gates → commit. NOT: edit files → commit → build.
2. On `cargo build` failure, the skill should instruct: (a) diagnose the build error, (b) if it is unrelated to the version change (e.g., pre-existing Zig compilation issue), fix the root cause first and re-attempt, (c) if the version itself caused the failure (should not happen for pure version bumps), revert the edits with `git checkout -- Cargo.toml markymark-plugin/.claude-plugin/plugin.json`.
3. Document that `cargo build` also regenerates `Cargo.lock`. A failed build may leave `Cargo.lock` in an inconsistent state. The skill should mandate `cargo build` (not just `cargo check`) because `cargo check` does NOT update `Cargo.lock` reliably for all workspace members.

**Severity:** Medium. Historical precedent: v0.4.2 needed a separate fixup commit (324f744) because `Cargo.lock` was not committed alongside `Cargo.toml`. This exact scenario.

### Finding 2: Race condition — push to dev between version bump and PR (Phase 2→3 gap)

**Scenario:** Agent completes Phase 2 (version bump committed on `dev`), human approves the diff, then in Phase 3 the agent creates a dev→main PR. Between Phase 2 completion and Phase 3 PR creation, another developer (or agent) pushes new commits to `dev`.

**Risk:** The PR includes unintended commits beyond the version bump. The changelog generated from git-cliff would be correct (it is based on the tag range), but the PR diff would contain surprise code changes.

**Mitigation the skill MUST specify:**
1. Phase 2 should end with a `git push` to `origin/dev` immediately after the version bump commit. This anchors the commit on remote.
2. Phase 3 should begin with `git pull --rebase origin dev` and re-check `git log origin/main..HEAD` to verify only expected commits are in the PR range.
3. If unexpected commits appear, STOP and alert the human. Do NOT create the PR with unexpected content.
4. The skill should note that the human STOP between Phase 2 and Phase 3 is where this race is most likely — the longer the human takes to approve, the higher the risk. Recommend proceeding to Phase 3 promptly after Phase 2 approval.

**Severity:** Medium. The git model (dev→main PRs, confirmed by merge history: PRs #36, #37, #38 all from `sethyanow/dev`) makes this a realistic scenario since `dev` is a shared branch.

### Finding 3: Quality gate failure after version bump (Phase 2 — Rollback scenario)

**Scenario:** Version is bumped, `cargo build` succeeds, but a quality gate fails: `cargo fmt` reports formatting issues, `cargo clippy` finds new warnings (possibly from updated dependency resolution), `cargo test` has failures, or smoke/E2E tests fail.

**Risk:** The version bump is entangled with an unrelated quality regression. Agent may be tempted to commit the version bump anyway and "fix later."

**Mitigation the skill MUST specify:**
1. Quality gates MUST pass BEFORE the version bump commit is created. This is a hard block, not advisory.
2. On quality gate failure, the skill should distinguish two cases:
   - (a) **Pre-existing failure** (test was already failing before version bump): the version bump is not the cause. Fix the underlying issue first, re-run gates, then proceed. This may require a separate commit before the version bump commit.
   - (b) **Regression caused by version bump** (extremely unlikely for a pure version string change, but possible if Cargo.lock resolution changes pull in a different dependency version): investigate the Cargo.lock diff, pin the dependency, and re-attempt.
3. The commit sequence should be: [fix commits if needed] → [version bump commit] → [push]. The version bump commit should be a clean, single-purpose commit containing ONLY: Cargo.toml, plugin.json, Cargo.lock, and optionally RELEASING.md if publish order changed.
4. **Never amend the version bump commit with unrelated fixes.** If a fix is needed after the version bump commit, it should be a separate commit. This preserves clean git history per project Rule #5 (never squash merge) and keeps the version bump bisectable.

**Severity:** High. This is the most likely failure mode in practice. The quality gate list from CI is substantial: fmt, clippy, full test suite, smoke tests, E2E LSP tests, E2E MCP tests, plugin hook tests. Any one failing blocks the release.

### Finding 4: git-cliff changelog command reliability (Phase 1 and Phase 3)

**Scenario:** The skill calls `git-cliff` for changelog generation. `git-cliff` is NOT installed on the developer's machine (confirmed: `which git-cliff` returns not found). It IS available in CI via the `orhun/git-cliff-action@v4` GitHub Action.

**Risks identified:**
1. **Local execution fails.** If the skill instructs the agent to run `git-cliff --unreleased` locally, it will fail with "command not found." The skill must handle this gracefully.
2. **Stale cliff.toml.** The config at `cliff.toml` uses Tera template syntax. If the Tera API changes across git-cliff versions, the local version may produce different output than CI.
3. **Tag-based range.** git-cliff derives changelogs from tag ranges. If the previous tag is missing locally (`git fetch --tags` not run), the changelog will be incorrect (too many or too few commits).
4. **`filter_unconventional = true`** in cliff.toml means non-conventional commits are silently dropped. If commits don't follow conventional format, the changelog will be incomplete. This is working-as-designed but the skill should warn the human about it.

**Mitigation the skill MUST specify:**
1. **Graceful degradation.** The skill should check `command -v git-cliff` first. If not available, fall back to `git log --oneline $(git describe --tags --abbrev=0)..HEAD` for a raw commit list. Recommend installing git-cliff but don't block on it.
2. **Tags must be fetched.** Before running git-cliff or git log with tag ranges, run `git fetch --tags`.
3. **For Phase 3 PR body**, if git-cliff is available use `git-cliff --latest --strip header` (matching the CI config args). If not, use the manual git log fallback for the PR body.
4. **Warn about non-conventional commits.** After generating the changelog, show a count of total commits vs. included commits. If they differ significantly, alert the human that some commits were filtered.

**Severity:** High. git-cliff is not installed locally, so this WILL fail on first use without the fallback logic. The CI workflow handles it via GitHub Action, but the local skill workflow has no such fallback.

### Finding 5: plugin.json format correctness (Phase 2 — Silent corruption)

**Scenario:** Agent edits `markymark-plugin/.claude-plugin/plugin.json` to bump the version field. JSON editing by agents has known failure modes.

**Risks identified:**
1. **JSON syntax error.** Agent introduces a trailing comma, missing quote, or malformed escape. The file becomes invalid JSON. No quality gate currently validates plugin.json syntax.
2. **Wrong field edited.** The file has only one `version` field at the top level, but a careless edit could accidentally modify other fields (e.g., description, or nested command strings).
3. **Version format mismatch.** Cargo.toml uses `"0.4.2"` (no `v` prefix). plugin.json also uses `"0.4.2"` (no `v` prefix). Git tags use `v0.4.2` (with prefix). The skill must be explicit about which format goes where.
4. **Version string not matching Cargo.toml.** These two files are independently edited. If one is bumped and the other forgotten, they diverge. Historical data: this has happened (Rule #4 in CLAUDE.md, MEMORY.md Release Process section).

**Mitigation the skill MUST specify:**
1. **JSON validation.** After editing plugin.json, run `python3 -c "import json; json.load(open('markymark-plugin/.claude-plugin/plugin.json'))"` (or `jq . < plugin.json > /dev/null`) to validate syntax. This should be a mandatory step, not optional.
2. **Cross-file version assertion.** After editing both files, the skill should verify they match: extract version from Cargo.toml and plugin.json and assert equality. Something like: `cargo metadata --format-version 1 --no-deps | python3 -c "import json,sys; v=set(p['version'] for p in json.load(sys.stdin)['packages'] if p['name'].startswith('markymark')); print(f'Cargo versions: {v}'); assert len(v)==1, 'version mismatch across crates'"` followed by a plugin.json check.
3. **Explicit format rules.** The skill must state: Cargo.toml version = `X.Y.Z` (no `v` prefix). plugin.json version = `X.Y.Z` (no `v` prefix). Git tag = `vX.Y.Z` (with `v` prefix).
4. **Minimal edit.** The skill should instruct: only change the `"version"` value in plugin.json. Do not reformat, reorder keys, or modify any other field.

**Severity:** Medium. No automated validation exists for plugin.json. The release CI copies it into the plugin archive (confirmed in release.yml lines 106-108) so corruption would ship broken plugin metadata.

### Finding 6: Phase 4 tag timing — tag on wrong branch (Phase 4)

**Scenario:** Human merges the dev→main PR. Agent then needs to tag `vX.Y.Z`. But the agent's local checkout may still be on `dev`, not `main`. Or `main` may not be up to date.

**Risk:** Tag is created on `dev` instead of `main`. The release CI triggers and builds from the wrong branch. The tag doesn't point to the merge commit on `main`.

**Mitigation the skill MUST specify:**
1. After human confirms merge, the agent must: `git fetch origin`, `git checkout main`, `git pull origin main`, verify the version bump commit is present (`git log --oneline -3`).
2. ONLY then: `git tag vX.Y.Z` and `git push origin vX.Y.Z`.
3. After tagging, verify: `git log --oneline -1 vX.Y.Z` shows the expected merge commit on main.
4. Convention from RELEASING.md: "Releases are always tagged from `main`" — the skill must enforce this.

**Severity:** High. This is an easy mistake to make since all development happens on `dev`. The agent will naturally be on `dev` when the human says "I merged it."

### Finding 7: Cargo.lock 7-entry invariant (Phase 2 — Validation)

**Scenario:** After `cargo build`, the skill should validate that Cargo.lock was correctly regenerated. The workspace has 7 internal crates (core, parser, index, lsp, mcp, cli, kernels).

**Risk:** If a crate is added or removed and RELEASING.md/the skill has a hardcoded "7 crate" reference, it becomes stale.

**Mitigation the skill MUST specify:**
1. Do NOT hardcode the crate count. Instead, validate dynamically: `cargo metadata --format-version 1 --no-deps | python3 -c "import json,sys; pkgs=[p for p in json.load(sys.stdin)['packages'] if p['name'].startswith('markymark')]; print(f'{len(pkgs)} internal crates: {[p[\"name\"] for p in pkgs]}')"`.
2. Verify ALL internal crates show the new version in Cargo.lock.
3. The `git diff --stat` of Cargo.lock should show changes. If Cargo.lock is unchanged after a version bump, something went wrong (the build didn't see the new version).

**Severity:** Low. Informational, but prevents a class of stale-documentation bugs.

### Finding 8: lefthook pre-commit hooks and version bump commits (Phase 2)

**Scenario:** When the agent creates the version bump commit, lefthook pre-commit hooks run: fmt check, clippy, cargo-audit, gitleaks, and zig build. These are sequential (parallel: false).

**Risk:** The commit is blocked by a pre-commit hook failure (most likely: clippy or zig build, since fmt/audit/gitleaks are unlikely to be affected by a version bump). This is actually GOOD — it's an additional quality gate. But the agent needs to understand that commit failure ≠ version bump failure.

**Mitigation the skill MUST specify:**
1. If pre-commit hooks fail on the version bump commit, the commit did NOT happen. Do NOT use `--amend` to fix and retry (per project CLAUDE.md: "Always create NEW commits rather than amending").
2. Fix the issue, re-stage, create a new commit attempt.
3. The skill should list the lefthook gates so the agent isn't surprised: fmt, clippy, cargo-audit, gitleaks, zig build.

**Severity:** Low. The hooks are protective. The skill just needs to set expectations.

### Summary Table

| # | Finding | Severity | Phase | Key Mitigation |
|---|---------|----------|-------|----------------|
| 1 | Build failure after Cargo.toml edit | Medium | 2 | Commit AFTER build+gates, not before |
| 2 | Race: new commits on dev between phases | Medium | 2→3 | Push immediately, re-check before PR |
| 3 | Quality gate failure after version bump | High | 2 | Hard block; separate fix commits from version bump |
| 4 | git-cliff not installed locally | High | 1, 3 | Graceful fallback to git log; check command -v first |
| 5 | plugin.json corruption/mismatch | Medium | 2 | JSON validation + cross-file version assertion |
| 6 | Tag on wrong branch | High | 4 | Explicit checkout main + pull before tagging |
| 7 | Hardcoded crate count | Low | 2 | Dynamic cargo metadata validation |
| 8 | lefthook blocks version bump commit | Low | 2 | Document hook list; never --amend on failure |

### Recommended Additions to Implementation

Based on SRE findings, the SKILL.md should include these sections beyond the 4 phases:

1. **Prerequisites section**: Check for git-cliff, list lefthook hooks, ensure tags are fetched.
2. **Validation commands**: JSON syntax check, cross-file version assertion, dynamic crate count.
3. **Rollback procedures**: Per-phase rollback instructions (Phase 2: git checkout files; Phase 3: close PR; Phase 4: delete tag).
4. **Version format reference table**: Cargo.toml=X.Y.Z, plugin.json=X.Y.Z, git tag=vX.Y.Z.
5. **Error handling flowchart**: Build failure → diagnose → fix → retry. Gate failure → separate commit → retry. Hook failure → fix → new commit (never amend).
