---
id: marky-7pw
title: '[EPIC] Roadmap Research: AI, Advanced Markdown, Ecosystem'
status: open
type: epic
priority: 3
owner: sethyanow@users.noreply.github.com
---

## Goal
Research and document feasibility for three future feature tracks. Produce prioritized proposals with effort estimates, not implementations.

## Requirements (IMMUTABLE)
- Each track produces a design document in docs/roadmap/
- Every proposal includes: problem statement, approach, effort (T-shirt: S/M/L/XL), dependencies, risks
- No implementation — outputs are documents and follow-up beads issues
- Marksman feature gaps (codeAction, foldingRange) must be addressed in Track B
- At least one follow-up implementation epic created from highest-priority proposals

## Tracks
### Track A: AI-Augmented Features (highest innovation potential)
- Semantic search across markdown workspaces
- Auto-linking suggestions (detect unlinked references)
- Content quality scoring
- AI-powered completions (context-aware heading/tag suggestions)
- Smart refactoring (heading reorganization, link updates)

### Track B: Advanced Markdown Intelligence (closest to current work)
- Cross-workspace references and navigation
- Template/snippet system for common patterns
- Link rot detection (dead link checking)
- TOC generation and management
- Folding ranges (marksman gap)
- Code actions (marksman gap)

### Track C: Ecosystem Integration (broadest reach)
- Obsidian plugin (use markymark as Obsidian's markdown intelligence)
- Logseq integration
- GitHub Actions (CI markdown linting/validation)
- VS Code extension
- Neovim/Helix LSP client configs

## Deliverables
- docs/roadmap/track-a-ai-features.md
- docs/roadmap/track-b-advanced-markdown.md
- docs/roadmap/track-c-ecosystem.md
- docs/roadmap/marksman-gap-analysis.md
- At least 1 follow-up implementation epic in beads

## Success Criteria
- [ ] All 3 track documents written with prioritized proposals
- [ ] Marksman gap analysis complete
- [ ] Each proposal has effort estimate and dependency analysis
- [ ] At least 1 implementation epic created from research
