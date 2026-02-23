# Research: Incremental md4c Block-Level Reparse

**Bead:** marky-tsds
**Date:** 2026-02-23
**Status:** Research complete, ready for brainstorm

---

## Problem Statement

markymark's Zig `DocumentEngine` does a full md4c reparse on every LSP `did_change` event.
The pipeline:

```
User types → LSP did_change(full text) → DocumentEngine.update(text)
  ├─ md4c full reparse (blocks.zig → inlines.zig → ExtractionRenderer callbacks)
  ├─ SIMD kernels (tags, block_ids, fence_map, token_estimate, content_hash)
  ├─ Post-processing (slugify, line_starts, positions)
  └─ Old state freed, new state installed
→ serializeState() → blob (lazy, on getBlob)
→ FFI → Rust from_blob() → DocumentIndex
→ LSP serves completions, hover, symbols
```

**Scaling problem:** md4c is O(N) single-pass. Measured at ~2.5ms/50KB, which means:

| File size | Estimated reparse time | User experience |
|-----------|----------------------|-----------------|
| 50KB | ~2.5ms | Imperceptible |
| 500KB | ~25ms | Fine with 75ms debounce |
| 2MB | ~100ms | Noticeable, borderline |
| 5MB | ~250ms | Laggy, editors struggle here too |
| 10MB | ~500ms | Unusable for real-time |

Most editors fall apart with multi-megabyte markdown. Making markymark handle them well is
a differentiator — "can we do this?"

---

## Current Architecture: blocks.zig Deep Dive

### The Block Phase State Machine

`processDoc()` in `blocks.zig` is the entry point. It's a simple loop:

```zig
pub fn processDoc(self: *Parser) Parser.Error!void {
    var pivot_line = Line{ .type = .blank };
    var off: OFF = 0;

    try self.enterBlock(.doc, 0, 0);

    while (off < self.size) {
        try self.analyzeLine(off, &off, &pivot_line, &line);
        try self.processLine(&pivot_line, &line, ...);
    }

    try self.endCurrentBlock();
    try self.buildRefDefHashtable();
    try self.leaveChildContainers(0);
    try self.processAllBlocks();  // walks block_bytes, fires inline parsing
    try self.leaveBlock(.doc, 0);
}
```

Two distinct phases:
1. **Block analysis** (`analyzeLine` + `processLine` loop) — classifies lines, builds `block_bytes`
2. **Block processing** (`processAllBlocks`) — walks `block_bytes`, fires inline parsing + renderer

### Parser State That Crosses Line Boundaries

The "parser snapshot" — everything `analyzeLine` reads from `self.*` at line start:

```
ParserSnapshot {
    // Container nesting (the hard part)
    containers: []Container,    // stack of active blockquotes/lists
    n_containers: u32,          // depth

    // Current leaf block
    current_block: ?usize,      // offset into block_bytes, or null
    pivot_line_type: LineType,   // previous line's type

    // Mode flags
    html_block_type: u8,        // 0 = none, 1-7 = active HTML block type
    fence_indent: u32,          // indentation of current code fence

    // List heuristics
    last_line_has_list_loosening_effect: bool,
    last_list_item_starts_with_two_blank_lines: bool,
}
```

Estimated size: container stack is max ~16 levels deep in practice (nested blockquotes +
lists). Each Container is ~24 bytes. Total snapshot: ~64-128 bytes typical, ~512 bytes worst case.

### Why Lines Aren't Independent

`analyzeLine` (500+ lines) does these cross-line-dependent operations:

1. **Container matching (lines 57-90):** Walks `containers[0..n_containers]`, checking if the
   current line continues each container (blockquote `>` prefix, list indentation). A single
   added/removed `>` changes `n_parents` for all subsequent lines.

2. **Pivot line continuation:** `effective_pivot_type` starts as `pivot_line.type`. Fenced code
   continuation (line 113), HTML block continuation (line 139), indented code continuation
   (line 227), and lazy text continuation (line 475) all depend on what the previous line was.

3. **Setext retrospection:** A `---` or `===` line (line 237) converts the *previous paragraph*
   into a heading. The paragraph block is retroactively changed from `.p` to `.h`.

4. **HTML block types 1-5** persist across lines until their specific end condition is found
   (e.g., `</script>` for type 1). Type is stored in `self.html_block_type`.

5. **Loose/tight list detection (line 537):** A blank line between list items makes the list
   "loose." This is retroactively patched onto the list opener in `block_bytes` when the list
   closes (`leaveChildContainers`, containers.zig:75).

