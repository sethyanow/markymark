---
name: prepare-release
description: Prepare a markymark release — version bump, quality gates, PR, tag, and release notes with human checkpoints
---

# prepare-release

Prepare a markymark release through a conversational 5-phase workflow with human checkpoints between each phase. The agent automates tedious parts (commit classification, version bumping, Cargo.lock regeneration, release notes) while the human makes all decisions (version number, PR approval, merge, tagging).

## When to Use

Use this skill when:
- You are preparing a new release of markymark
- You need to bump the version across all workspace crates
- You want to create a release PR from `dev` to `main`
- You need to refine release notes after a tag is pushed

## Prerequisites

Before starting, verify:

```bash
# Ensure tags are fetched (needed for changelog range)
git fetch --tags

# Check for git-cliff (optional, has fallback)
command -v git-cliff && echo "git-cliff available" || echo "git-cliff not installed — will use git log fallback"
```

**Lefthook pre-commit hooks** will run on the version bump commit: fmt check, clippy, cargo-audit, gitleaks, zig build. These are protective gates, not obstacles.

## Version Format Reference

| Location | Format | Example |
|----------|--------|---------|
| `Cargo.toml` `workspace.package.version` | `X.Y.Z` (no `v` prefix) | `0.5.0` |
| `markymark-plugin/.claude-plugin/plugin.json` `version` | `X.Y.Z` (no `v` prefix) | `0.5.0` |
| Git tag | `vX.Y.Z` (with `v` prefix) | `v0.5.0` |
| `Cargo.lock` internal crate entries | `X.Y.Z` (auto-generated) | `0.5.0` |

---

## Phase 1: Assessment

**Goal:** Classify commits since last release and propose a semver version.

### Steps

1. **Find the last release tag:**
   ```bash
   git fetch --tags
   git tag --list 'v*' --sort=-version:refname | head -1
   ```

2. **List commits since last tag:**
   ```bash
   git log $(git tag --list 'v*' --sort=-version:refname | head -1)..HEAD --oneline
   ```

3. **Classify commits by conventional commit type:**
   - `feat` = new feature (minor bump)
   - `fix` = bug fix (patch bump)
   - Any commit with `BREAKING CHANGE` or `!` after type = major bump
   - `refactor`, `perf`, `docs`, `test`, `chore`, `ci`, `style` = patch bump (no user-facing change)

4. **Propose semver bump** based on the highest-priority commit type:
   - Any breaking change -> **major**
   - Any `feat` -> **minor**
   - Only fixes/refactors/docs -> **patch**

5. **Generate changelog preview:**

   If git-cliff is available:
   ```bash
   git-cliff --unreleased --strip header
   ```

   If git-cliff is NOT available (fallback):
   ```bash
   git log $(git tag --list 'v*' --sort=-version:refname | head -1)..HEAD --pretty=format:"- %s (%h)" --reverse
   ```

