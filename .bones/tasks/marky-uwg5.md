---
id: marky-uwg5
title: Update RELEASING.md publish order and add Release Process to MEMORY.md
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-t124
---



Correct RELEASING.md to include markymark-kernels and fix dependency-derived publish order. Add Release Process section to MEMORY.md documenting version locations, Cargo.lock pitfall, and publish order. This is documentation-only work (no code changes).

## Design

## Goal
Fix stale release docs and create persistent agent memory for release process knowledge. Documentation-only work — no source code changes.

## Effort Estimate
2-3 hours

## Implementation

### 1. Fix RELEASING.md publish order (Section: 'crates.io Publishing')

Current RELEASING.md (WRONG — missing kernels, wrong order):
```
core → parser → index → lsp → mcp → cli
```

Correct order (verified via `cargo metadata --format-version 1 --no-deps`):
```
1. kernels   (no regular internal deps)
2. core      (depends on kernels)
3. parser    (depends on core)
4. index     (depends on core, kernels, parser)
5. lsp       (depends on core, index, kernels, parser)  ← can publish in parallel with mcp
6. mcp       (depends on core, index, kernels, parser)  ← can publish in parallel with lsp
7. cli       (depends on core, lsp, mcp)
```

Update the bash command block to include all 7 crates with comments noting parallel-publishable pairs.

Also add a note that the publish order should be re-derived before each release:
```bash
cargo metadata --format-version 1 --no-deps | python3 -c "..."
```

### 2. Add Release Process section to MEMORY.md

Insert new section with heading '## Release Process' after the 'Key Patterns' section and before 'Performance Optimization Roadmap'.

Content must cover these three known pitfalls:
1. **plugin.json version** — markymark-plugin/.claude-plugin/plugin.json has its own version string not derived from Cargo.toml. Must be bumped manually. (Project Rule #4)
2. **Cargo.lock regeneration** — After editing workspace version in Cargo.toml, must run `cargo build` to regenerate Cargo.lock with new internal crate versions (7 entries change). Then commit Cargo.lock. Historical precedent: v0.4.2 needed a separate commit (324f744) because lockfile was missed.
3. **Publish order staleness** — RELEASING.md publish order drifted when markymark-kernels was added. Always re-derive from `cargo metadata` before publishing.

Also include:
- Version location table: Cargo.toml workspace.package.version, plugin.json version, Cargo.lock (7 internal crate entries)
- Reference to prepare-release skill (note: skill is being created in sibling task)
- Tag convention: vMAJOR.MINOR.PATCH on main branch only

### 3. Validate with markymark LSP/MCP tools

After edits:
- Run `mcp get-outline` on MEMORY.md to verify heading hierarchy is clean
- Run `mcp get-diagnostics` on both files to check for broken links or duplicate headings
- Manual scan: no broken wiki-links or cross-references introduced

## Success Criteria
- [ ] RELEASING.md 'crates.io Publishing' section lists all 7 crates in correct dependency order
- [ ] RELEASING.md bash commands start with `cargo publish -p markymark-kernels` (was missing entirely)
- [ ] RELEASING.md notes lsp+mcp can be published in parallel
- [ ] MEMORY.md has '## Release Process' heading with version location table
- [ ] MEMORY.md documents all 3 pitfalls: plugin.json, Cargo.lock, publish order staleness
- [ ] MEMORY.md references commit 324f744 as historical evidence of Cargo.lock pitfall
- [ ] markymark get-diagnostics returns no new errors for RELEASING.md
- [ ] markymark get-diagnostics returns no new errors for docs/MEMORY.md
- [ ] No source code files modified (documentation-only constraint)

## Key Considerations (SRE Review)

**Concurrent Edit Risk:**
Other agents may be editing MEMORY.md or committing to the same branch. Before editing:
- Run `git status` to confirm no uncommitted MEMORY.md changes from other agents
- If conflicts exist, rebase or resolve before proceeding
- If other agents are actively working in this worktree, defer MEMORY.md edits

**Forward Reference to Skill:**
The MEMORY.md section references the prepare-release skill that doesn't exist yet.
Use phrasing like 'See prepare-release skill (marky-plugin/skills/prepare-release/)' without linking to a specific file path that may change.

**MEMORY.md Curation Rule:**
MEMORY.md should stay concise and high-signal. The Release Process section should be compact (under 40 lines). Don't duplicate RELEASING.md content — cross-reference it.

## Anti-Patterns
- ❌ NO duplicating full RELEASING.md content into MEMORY.md (cross-reference instead)
- ❌ NO hardcoding publish order as permanent truth (always note it must be re-derived)
- ❌ NO editing source code files (this task is documentation-only)
- ❌ NO removing existing MEMORY.md content to make room (add section, don't replace)