### block_bytes: The Flat Output Buffer

Block analysis writes to `block_bytes` — a flat `ArrayListUnmanaged(u8)` containing:
- `BlockHeader` structs (aligned, 16 bytes each): block_type, flags, data, n_lines
- `VerbatimLine` arrays immediately after each header: beg, end, indent per line

Container openers/closers are also BlockHeaders with `BLOCK_CONTAINER_OPENER/CLOSER` flags.

`processAllBlocks` walks this buffer linearly, firing enter/leave block callbacks and
running inline parsing on leaf block content. It's a second pass over the same data.

**Key limitation:** This is append-only. There's no way to splice in modified blocks without
rebuilding everything after the edit point.

---

## Why the Old Rust Incremental Didn't Help

The old system (tag `fixed-incremental`, ~2100 lines, deleted in marky-n78f) worked at the
**extractor merge** layer:

```
Full parse → 5 extractors run → on edit, re-run affected extractors → merge old + new entries
```

This was wrong because:
1. The full parse still happened every time (tree-sitter, then regex extractors)
2. The merge logic was the buggiest code in the project (coordinate-space bugs, gap detection,
   boundary heuristics — marky-g0dn, marky-wjf, marky-v8y/kvr)
3. It solved "merge results faster" instead of "parse less"

The right approach is making the parser itself skip unchanged regions.

---

## Proposed Hybrid: SIMD Boundaries + Chunk Tree

### Layer 1: SIMD Structural Boundary Scan

**Goal:** Identify positions in the document where parser state is fully determined regardless
of what came before — "guaranteed convergence points."

**Safe boundaries (strongest to weakest):**

| Boundary | Why it resets state | Detection |
|----------|-------------------|-----------|
| Blank line outside any fenced code block | Container stack must be 0 (no lazy continuation across blank lines at indent 0), pivot becomes .blank | SIMD: track fence toggles, find `\n\n` |
| ATX heading (`# `) at indent 0 | Self-contained single-line block, forces new block | `heading_scan` kernel exists |
| Thematic break (`---`/`***`/`___`) | Self-contained, forces new block | Detectable in one SIMD pass |
| Opening code fence at indent 0 | Known state transition (pivot → .fencedcode) | `fence_map` kernel exists |

**Caveat:** Blank lines inside containers (blockquotes, lists) are NOT safe boundaries because
the container stack persists. Only truly top-level blank lines reset everything.

**Implementation:** A new `boundary_scan` SIMD kernel that:
1. Tracks fenced code state (backtick/tilde count toggles)
2. Identifies blank lines (`\n\n` or `\n\r\n`) outside fences
3. Returns an array of byte offsets representing safe cut points

Existing `fence_map` kernel already does step 1. Could be extended or composed.

### Layer 2: Chunk Tree with Cached Parser State

**Data structure:** A balanced tree (B-tree, skip list, or even a sorted Vec for simplicity)
where each node represents a **chunk** — the text between two consecutive safe cut points.

```zig
const Chunk = struct {
    // Source location
    byte_start: u32,
    byte_end: u32,
    line_start: u32,       // first line number in chunk
    line_count: u16,       // lines in this chunk

    // Cached parser state
    entry_state: ParserSnapshot,  // state at chunk start
    exit_state: ParserSnapshot,   // state after parsing this chunk

    // Output
    block_output: []u8,    // block_bytes segment for this chunk
    ref_defs: []RefDef,    // link reference definitions found in this chunk

    // Quick change detection
    content_hash: u64,     // for fast "did anything change?" check
};
```

**On initial parse:** Run full `processDoc`, but snapshot parser state at each safe boundary.
Store chunks. This adds ~1ms overhead for the snapshots on a 5MB file (negligible).

**Memory:** ~200 bytes per chunk. A 5MB file with blank lines every ~50 lines → ~2000 chunks
→ ~400KB of chunk metadata. Acceptable.

### Layer 3: Edit Propagation

On `did_change` with edit range `[edit_start, old_end) → new_text`:

