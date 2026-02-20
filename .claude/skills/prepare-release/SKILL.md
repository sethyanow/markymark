---
name: prepare-release
description: Prepare a markymark release — version bump, quality gates, PR, and tag with human checkpoints
---

# prepare-release

Prepare a markymark release through a conversational 4-phase workflow with human checkpoints between each phase. The agent automates tedious parts (commit classification, version bumping, Cargo.lock regeneration) while the human makes all decisions (version number, PR approval, merge).

## When to Use

Use this skill when:
- You are preparing a new release of markymark
- You need to bump the version across all workspace crates
- You want to create a release PR from `dev` to `main`
- You need to tag a release after a PR is merged

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

2. **Edit `markymark-plugin/.claude-plugin/plugin.json`** version field:
   - Change ONLY the `"version"` value. Do not reformat, reorder keys, or modify other fields.

3. **Validate plugin.json syntax:**
   ```bash
   python3 -c "import json; json.load(open('markymark-plugin/.claude-plugin/plugin.json')); print('plugin.json: valid')"
   ```

4. **Cross-file version assertion** (both files must match):
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

5. **Rebuild to regenerate Cargo.lock:**
   ```bash
   cargo build
   ```
   This MUST use `cargo build` (not `cargo check`) because `cargo check` does not reliably update `Cargo.lock` for all workspace members.

   **If build fails:**
   - Diagnose the error
   - If unrelated to version change: fix the root cause first (separate commit), then retry
   - If caused by the version change itself (unlikely): revert edits with `git checkout -- Cargo.toml markymark-plugin/.claude-plugin/plugin.json`

6. **Validate Cargo.lock regeneration** (dynamic, not hardcoded):
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

7. **Run full quality gates** (all must pass before committing):
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

8. **Validate RELEASING.md publish order** against current cargo metadata:
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

9. **Commit ALL version-bumped files in one commit:**
   ```bash
   git add Cargo.toml Cargo.lock markymark-plugin/.claude-plugin/plugin.json
   # Also add RELEASING.md if it was updated in step 8
   git commit -m "$(cat <<'EOF'
   chore(release): bump version to A.B.C
   EOF
   )"
   ```
   The version bump commit must contain ONLY version-related files. No unrelated changes.

   **If pre-commit hooks fail:** The commit did NOT happen. Fix the issue, re-stage, and create a NEW commit attempt. Do NOT use `--amend`.

10. **Push to remote immediately** (prevents race conditions):
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

The PR is ready for your review. Please merge it when satisfied.
After you merge, tell me and I'll tag the release.
```

---

## Phase 4: Tag

**Goal:** Tag the release on `main` after the human has merged the PR.

### Steps

1. **Switch to main and pull** (tag MUST be on main, not dev):
   ```bash
   git fetch origin
   git checkout main
   git pull origin main
   ```

2. **Verify the version bump commit is present:**
   ```bash
   git log --oneline -5
   ```
   Confirm the version bump commit (or merge commit containing it) is visible.

3. **Create and push the tag:**
   ```bash
   git tag vA.B.C
   git push origin vA.B.C
   ```

4. **Verify the tag:**
   ```bash
   git log --oneline -1 vA.B.C
   ```
   This should show the merge commit on main.

5. **Switch back to dev:**
   ```bash
   git checkout dev
   git pull origin dev
   ```

6. **Present completion:**
   ```
   ## Release Tagged

   Tag: vA.B.C
   Branch: main (correct)
   Release CI: Triggered by tag push

   The release workflow will:
   1. Build binaries for 5 targets
   2. Package the Claude Code plugin archive
   3. Create a GitHub Release with artifacts

   Monitor at: https://github.com/sethyanow/markymark/actions
   ```

---

## Rollback Procedures

If something goes wrong at any phase:

| Phase | Rollback |
|-------|----------|
| Phase 2 (before commit) | `git checkout -- Cargo.toml Cargo.lock markymark-plugin/.claude-plugin/plugin.json` |
| Phase 2 (after commit, before push) | `git reset HEAD~1` (undoes commit, keeps changes staged) |
| Phase 3 (PR created) | Close the PR via `gh pr close [number]` |
| Phase 4 (tag pushed) | `git tag -d vA.B.C && git push origin :refs/tags/vA.B.C` (delete local and remote tag) |

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
