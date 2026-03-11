---
id: marky-syx
title: 'E: BRZA-powered lazy AST — decouple index update from tree-sitter parse'
status: open
type: feature
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-0jz, marky-7dq]
parent: marky-77i
---




## Problem
Even with F (debounce) and D (selective inline skip), per-parse speedup caps at ~2.8x. The 10x target requires decoupling the fast path (index update) from the slow path (tree-sitter AST rebuild).

## Concept
On did_change:
1. Update text buffer immediately
2. SIMD-scan ONLY the changed text region via BRZA ScanBackend
3. Merge scan results with position-adjusted old index data
4. Defer full tree-sitter parse until a request actually needs the AST (hover, goto-def, document-symbol)

This makes the typing hot path microsecond-scale (SIMD scan of changed region only), while AST-dependent operations pay the parse cost on demand.

## Prerequisites
- marky-v8g (TreeSitterScanBackend) must be complete — wraps current extraction into ScanBackend trait
- F+D should be landed and validated first
- ScanBackend integration into apply_document_changes path

## Key Architecture
- markymark-core/src/scanner.rs: ScanBackend trait
- markymark-index/src/document/mod.rs:377: DocumentIndex::from_scan()
- markymark-lsp/src/state/mod.rs: wire ScanBackend into apply_document_changes

## Expected Impact
Potentially 10x+ for typing UX. Index updates become microsecond-scale. AST rebuilds happen lazily, amortized across requests.

## Risks
- Lazy AST adds complexity to LSP state management (must track whether AST is stale)
- Some LSP features (diagnostics) may need AST — must define which operations are scan-only vs AST-required
- BRZA scan coverage vs tree-sitter AST coverage: scan may miss edge cases that AST catches
