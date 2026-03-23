---
id: marky-n2mk
title: 'prepare-release SKILL.md: PR_NUMBER placeholder unresolved in Phase 5'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

Phase 5 Step 2 of prepare-release skill uses literal PR_NUMBER in gh api command (line 371). Phase 5 runs in a separate session from Phase 3 where the PR was created, so the agent has no way to resolve the placeholder. Fix: add a gh pr list command to programmatically look up the release PR number, or document both options (extract from URL, gh CLI lookup).

## Design

## Goal

Fix the unresolved PR_NUMBER placeholder in Phase 5 Step 2 of the prepare-release
skill so that an agent starting Phase 5 in a fresh session can programmatically
look up the release PR number.

## Effort Estimate

~30 minutes (documentation-only edit, single file)

## Context

Phase 5 runs in a separate session from Phase 3 (where the PR was created). The
agent has no memory of the PR URL. The literal text at SKILL.md line 371:

    gh api repos/sethyanow/markymark/pulls/PR_NUMBER/reviews --jq '.[].body'

needs a preceding step that resolves the PR number.

## Implementation Checklist

File: .claude/skills/prepare-release/SKILL.md (lines 369-372)

- [ ] Insert a substep before the gh api command that resolves the PR number:
      ```bash
      PR_NUMBER=\$(gh pr list --base main --head dev --state merged --limit 1 --json number --jq '.[0].number')
      ```
- [ ] Update the gh api command to use the variable:
      ```bash
      gh api repos/sethyanow/markymark/pulls/\$PR_NUMBER/reviews --jq '.[].body'
      ```
- [ ] Add a guard clause: if PR_NUMBER is empty, print diagnostic and skip
- [ ] Keep the surrounding comment ("Fetch PR review comments for additional context")

Exact replacement for lines 369-372:

Before:
```
2. **Fetch PR review comments** for additional context (Copilot summary, CodeRabbit findings):
   \`\`\`bash
   gh api repos/sethyanow/markymark/pulls/PR_NUMBER/reviews --jq '.[].body'
   \`\`\`
```

After:
```
2. **Fetch PR review comments** for additional context (Copilot summary, CodeRabbit findings):
   \`\`\`bash
   # Look up the release PR number (dev -> main, most recent merged)
   PR_NUMBER=\$(gh pr list --base main --head dev --state merged --limit 1 --json number --jq '.[0].number')
   if [ -z "\$PR_NUMBER" ]; then
     echo "Warning: No merged dev->main PR found. Skipping review comments."
   else
     gh api repos/sethyanow/markymark/pulls/\$PR_NUMBER/reviews --jq '.[].body'
   fi
   \`\`\`
```

## Success Criteria

- [ ] SKILL.md Phase 5 Step 2 no longer contains the literal string "PR_NUMBER"
      as an unresolved placeholder
- [ ] The replacement uses gh pr list with --base main --head dev --state merged
      to look up the PR number programmatically
- [ ] A guard handles the case where no matching PR is found (empty result)
- [ ] Surrounding skill structure (Phase numbering, step numbering) unchanged
- [ ] Skill still renders correctly as markdown (no broken fences)

## Key Considerations (SRE Review)

**Edge Case: Multiple merged PRs from dev to main**
The --limit 1 flag returns the most recent merged PR. Since releases are sequential
and the human just merged the release PR, the most recent is always correct. If
multiple PRs were merged in rapid succession, the agent would pick the last one,
which is the release PR.

**Edge Case: PR not yet merged**
If the human calls Phase 5 before merging, --state merged returns nothing. The guard
clause handles this by printing a warning and skipping. The agent still has the
auto-generated release notes from Step 1 and the git log from git-cliff.

**Edge Case: Repo name hardcoded**
The existing gh api URL already hardcodes sethyanow/markymark. This is a pre-existing
concern, not introduced by this change. A follow-up could use gh repo view --json
owner,name but that's out of scope.

## Anti-patterns

- Do NOT remove the existing Step 2 comment explaining what this fetches
- Do NOT change step numbering in Phase 5
- Do NOT add --state all (we only want merged PRs, not open/closed drafts)
- Do NOT use gh pr view (requires known PR number, which is exactly what we lack)
