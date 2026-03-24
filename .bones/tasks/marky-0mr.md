---
id: marky-0mr
title: 'G: Zig md4c streaming parser as fast-path replacement for tree-sitter'
status: open
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-77i]
---




























## Problem

Tree-sitter-md dual grammar is the architectural bottleneck: block parse (5.7ms) + 500 inline FFI calls × 13μs = 7.1ms inline = 12.8ms total. Our extractors are 46μs (0.3%). Incremental tree-sitter yields only 1.3x (ceiling). Option D (vendor + skip) caps at ~2.8x. The problem is structural — too many FFI round-trips — not tunable.

## Research Finding: Bun's Zig md4c Port

Bun (MIT license) contains a hand-written Zig port of md4c in src/md/ (~8,274 lines, 15 files). md4c is the same parser that powers GitHub's markdown rendering.

Architecture:
- Single-pass streaming with callback vtable (Renderer: enterBlock/leaveBlock/enterSpan/leaveSpan/text)
- CommonMark + GFM extensions (tables, strikethrough, tasklists, wiki-links, LaTeX math)
- No dual grammar, no FFI boundary, single pass over []const u8
- ArrayListUnmanaged with allocator parameter (no arena needed)
- Proven lineage: md4c referenced throughout source comments

Key files in Bun repo (src/md/):
- parser.zig (285L): Parser struct, init/deinit, public API, method delegation
- blocks.zig (865L): processDoc, analyzeLine, processLine — block-level parsing
- inlines.zig (746L): emphasis delimiters, processLeafBlock, processInlineContent
- line_analysis.zig (527L): ATX headers, setext, fences, HR, HTML blocks, tables
- links.zig (527L): bracket links, wiki-links, autolinks, ref-links
- types.zig (387L): BlockType/SpanType/TextType enums, Renderer vtable, OFF=u32
- html_renderer.zig (714L): HTML output renderer (reference implementation)
- ref_defs.zig (351L): reference definition hashtable
- containers.zig (192L): block container push/pop/enter/leave
- entity.zig (2164L): HTML entity lookup tables
- helpers.zig (482L): character classification, indentation
- autolinks.zig (300L): URL/email autolink scanning
- unicode.zig (477L): Unicode folding (ported from md4c)
- render_blocks.zig (153L): block-level rendering delegation
- root.zig (104L): public Options, renderToHtml, renderWithRenderer

## Solution: Option G

Replace tree-sitter as the primary parse path with a Zig md4c streaming parser. Keep tree-sitter only for lazy AST when LSP features (hover, goto-def) need full tree structure.

On did_change:
1. Update text buffer immediately
2. Run Zig md4c parser with custom Renderer vtable that feeds directly into extractors
3. Index updated — sub-millisecond for full 50KB doc (md4c benchmarks ~200MB/s)

On hover/goto-def (if AST needed):
4. Lazy tree-sitter parse (same as Option E concept)

## Why Better Than Options D/E

vs Option D (vendor tree-sitter-md, skip inlines): D caps at ~2.8x. G eliminates the dual grammar entirely.
vs Option E (SIMD scan fast path): SIMD scan is regex-level correctness, misses edge cases. md4c is CommonMark-compliant, handles emphasis, tables, ref-links, code blocks. md4c Renderer vtable maps directly to ScanBackend trait.
vs tree-sitter incremental: 1.3x ceiling. md4c full reparse is faster than tree-sitter incremental because single-pass, no FFI overhead, no dual grammar.

## What We Already Have

- Zig build infrastructure works (BRZA kernels, build.rs invokes zig build)
- FFI bridge pattern proven (call_scan_ffi<T>, repr(C) structs, safe_slice)
- ScanBackend trait designed (marky-v8g) — md4c Renderer maps to it
- Extractor architecture already decoupled from parser

## Implementation Plan

1. Copy Bun src/md/ into zig/ workspace (MIT license, ~8K lines)
2. Strip Bun-specific dependencies (bun.JSError → standard errors, bun.StackCheck → manual guard)
3. Write custom Renderer vtable emitting extractor-compatible types with byte offsets (headings, links, wiki-links, code blocks)
4. FFI bridge: one call in, structured extraction results out (same pattern as BRZA)
5. Wire into ScanBackend trait for fast path
6. Keep tree-sitter for lazy AST (hover/goto-def needs)
7. Benchmark: validate sub-1ms on 50KB doc, compare with tree-sitter 12.8ms baseline

## Risks

- Maintenance burden: we'd own a fork of md4c in Zig (but stable spec, Bun maintains upstream)
- XML tags are NOT markdown — still need our custom XML tag extractor alongside
- 200MB/s claim needs validation with extraction overhead
- Lazy AST adds state complexity (track stale AST in LSP server)

## Supersedes

This effectively replaces Options D and E if successful. F (debounce) remains complementary.

## Log

- [2026-03-23T13:53:48Z] [Seth] Review-implementation APPROVED. All 24/25 children closed. Implementation verified: md4c Zig port, FFI bridge, Md4cScanBackend, tree-sitter retained, XML tag extraction, benchmarks. 1416 Rust tests pass, Zig tests pass, clippy clean. Only marky-8d08 (cross-env benchmarks) remains open — requires manual hardware testing, not an implementation gap. Epic closed per user authorization.
