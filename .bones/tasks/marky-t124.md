---
id: marky-t124
title: '[EPIC] Release preparation skill + process formalization'
status: closed
type: epic
priority: 1
owner: sethyanow@users.noreply.github.com
---



Create a prepare-release Claude Code plugin skill and formalize the release process in agent docs. Addresses three known release failure modes: (1) plugin.json version not bumped, (2) Cargo.lock not committed after workspace version bump, (3) stale publish order in RELEASING.md missing markymark-kernels. Skill is conversational with 4 phases: Assessment, Version Bump, PR Prep, Tag. Human stays in the loop for version confirmation, PR review, and merge.

## Design

## Requirements (IMMUTABLE)

### Skill: prepare-release
- R1: Skill lives at markymark-plugin/skills/prepare-release/SKILL.md
- R2: Conversational 4-phase flow: Assessment → Version Bump → PR Prep → Tag
- R3: Assessment phase classifies commits by conventional commit type and proposes semver version
- R4: Assessment shows git-cliff changelog preview before human confirms version
- R5: Version bump edits Cargo.toml (workspace), plugin.json, rebuilds to regen Cargo.lock
- R6: Full pre-release checklist runs: fmt, clippy, tests, smoke tests
- R7: Validates RELEASING.md publish order against cargo metadata dependency graph
- R8: Updates RELEASING.md if publish order is stale
- R9: Commits ALL version-bumped files in one commit (Cargo.toml, plugin.json, Cargo.lock, RELEASING.md if changed)
- R10: Creates dev→main PR with changelog body
- R11: Waits for human to merge PR before proceeding to tag
- R12: After human merges, pushes vX.Y.Z tag to trigger CI release
- R13: Agent NEVER merges the PR (project rule #7)

### Agent Docs Updates
- R14: MEMORY.md gets a Release Process section with version locations, publish order, Cargo.lock pitfall
- R15: CLAUDE.md release section references the skill and documents the Cargo.lock commit requirement
- R16: RELEASING.md corrected: publish order updated to include markymark-kernels, correct dependency order

## Success Criteria (MUST ALL BE TRUE)
- [ ] prepare-release SKILL.md exists at markymark-plugin/skills/prepare-release/SKILL.md
- [ ] Skill follows existing markdown-check SKILL.md format conventions
- [ ] RELEASING.md publish order matches: kernels → core → parser → index → lsp, mcp → cli
- [ ] MEMORY.md has Release Process section with version locations and known pitfalls
- [ ] CLAUDE.md references prepare-release skill in relevant sections
- [ ] All changes are read-only safe (no code changes, only docs and skill definition)

## Anti-Patterns (FORBIDDEN)
- ❌ NO automated PR merging (rule #7: human merges all PRs)
- ❌ NO skipping Cargo.lock in version bump commits (exact pitfall that motivated this work)
- ❌ NO hardcoded publish order in the skill (must derive from cargo metadata each time)
- ❌ NO tagging before PR is merged (tag goes on main after merge)
- ❌ NO version bump without running quality gates (fmt, clippy, test, smoke)
- ❌ NO modifying source code as part of this epic (read-only constraint while bugs worked in parallel)

## Approach
Create a local Claude Code plugin skill (SKILL.md) that guides agents through a conversational 4-phase release preparation workflow. The skill encodes the release process as structured instructions, with human checkpoints between phases. Simultaneously update MEMORY.md, CLAUDE.md, and RELEASING.md to formalize the process in agent memory.

## Architecture
- markymark-plugin/skills/prepare-release/SKILL.md — the skill definition
- docs/MEMORY.md — new Release Process section
- CLAUDE.md — reference to skill, Cargo.lock pitfall
- RELEASING.md — corrected publish order

## Design Rationale
### Problem
Release preparation has 3 known failure modes: (1) plugin.json version not bumped, (2) Cargo.lock not committed after workspace version bump, (3) publish order in docs is stale (missing markymark-kernels). Agents working autonomously on releases will hit these unless the process is encoded as a skill they follow.

### Research Findings
**Codebase:**
- v0.4.2 release required TWO commits: one for version bump (76fde10), one for Cargo.lock (324f744)
- markymark-kernels has zero regular deps but markymark-core depends on it — publish order is kernels-first
- RELEASING.md lists 6 crates in wrong order, actual workspace has 7
- git-cliff already configured at cliff.toml with conventional commit grouping
- Existing skill (markdown-check) provides format template

**External:**
- semver.org spec (docs/semver.md) — standard rules for version classification

### Approaches Considered

#### 1. Conversational multi-phase skill ✓
**What it is:** A SKILL.md that guides agents through 4 phases with human checkpoints between each. Agent assesses, human confirms, agent bumps, human reviews PR, human merges, agent tags.

**Chosen because:** Matches the project's human-in-the-loop philosophy. Prevents autonomous mistakes while still automating the tedious parts (commit classification, version bumping, Cargo.lock regen).

#### 2. Fully automated release script ❌
**Why we looked at this:** Could be faster for frequent releases.
**⚠️ REJECTED BECAUSE:** Violates rule #7 (no agent PR merges), removes human oversight from version decisions, and can't handle PR review triage. Risk of wrong version or broken release.
**🚫 DO NOT REVISIT UNLESS:** Project moves to automated CI-driven releases with no human review.

### Scope Boundaries
**In scope:** Skill creation, MEMORY/CLAUDE/RELEASING doc updates
**Out of scope:** crates.io publishing automation (stays manual per RELEASING.md), CI workflow changes, marketplace submission
