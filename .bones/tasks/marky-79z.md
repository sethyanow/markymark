---
id: marky-79z
title: Write RELEASING.md with release workflow documentation
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-peu
---


Document the complete release process for future maintainers.

## Deliverables
1. RELEASING.md covering:
   - Pre-release checklist (tests, clippy, metadata)
   - crates.io publish order and commands
   - Git tagging convention
   - GitHub Actions release pipeline
   - Claude Code marketplace submission steps
   - Post-release verification

## Design

## Goal
Document the complete release process for future maintainers.

## Implementation Steps

### Step 1: Create RELEASING.md
At /Volumes/code/markymark/RELEASING.md with sections:

1. Pre-release checklist (tests, clippy, fmt, plugin tests, metadata check)
2. Version bumping (workspace version in root Cargo.toml)
3. crates.io publishing (exact commands in dependency order with --dry-run first):
   - markymark-core -> parser -> index -> lsp -> mcp -> cli
4. GitHub Release (tag convention v{VERSION}, push triggers CI)
5. Claude Code marketplace (download plugin archive from GH Release, submit via web)
6. Post-release verification (crates.io, GitHub Release, docs.rs)

### Step 2: Commit
\`\`\`bash
git add RELEASING.md
git commit -m "docs: add release workflow documentation"
\`\`\`

## Success Criteria
- [ ] RELEASING.md exists at project root
- [ ] Covers crates.io publish order with exact commands
- [ ] Covers GitHub Release tag workflow
- [ ] Covers Claude Code marketplace submission
- [ ] Includes pre-release and post-release checklists
