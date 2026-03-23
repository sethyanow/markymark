---
id: marky-bt3e
title: 'Refine ix3 for Tier 1: scope, three-path extraction, Zig consolidation'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---


## Purpose
Refinement blocker for marky-ix3. The ix3 epic has drifted since it was cut (Feb 16) due to three architectural shifts: Option H blob format, ExtractionRenderer solidification, and the expanded vision of ix3 as unified agent knowledge layer (not just cross-language bridging).

This task captures full handoff context for a fresh session to refine ix3 before implementation begins.

## Key Context (read ix3 notes from 2026-02-20 for full details)

### The Vision
Generated code docs (external markdown from rustdoc etc.) get dropped into markymark workspace. ix3 indexes all backtick code references. Agents query via standard LSP calls — workspaceSymbol, hover, findReferences — and get a unified answer across headings, code spans, and structured data. Tool stays indifferent to generated vs hand-written markdown. Reports issues via LSP diagnostics as it does today.

### Three-Path Extraction Problem
Code spans need to work across all three DocumentIndex construction paths:
1. **from_ast** (tree-sitter) — primary for MCP batch/add-root. extract.rs regex (~30 lines)
2. **from_scan** (md4c ScanBackend) — LSP hot path. ExtractionRenderer SpanType::code callbacks (already exist, currently ignored)
3. **from_blob** (Zig Document Engine) — future primary. Needs blob v2 with code_span_count field

### Zig Layer Consolidation
User wants to explore sinking ALL extraction concerns to Zig over time. ExtractionRenderer already handles headings + links. Tags, blocks, XML tags could follow. End state: extract.rs (Rust 862-line regex file) becomes compatibility shim, Zig owns all extraction. This aligns with Option H trajectory.

Questions for refinement:
- Should Tier 1 extraction live in Zig only (ExtractionRenderer) with a scan_code_spans() FFI, or also in extract.rs as fallback?
- What's the migration path for existing extractors (tags, blocks, frontmatter) to Zig?
- Does consolidating extraction in Zig change the fgl8 (extract.rs split) calculus? If extract.rs is going away, splitting it is wasted work.
- How does this affect IncrementalOverrides? If Zig owns extraction, Rust-side incremental merge may not need per-extractor overrides.

### Scope Recommendations (from analysis)
1. Drop fgl8 dependency — not blocking Tier 1
2. Narrow to Tier 1 only (backtick inline code spans) for first implementation pass
3. Defer confidence scoring to Tier 2/3
4. Make CodeSpanEntry::kind optional (None for Tier 1)
5. Add scan_code_spans() to ScanBackend trait
6. Add dedup by (identifier, uri) in RealmIndex
7. Note blob v2 as follow-on, not blocking from_scan-based Tier 1

### Codebase Entry Points
- ExtractionRenderer: zig/src/md4c/extraction_renderer.zig (SpanType::code at enterSpan line 180-204)
- ScanBackend trait: markymark-core/src/scanner.rs:108
- DocumentIndex types: markymark-index/src/document/types.rs
- from_scan: markymark-index/src/document/mod.rs:556
- from_blob: markymark-index/src/document/from_blob.rs
- RealmIndex: markymark-index/src/realm/mod.rs
- extract.rs: markymark-parser/src/extract.rs (862 lines)
- MCP search engine: markymark-mcp/src/engine/search.rs:29-59
- LSP workspace_symbol: markymark-lsp/src/server.rs:794-885
- LSP hover: markymark-lsp/src/server.rs:552-660
- SymbolAtPosition: markymark-lsp/src/state/navigation.rs:11-22

### Beads Context
- marky-ix3: epic with full design + 2026-02-20 drift notes
- marky-0mr: md4c parser (in progress, ix3 depends on it)
- marky-io3h: Option H Zig Document Engine (blob format)
- marky-fgl8: extract.rs split (may be deprioritized if Zig consolidation proceeds)
- marky-hwc: Knowledge Tool Plugins (blocked on ix3)
- marky-mkr: Agent Tooling (blocked on ix3)
- marky-qyf: Editor Distribution (blocked on ix3)
