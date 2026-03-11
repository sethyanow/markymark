---
id: marky-zsys
title: 'Engine Pipeline v2: incremental diffing, zero-copy blob, edit ranges'
status: open
type: feature
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-n7wx]
---


## Design

## Problem

Deferred items from Epic H (marky-io3h) brainstorming. Investigated 2026-02-19 and found low ROI while RealmIndex was the bottleneck. After RealmIndex v2 (marky-n7wx) optimizations, these may become the next bottleneck worth addressing.

## Deferred Items

### 1. Internal incremental diffing in engine.update()
- Currently: full md4c reparse on every update (~2.5ms at 50KB)
- Opportunity: diff previous headings/links against new, skip unchanged sections
- Constraint: md4c is streaming single-pass — no incremental parse possible. Diffing would be post-parse (compare stored vs new extracted data, skip blob rebuild if identical)
- Expected gain: skip blob serialization when content hash unchanged. ~0.5-1ms savings.

### 2. Zero-copy DocumentIndex borrowing from blob
- Currently: from_blob() copies text from blob pool into bumpalo arena (1-2ms at 50KB)
- Opportunity: DocumentIndex borrows directly from engine-owned blob, no arena copy
- Constraint: Breaks lifetime model. DocumentIndex<'blob> would need engine lifetime, which propagates to RealmIndex and all callers. Would require significant refactor of DocumentIndex, RealmIndex, LSP state.
- Expected gain: ~1-2ms at 50KB. High effort for moderate gain.

### 3. Edit range support in engine.update()
- Currently: engine.update(text) takes full text, no edit information
- Opportunity: engine.update(text, edit_ranges) could skip parsing unchanged regions
- Constraint: md4c doesn't support incremental parsing. Edit ranges could optimize: (a) content hash check (skip if unchanged), (b) blob rebuild (skip sections outside edit range), (c) slug caching (reuse slugs for unchanged headings)
- Expected gain: depends on how much slug/position computation can be skipped.

## Re-evaluation Criteria

Revisit after marky-n7wx is complete and marky-8d08 benchmarks show the engine pipeline is the dominant cost. If RealmIndex v2 reduces its overhead to <1ms, the engine's ~4ms becomes >80% of the hot path and these optimizations become worthwhile.

## Files
- zig/src/engine/document.zig (Zig engine internals)
- markymark-kernels/src/engine.rs (Rust FFI wrapper)
- markymark-index/src/document/from_blob.rs (blob deserialization)
- markymark-index/src/document/mod.rs (DocumentIndex lifetime model)