```
1. SIMD boundary_scan on the edited region (+ context) to find new safe cut points
2. Find chunks overlapping [edit_start, old_end) in the tree          — O(log N)
3. Merge/split chunks if safe cut points changed                      — O(affected)
4. For each affected chunk:
   a. Get entry_state from the chunk before it (or initial state if first)
   b. Re-run analyzeLine + processLine on this chunk's text
   c. Capture new exit_state
   d. Compare to next chunk's cached entry_state
   e. If match → STOP (convergence). If mismatch → continue to next chunk.
5. Rebuild block_bytes from chunk segments (concatenate or use rope)
6. Run processAllBlocks only on changed block_bytes regions
```

**Convergence analysis:**

| Edit type | Typical convergence | Why |
|-----------|-------------------|-----|
| Type in a paragraph | 1 chunk (immediate) | No structural change, exit state matches |
| Add/remove a blank line | 1-2 chunks | May merge/split chunks but state converges at next boundary |
| Add `>` (blockquote) | Chunks until next blank line at indent 0 | Container stack propagates through nested content |
| Add `` ``` `` (code fence) | Chunks until matching close fence or next boundary | Fence state propagates |
| Add `---` (thematic break) | 1-2 chunks | Self-contained, minimal propagation |

Worst case (pathological): a file with no blank lines and a `>` added at line 1 → full reparse.
But such files are extremely rare in practice.

---

## Open Design Questions (Brainstorm These)

### Q1: Chunk Granularity — Fixed vs Semantic

**Option A: Fixed-size (sqrt decomposition)**
- Divide into √N-line blocks (~316 lines for 100K lines)
- Pro: predictable chunk count, simple tree
- Con: chunks may split mid-paragraph/mid-fence, complicating state restoration

**Option B: Semantic boundaries (safe cut points)**
- Chunks at blank-outside-fence boundaries
- Pro: convergence is natural (every chunk boundary is a convergence point by definition)
- Con: variable chunk sizes, some files may have very few boundaries (one huge chunk)

**Option C: Hybrid** — semantic boundaries preferred, but if a chunk exceeds a max size
(e.g., 500 lines), insert synthetic checkpoints and store full parser snapshots there.

Leaning: **Option C**. Semantic boundaries give free convergence. Size cap prevents degenerate
single-chunk scenarios.

### Q2: block_bytes Structure

Currently flat append-only `ArrayListUnmanaged(u8)`. Needs to support splicing for incremental.

**Options:**
- **Segmented buffer** — each chunk owns its own `[]u8` segment. `processAllBlocks` iterates segments.
- **Rope / B-tree of bytes** — arbitrary insert/delete/replace at any offset.
- **Rebuild on demand** — keep per-chunk segments, concatenate into flat buffer only when
  `processAllBlocks` needs it. Simple, but O(N) concatenation.

Leaning: **Segmented buffer**. Each chunk owns its block_bytes. `processAllBlocks` already
walks linearly — it can walk segments sequentially without concatenation.

### Q3: Link Reference Definitions

`buildRefDefHashtable()` currently runs after ALL blocks are analyzed. It walks all paragraph
blocks, consuming ref def lines from their start. This is inherently global — a ref def in
chunk 1 affects link resolution in chunk 500.

**Options:**
- **Per-chunk ref def extraction** — each chunk tracks its own ref defs. Global ref_defs
  HashMap rebuilt from chunk contributions. On edit, only reprocess affected chunk's ref defs.
- **Two-pass approach** — block analysis is incremental, ref def extraction remains global
  but fast (just scan paragraph-start lines for `[label]:` pattern).

Leaning: **Per-chunk extraction with merge**. Ref defs are rare in practice. The merge
step is cheap.

### Q4: processAllBlocks Scope

Can we restrict `processAllBlocks` (inline parsing + renderer callbacks) to only the changed
chunks?

**Constraint:** Inline parsing is stateless across blocks. Each leaf block is parsed
independently. So yes — if block_bytes is segmented, we can run `processAllBlocks` only
on changed segments.

**Caveat:** The ExtractionRenderer accumulates results in ArrayLists. Changed chunks need
their old extraction results removed and new ones inserted. This is the same
"merge old + new" problem the old Rust incremental had, but now it's at the block level
(coarser granularity, much simpler).

### Q5: API Surface

```zig
// Existing (kept for MCP batch / initial load)
pub fn update(self: *DocumentEngine, text: []const u8) Error!void