6. **Check for non-conventional commits** (filtered by git-cliff's `filter_unconventional = true`):
   ```bash
   # Count total vs conventional
   TOTAL=$(git log $(git tag --list 'v*' --sort=-version:refname | head -1)..HEAD --oneline | wc -l)
   echo "Total commits: $TOTAL"
   # If using git-cliff, compare against changelog entry count
   ```
   If significant commits were filtered, warn the human.

7. **Present to human:**
   ```
   ## Release Assessment

   Last release: vX.Y.Z
   Commits since: N

   ### Commit Classification
   - Features: N
   - Bug Fixes: N
   - Refactoring: N
   - Documentation: N
   - Other: N

   ### Proposed Version: X.Y.Z -> A.B.C (MINOR/PATCH/MAJOR bump)

   ### Changelog Preview
   [changelog content]

   Please confirm the version number or specify a different one.
   ```

### STOP: Wait for human to confirm version number.

---

## Phase 2: Version Bump

**Goal:** Bump version in all files, run quality gates, commit.

**Critical ordering:** Edit files -> build -> quality gates -> commit. NEVER commit before build succeeds.

### Steps

1. **Edit `Cargo.toml`** (root workspace version):
   ```toml
   [workspace.package]
   version = "A.B.C"
   ```

2. **Edit all inter-crate dependency versions** in each crate's `Cargo.toml`:
   - Search for `markymark-` dependencies with `version = "OLD"` across all crate `Cargo.toml` files
   - Update each to `version = "A.B.C"` while preserving `path`, `optional`, and `features` attributes
   - Affected files: `markymark-index/Cargo.toml`, `markymark-parser/Cargo.toml`, `markymark-lsp/Cargo.toml`, `markymark-cli/Cargo.toml`, `markymark-mcp/Cargo.toml`
   - Use `grep` to find all instances: `grep -rn 'markymark-.*version = "' */Cargo.toml`

3. **Edit `markymark-plugin/.claude-plugin/plugin.json`** version field:
   - Change ONLY the `"version"` value. Do not reformat, reorder keys, or modify other fields.

4. **Validate plugin.json syntax:**
   ```bash
   python3 -c "import json; json.load(open('markymark-plugin/.claude-plugin/plugin.json')); print('plugin.json: valid')"
   ```

5. **Cross-crate package version assertion** (all crate package versions and plugin.json must match):
   ```bash
   CARGO_VER=$(cargo metadata --format-version 1 --no-deps | python3 -c "
   import json, sys
   pkgs = [p for p in json.load(sys.stdin)['packages'] if p['name'].startswith('markymark')]
   versions = set(p['version'] for p in pkgs)
   assert len(versions) == 1, f'Version mismatch across crates: {versions}'
   print(versions.pop())")

   PLUGIN_VER=$(python3 -c "import json; print(json.load(open('markymark-plugin/.claude-plugin/plugin.json'))['version'])")

   echo "Cargo.toml version: $CARGO_VER"
   echo "plugin.json version: $PLUGIN_VER"
   [ "$CARGO_VER" = "$PLUGIN_VER" ] && echo "Versions match" || echo "ERROR: Version mismatch!"
   ```
   **Note:** This checks crate *package* versions and plugin.json. Inter-crate dependency `version = "..."` fields (step 2) are validated by `cargo build` in step 6 — if they're wrong, the build fails with "failed to select a version for the requirement".

6. **Rebuild to regenerate Cargo.lock:**
   ```bash
   cargo build
   ```
   This MUST use `cargo build` (not `cargo check`) because `cargo check` does not reliably update `Cargo.lock` for all workspace members.

   **If build fails:**
   - Diagnose the error
   - If unrelated to version change: fix the root cause first (separate commit), then retry
   - If caused by the version change itself (unlikely): revert edits with `git checkout -- Cargo.toml markymark-*/Cargo.toml markymark-plugin/.claude-plugin/plugin.json`

7. **Validate Cargo.lock regeneration** (dynamic, not hardcoded):
   ```bash
   cargo metadata --format-version 1 --no-deps | python3 -c "
   import json, sys
   pkgs = [p for p in json.load(sys.stdin)['packages'] if p['name'].startswith('markymark')]
   print(f'{len(pkgs)} internal crates: {sorted(p[\"name\"] for p in pkgs)}')
   versions = set(p['version'] for p in pkgs)
   assert len(versions) == 1, f'Version mismatch: {versions}'
   print(f'All at version {versions.pop()}')"
   ```
   Also verify Cargo.lock actually changed:
   ```bash
   git diff --stat Cargo.lock | head -3
   ```
   If Cargo.lock is unchanged after a version bump, something went wrong.

8. **Run full quality gates** (all must pass before committing):
   ```bash
   # Format check
   cargo fmt --all -- --check

   # Lint
   cargo clippy --workspace --all-targets -- -D warnings

   # All tests
   cargo test --workspace

   # Smoke tests
   cargo test -p markymark-cli --test smoke_lsp --test smoke_mcp

   # E2E protocol tests
   cargo test -p markymark-cli --test lsp_methods --test mcp_methods -- --nocapture

   # Plugin hook tests
   bash markymark-plugin/tests/test_hooks.sh
   ```

   **If quality gates fail:**
   - **Pre-existing failure** (was already failing before version bump): Fix the underlying issue in a **separate commit** before the version bump commit. Then re-run gates.
   - **Regression from version bump** (rare — investigate Cargo.lock diff for dependency version changes): Pin the dependency and re-attempt.
   - **NEVER** commit the version bump with failing gates.
   - **NEVER** amend a previous commit to include fixes. Always create new commits.

9. **Validate RELEASING.md publish order** against current cargo metadata:
   ```bash
   cargo metadata --format-version 1 --no-deps | python3 -c "
   import json, sys
   meta = json.load(sys.stdin)
   for p in sorted(meta['packages'], key=lambda x: x['name']):
       if not p['name'].startswith('markymark'): continue
       deps = [d['name'] for d in p['dependencies']
               if d['name'].startswith('markymark') and d.get('kind') is None]
       print(f\"{p['name']}: {deps if deps else '(none)'}\")"
   ```
   Compare the output against the publish order listed in RELEASING.md. If they differ, update RELEASING.md as part of this commit.

10. **Commit ALL version-bumped files in one commit:**
   ```bash
   git add Cargo.toml Cargo.lock markymark-plugin/.claude-plugin/plugin.json
   git add markymark-*/Cargo.toml  # inter-crate dependency versions
   # Also add RELEASING.md if it was updated in step 10
   git commit -m "$(cat <<'EOF'
   chore(release): bump version to A.B.C
   EOF
   )"
   ```
   The version bump commit must contain ONLY version-related files. No unrelated changes.

   **If pre-commit hooks fail:** The commit did NOT happen. Fix the issue, re-stage, and create a NEW commit attempt. Do NOT use `--amend`.

11. **Push to remote immediately** (prevents race conditions):
    ```bash
    git push origin dev
    ```

### STOP: Show the diff to the human for review.

```
## Version Bump Complete

Version bumped to A.B.C across:
- Cargo.toml (workspace version)
- plugin.json
- Cargo.lock (N internal crates updated)
- RELEASING.md (if publish order changed)

Quality gates: All passing
Commit: [hash]
Pushed to: origin/dev

Please review the changes. When ready, I'll create the PR.
```

---

## Phase 3: PR Prep

**Goal:** Create a dev -> main PR with changelog body.

### Steps

1. **Check for unexpected commits** (race condition guard):
   ```bash
   git pull --rebase origin dev
   git log origin/main..HEAD --oneline
   ```
   If there are commits beyond the version bump (and any pre-existing fix commits), STOP and alert the human. Do NOT create a PR with unexpected content.

2. **Generate PR body:**

   If git-cliff is available:
   ```bash
   git-cliff --latest --strip header
   ```

   If git-cliff is NOT available (fallback):
   ```bash
   git log $(git tag --list 'v*' --sort=-version:refname | head -1)..HEAD --pretty=format:"- %s (%h)" --reverse
   ```

3. **Create PR:**
   ```bash
   gh pr create --base main --head dev \
     --title "Release vA.B.C" \
     --body "$(cat <<'EOF'
   ## Release vA.B.C

   [changelog content from step 2]

   ## Checklist
   - [ ] Changelog reviewed
   - [ ] Version numbers correct (Cargo.toml, plugin.json, Cargo.lock)
   - [ ] Quality gates passing
   - [ ] Ready to merge
   EOF
   )"
   ```

4. **Agent NEVER merges the PR** (Project Rule #7). The human merges all PRs.

### STOP: Wait for human to merge the PR.

```
## PR Created

PR: [URL]
Base: main <- dev
Title: Release vA.B.C

The PR is ready for your review. Please:
1. Merge it when satisfied
2. Tag the release: `git tag vA.B.C && git push origin vA.B.C`
3. Tell me when the tag is pushed — I'll refine the release notes
```

---

## Phase 4: Tag (Human-Owned)

**Goal:** Tag the release on `main` after the human has merged the PR.

**This phase is performed by the human**, not the agent. The agent cannot checkout `main` in a worktree environment (where `main` is checked out in a different worktree). The human tags from their main worktree or bare repo.

### Human performs these steps

```bash
# From the main worktree (or any checkout of main):
git checkout main
git pull origin main
git tag vA.B.C
git push origin vA.B.C
```

### STOP: Wait for human to confirm the tag is pushed.

The agent verifies the tag exists:
```bash
git fetch --tags
git log --oneline -1 vA.B.C
```

---

## Phase 5: Release Notes Refinement

**Goal:** Replace the auto-generated git-cliff release notes with a curated, narrative version.

The CI release workflow (triggered by the tag push) creates a GitHub Release with auto-generated notes from git-cliff. These are a raw commit dump with internal issue IDs and no narrative structure. This phase replaces them with human-quality release notes.

### Steps

1. **Read the auto-generated release notes:**
   ```bash
   gh release view vA.B.C --json body --jq '.body'
   ```

2. **Fetch PR review comments** for additional context (Copilot summary, CodeRabbit findings):
   ```bash
   # Look up the release PR number (dev -> main, most recent merged)
   PR_NUMBER=$(gh pr list --base main --head dev --state merged --limit 1 --json number --jq '.[0].number')
   if [ -z "$PR_NUMBER" ]; then
     echo "Warning: No merged dev->main PR found. Skipping review comments."
   else
     gh api repos/sethyanow/markymark/pulls/$PR_NUMBER/reviews --jq '.[].body'
   fi
   ```

3. **Draft curated release notes** following this structure:

   ```markdown
   ## vA.B.C — [Release Title: 2-4 word theme]

   [1-2 paragraph narrative describing the major theme of this release.
   What changed architecturally? What's the user-visible impact?]

   ### Highlights
   - **[Theme 1]** — [1-2 sentence summary with architectural context]
   - **[Theme 2]** — [1-2 sentence summary]

   ### New Features
   - [Feature description] ([commit](link))

   ### [Thematic Bug Fix Groups]
   Group fixes by theme (Soundness, FFI, LSP, Parser, etc.) rather than
   listing them flat. Each group gets a heading like:
   - "Soundness & Memory Safety"
   - "FFI Hardening"
   - "LSP Fixes"
   - "Parser Fixes"

   ### Testing
   - [Notable test additions]

   ### Refactoring
   - [Notable structural changes]

   ### Infrastructure
   - [Build/release/CI changes]

   **Full diff:** [vPREV...vA.B.C](https://github.com/sethyanow/markymark/compare/vPREV...vA.B.C)
   **Release PR:** [#N](https://github.com/sethyanow/markymark/pull/N)
   ```

   **Writing guidelines:**
   - Lead with a narrative intro explaining the architectural story
   - Group by theme, not by flat commit type
   - Include commit links for traceability
   - Strip internal noise: memory curation commits, merge commits, version bump commits
   - Do NOT include previous release content (git-cliff sometimes includes it)
   - Keep internal issue IDs (marky-xxxx) out of user-facing notes

4. **Present draft to human for approval** before publishing.

### STOP: Wait for human to approve release notes.

5. **Publish the curated notes:**
   ```bash
   gh release edit vA.B.C --notes "$(cat <<'ENDOFNOTES'
   [approved release notes content]
   ENDOFNOTES
   )"
   ```

6. **Verify the update:**
   ```bash
   gh release view vA.B.C --json name,tagName --jq '"\(.name) — \(.tagName)"'
   ```

---

## Rollback Procedures

If something goes wrong at any phase:

| Phase | Rollback |
|-------|----------|
| Phase 2 (before commit) | `git checkout -- Cargo.toml Cargo.lock markymark-plugin/.claude-plugin/plugin.json markymark-*/Cargo.toml` |
| Phase 2 (after commit, before push) | `git reset HEAD~1` (undoes commit, keeps changes staged) |
| Phase 3 (PR created) | Close the PR via `gh pr close [number]` |
| Phase 4 (tag pushed) | `git tag -d vA.B.C && git push origin :refs/tags/vA.B.C` (delete local and remote tag) |
| Phase 5 (notes published) | Re-run `gh release edit vA.B.C --notes "..."` with corrected content |

---

## Error Handling

### Build failure after version edit
1. Diagnose the build error
2. If unrelated to version change: fix root cause in a separate commit, then retry build
3. If version-related: revert edits (`git checkout -- Cargo.toml markymark-plugin/.claude-plugin/plugin.json`)

### Quality gate failure
1. Determine if pre-existing or caused by version bump
2. Pre-existing: fix in separate commit(s) before the version bump commit
3. Version-related: investigate Cargo.lock diff for dependency changes
4. NEVER commit version bump with failing gates

### Pre-commit hook failure
1. The commit did NOT happen — do not use `--amend`
2. Fix the issue
3. Re-stage files
4. Create a NEW commit attempt

### Unexpected commits on dev (race condition)
1. `git log origin/main..HEAD --oneline` shows unexpected commits
2. STOP and alert the human
3. Do NOT create PR with unreviewed content

## Related

- [RELEASING.md](../../../RELEASING.md) — full release documentation including crates.io publishing
- [MEMORY.md Release Process](../../../docs/MEMORY.md) — version locations and known pitfalls
- [cliff.toml](../../../cliff.toml) — git-cliff changelog configuration
