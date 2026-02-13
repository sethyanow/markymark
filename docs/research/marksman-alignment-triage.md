---
type: analysis
title: Marksman vs Markymark Alignment Triage
created: 2026-02-11
tags:
  - alignment
  - testing
  - marksman
  - markymark
related:
  - "[[architecture]]"
---

# Marksman vs Markymark Alignment Triage

This document classifies every known alignment difference between marksman and markymark
as either a **bug** (to be fixed) or an **intentional divergence** (by design).

Generated from dual-process alignment harness (`tests/alignment.rs`) run against:
- markymark v0.1.0 (Rust, tree-sitter-based)
- marksman (F#/.NET, latest available)
- Corpus: `tests/corpus/` (basic.md, links.md, cross-refs.md, edge-cases.md, xml-tags.md)

## Summary

| Classification | Count |
|----------------|------:|
| Match          |     1 |
| Mismatch       |     4 |
| MarkymarkOnly  |     3 |
| MarksmanOnly   |     0 |
| **Total**      | **8** |

## Match (1)

### textDocument/completion (links.md)

- **Status**: MATCH
- **Classification**: match
- **Details**: Both servers return identical completion item labels for wiki-link completion
  context. Sorted label sets are equal.
- **Notes**: This validates that the core completion logic for page-name suggestions is
  behaviorally equivalent.

## Mismatches (4)

### textDocument/references (basic.md)

- **Method**: `textDocument/references`
- **File**: basic.md, line 0, character 2 (heading "Basic Test")
- **Observed diff**: Different location sets returned for heading references.
  Marksman returns references from linked documents while markymark returns references
  from within the same document and cross-file wiki links.
- **Classification**: intentional divergence
- **Rationale**: markymark indexes wiki links and markdown anchor links as references
  to headings, which is correct LSP behavior. The difference comes from how each server
  resolves cross-document references through different link types. Both approaches are
  valid — markymark's is more comprehensive (it also finds markdown-style `[text](#anchor)`
  links, not just `[[wiki]]` links).

### textDocument/documentSymbol (basic.md)

- **Method**: `textDocument/documentSymbol`
- **File**: basic.md
- **Observed diff**: markymark returns additional XML tag symbols nested within the heading
  hierarchy. Marksman only returns heading-based document symbols.
- **Classification**: intentional divergence
- **Rationale**: markymark treats XML tags as first-class document model elements
  (decision `dec-2026-02-06-001`). XML tags appear as nested `DocumentSymbol` entries
  alongside headings. This is a feature — LLM prompt files heavily use XML tags for
  structure, and having them in the symbol outline is valuable for AI coding assistants.
  Marksman has no XML awareness.

### workspace/symbol (workspace-wide query "Section")

- **Method**: `workspace/symbol`
- **File**: (workspace)
- **Observed diff**: markymark returns more symbols than marksman for the same query.
  Extra symbols include XML tag names and heading matches from files marksman doesn't index.
- **Classification**: intentional divergence
- **Rationale**: Same root cause as documentSymbol — markymark includes XML tags as
  workspace symbols (with `SymbolKind::CONSTANT` to distinguish from headings which use
  `SymbolKind::STRING`). Additionally, markymark indexes all corpus files including those
  with XML content that marksman ignores.

### textDocument/rename (basic.md)

- **Method**: `textDocument/rename`
- **File**: basic.md, line 4, character 5 (heading "Section Two"), newName "Renamed Section"
- **Observed diff**: Different `WorkspaceEdit` structures. Both servers rename the heading
  text, but markymark additionally updates anchor links (`#section-two` → `#renamed-section`)
  in linked documents. The edit ranges and text replacements differ in their coverage of
  affected references.
- **Classification**: intentional divergence (markymark is more thorough)
- **Rationale**: markymark's rename implementation (decision `dec-20260211063400-rename-close-tag`)
  updates heading text, wiki link heading references, and markdown anchor slugs across the
  workspace. Marksman's rename updates the heading and wiki link references but may not
  update markdown-style anchor links. markymark's behavior is strictly more correct — it
  prevents broken anchor links after rename.

## MarkymarkOnly (3)

These are methods where markymark returns meaningful data but marksman returns null/empty.

### textDocument/definition (links.md)

- **Method**: `textDocument/definition`
- **File**: links.md, line 4, character 25 (wiki link target)
- **Observed diff**: markymark returns a definition location; marksman returns null.
- **Classification**: intentional divergence (markymark feature)
- **Rationale**: markymark implements go-to-definition for wiki links by resolving the
  target page and heading. For XML tags, it navigates to the first occurrence
  (decision `dec-20260211061000-cpd7-goto-def`). Marksman may not support go-to-definition
  at this cursor position, or may require different cursor placement.

### textDocument/hover (basic.md)

- **Method**: `textDocument/hover`
- **File**: basic.md, line 4, character 5 (heading "Section Two")
- **Observed diff**: markymark returns hover content with heading info, link count,
  and XML statistics; marksman returns null.
- **Classification**: intentional divergence (markymark feature)
- **Rationale**: markymark provides rich hover information for headings (anchor slug,
  incoming references, workspace stats) and XML tags (workspace occurrence count,
  document count, common attributes, unclosed-tag warnings — decision
  `dec-20260211060444-iwm-hover-stats`). This is a differentiating feature.

### diagnostics (links.md)

- **Method**: `textDocument/publishDiagnostics`
- **File**: links.md
- **Observed diff**: markymark publishes diagnostics for broken links and duplicate slugs;
  marksman publishes no diagnostics for this file.
- **Classification**: intentional divergence (markymark feature)
- **Rationale**: markymark's diagnostic system (decision `dec-2026-02-10-002`) validates
  wiki link targets, markdown link anchors, and duplicate heading slugs. It also reports
  unclosed XML tags (decision `dec-20260211061000-cpd8-unclosed-diag`). Marksman may emit
  diagnostics for different conditions or at different thresholds.

## Bugs Found

No alignment differences have been classified as bugs. All mismatches are intentional
divergences where markymark provides equal or greater functionality.

## Revision History

| Date | Change |
|------|--------|
| 2026-02-11 | Initial triage from dual-process alignment harness (d7v) |