// New: incremental update with edit range
pub fn updateRange(
    self: *DocumentEngine,
    text: []const u8,          // full new text
    edit_start: u32,           // byte offset of edit start
    old_end: u32,              // byte offset of old text end
    new_end: u32,              // byte offset of new text end
) Error!void
```

LSP `did_change` already provides these ranges (it's what tree-sitter's `InputEdit` uses).
The Rust side (`engine.update()` in `state/mod.rs`) currently passes full text — would need
to forward the edit ranges from `apply_document_changes`.

### Q6: ParserSnapshot Comparison

Convergence detection requires comparing two ParserSnapshot values. What constitutes "equal"?

**Must match:**
- `n_containers` and each container's `ch` + `contents_indent` + `mark_indent`
- `pivot_line.type`
- `html_block_type`
- `fence_indent` (only if pivot is fenced code)
- `current_block != null` (whether a leaf block is open)

**Can differ (don't affect subsequent lines):**
- `current_block` offset value (different position in block_bytes)
- `last_line_has_list_loosening_effect` (only affects current list, which already closed)
- Container `is_loose` (retroactive, doesn't affect line classification)
- Container `block_byte_off` (output position, not parser state)

This needs careful validation — incorrect comparison means either false convergence (wrong
output) or false divergence (unnecessary reparsing).

### Q7: Convergence Guarantees

**When can we prove convergence is correct?**

If at chunk boundary B, the new parse produces the same ParserSnapshot as the cached snapshot,
then all subsequent `analyzeLine` calls will produce identical output because:
- `analyzeLine` is a pure function of (text, parser_state)
- The text after boundary B is unchanged
- The parser state at B is identical

**Edge case:** `buildRefDefHashtable` can change which paragraphs are consumed. If an edit
adds or removes a ref def, paragraphs in OTHER chunks could be affected (they might now
have a valid/invalid `[ref]` reference). But this only affects inline parsing, not block
structure. The block phase output is stable.

So: **convergence is correct for block analysis.** The inline phase (processAllBlocks) may
need to re-run on blocks containing references to changed ref defs, but that's a cheaper
secondary pass.

---

## Existing Infrastructure Inventory

| Component | Location | Reusable for | |
|-----------|----------|------------|---|
| `fence_map` kernel | `zig/src/kernels/fence_map.zig` | Identifying fenced code ranges (Layer 1 boundary detection) |
| `heading_scan` kernel | `zig/src/kernels/heading_scan.zig` | ATX heading boundaries |
| `block_scan` kernel | `zig/src/kernels/block_scan.zig` | Block-level markers |
| `content_hash` kernel | `zig/src/kernels/content_hash.zig` | Fast chunk change detection |
| `token_estimate` kernel | `zig/src/kernels/token_estimate.zig` | Not directly, but similar SIMD patterns |
| `DocumentEngine` | `zig/src/engine/document.zig` | Container for incremental state |
| LSP `apply_document_changes` | `markymark-lsp/src/state/mod.rs` | Already computes edit ranges |

---

## Related Work and Patterns

### DSA Patterns

- **Sqrt decomposition:** Array divided into √N blocks. Per-block aggregates rebuilt on update.
  O(√N) per operation. Simplest to implement. Our chunks are the "blocks."
- **Segment tree with lazy propagation:** O(log N) query/update. Each node stores aggregate.
  Lazy: defer recomputation until needed. More complex but better asymptotics.
- **Finger tree with monoidal annotations:** Functional. Split/concat at edit point, annotations
  recomputed up the spine. Natural for rope-like text structures.
- **Skip list with state checkpoints:** Probabilistic. Checkpoints at random intervals.
  Good amortized behavior. Simpler than balanced trees.

### Prior Art in Parsers

- **tree-sitter:** Incremental CST. Nodes store byte ranges. On edit, identifies minimal
  reparse set via range overlap. GLR parser can resume from any point. Most sophisticated
  but requires a different parser architecture than md4c's.
- **Lezer (CodeMirror 6):** Incremental LR parser. Reuses unchanged tree nodes. Stores
  "reuse markers" at node boundaries.
- **Roslyn (C# compiler):** Incremental red-green trees. Green nodes are immutable, shared
  across versions. Red nodes provide parent pointers.
- **No known md4c fork does incremental.** This would be novel.

---

## Next Steps

1. **Brainstorm session** — Resolve the 7 open design questions above
2. **Prototype** — ParserSnapshot struct + comparison semantics (smallest useful unit)
3. **Benchmark** — Measure actual reparse time at 1MB, 5MB, 10MB to validate the problem
4. **boundary_scan kernel** — Extend fence_map or build new SIMD kernel for safe cut points
5. **Chunk tree MVP** — Implement on a branch, measure convergence behavior on real documents
