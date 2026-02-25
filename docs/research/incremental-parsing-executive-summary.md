# Incremental Parsing: Executive Summary

**Research Completed:** 2026-02-23
**Status:** Ready for brainstorm and architecture decisions
**Audience:** markymark team for Phase B planning

---

## What Actually Works in Production

### 1. tree-sitter (✓ Production)
- **How:** CST nodes store byte ranges. Edits trigger O(log N) range overlap tests.
- **Nodes outside edit range:** Reused (literally same Arc/Rc objects in memory)
- **Result:** ~99% node sharing on typical edits
- **Latency:** <5ms incremental updates on 100KB files
- **Used by:** Vim, Neovim, Helix, GitHub web editor
- **For markdown:** Possible but requires grammar (not always available)

### 2. Lezer (✓ Production)
- **How:** LR parser with context-aware node reuse. Context hashes stored in tree.
- **Incremental:** If surrounding context (indentation, nesting) matches, reuse old node and subtree
- **Latency:** <5ms incremental updates on CodeMirror/Obsidian
- **Used by:** Obsidian, CodeMirror 6, many web editors
- **For markdown:** Native incremental markdown parser available
- **Downside:** Complex LR machinery, non-trivial to port

### 3. Rope + SIMD Boundaries (✓ Production)
- **How:** Text stored in rope (B-tree of chunks). SIMD scans identify safe reparse boundaries.
- **Safe boundaries:** Blank lines, ATX headings, thematic breaks, code fence opens
- **Reparse scope:** Only chunks between boundaries, O(√N) cost typical
- **Latency:** <1ms file open, <5ms incremental on 100KB
- **Used by:** Zed, Xi, Helix (in combination with tree-sitter)
- **For markdown:** Excellent fit, markymark already has SIMD kernels

### 4. Roslyn Red-Green Trees (✓ Production, complex)
- **How:** Green tree (immutable, no parent refs, built bottom-up). Red tree (façade, parent refs, top-down).
- **Incremental:** On edit, rebuild ~O(log N) green nodes. Red nodes discarded and rebuilt on next access.
- **Result:** 99% green tree node sharing
- **Latency:** Sub-millisecond on typical C# edits
- **Downside:** Heavy machinery, designed for compiled languages
- **Not recommended for markdown**

---

## What DOESN'T Exist

### Incremental Markdown Parsers (Comprehensive Survey)
| Parser | Implementation | Incremental | Notes |
|--------|-------|-------------|-------|
| **md4c** | C reference | ✗ | Full reparse always. markymark's basis. |
| **cmark** | C reference | ✗ | Same as md4c. |
| **cmark-gfm** | C GFM variant | ✗ | No incremental. |
| **pulldown-cmark** | Rust | ✗ | Pull parsing is elegant but no LSP delta support. |
| **commonmark.js** | JavaScript | ✗ | Full reparse. |
| **marked.js** | JavaScript | ✗ | Full reparse. |
| **Lezer markdown** | Lezer-based | ✓ YES | Only production incremental markdown parser. |

**Conclusion:** No md4c fork implements incremental parsing. This is an opportunity for markymark.

---

## Why Block-Level Incremental is Feasible for Markdown

### Safe Boundaries (Guaranteed Convergence Points)

These positions always produce **deterministic parser state** regardless of prior context:

1. **Blank line outside fenced code** → Container stack resets to 0
2. **ATX heading (`# `)** at indent 0 → Self-contained, forces new block
3. **Thematic break (`---`/`***`/`___`)** → Self-contained state transition
4. **Code fence opener** at indent 0 → Known state transition

**Why it works:** CommonMark spec mandates container stack is deterministic after these boundaries. No retroactive state changes possible.

### CommonMark Two-Phase Parsing Supports Chunking

- **Phase 1 (Block structure):** Process lines sequentially. Can be incremental per chunk.
- **Phase 2 (Inline parsing):** Stateless per-block. Can be parallelized.
- **Link ref defs:** Found in phase 1, used in phase 2. Can extract per-chunk and merge.

**Result:** Phase 1 can be incremental. Phase 2 stays fast and stateless.

---

## Recommended MVP: SIMD Boundaries + Sqrt Chunk Tree

### Architecture

1. **Layer 1: SIMD Boundary Scan**
   - Input: Full markdown text
   - Output: Byte offsets of safe convergence points (blank lines, headings, etc.)
   - Cost: 100-200 MB/s (negligible overhead)
   - Reusable: Existing `fence_map` kernel extended, new `boundary_scan` kernel

2. **Layer 2: Chunk Tree (sqrt decomposition)**
   ```zig
   Chunk {
       byte_start, byte_end,
       entry_state: ParserSnapshot,      // state at start
       exit_state: ParserSnapshot,       // state after parsing
       block_output: []u8,               // block_bytes segment
       content_hash: u64,                // for change detection
   }
   ```
   - Stored in sorted Vec by byte_range
   - Per-chunk snapshots at safe boundaries

3. **Layer 3: Edit Propagation**
   - Find affected chunk(s)
   - Reparse from `entry_state`
   - If new `exit_state == next chunk's entry_state`, STOP (convergence reached)
   - Else reparse next chunk. Repeat until convergence.

### Performance Model

| Edit Type | Typical Propagation | Why |
|-----------|-------------------|-----|
| Type in paragraph | 1 chunk | No structural change, exit state matches |
| Add/remove blank line | 1-2 chunks | Chunk boundaries may shift but converge locally |
| Add `>` (blockquote) | Up to next blank line | Container stack propagates, stops at blank |
| Add `` ``` `` (fence) | Up to close fence or next boundary | Fence state propagates deterministically |
| Worst case | Full reparse | Pathological (no blank lines in file) |

**Expected speedup:** 3-5x on typical edits, 10x+ on structural edits at 5MB scale.

---

## Implementation Roadmap (Brainstorm Decisions)

### Q1: Chunk Granularity
- **Option A:** Fixed size (√N decomposition) — predictable, simple
- **Option B:** Semantic (safe boundaries) — natural convergence, variable size
- **Option C:** Hybrid — semantic preferred, size cap 500 lines, synthetic checkpoints beyond

**Recommendation:** Option C (hybrid semantic).

### Q2: block_bytes Structure
- **Segmented:** Each chunk owns []u8. Walk segments sequentially in processAllBlocks.
- **Rope/Rebuild:** Concatenate on demand or rebuild from segments.

**Recommendation:** Segmented (natural composition with chunk tree).

### Q3: Link Ref Defs
- **Per-chunk extraction:** Each chunk tracks ref defs, merge globally.
- **Global fast scan:** Ref defs rare; scan all paragraphs once after phase 1.

**Recommendation:** Per-chunk with merge (cleaner design).

### Q4: API Design
```zig
// Existing (keep for MCP)
pub fn update(self: *DocumentEngine, text: []const u8) Error!void

// New: forward edit ranges from LSP
pub fn updateRange(
    self: *DocumentEngine,
    text: []const u8,
    edit_start: u32,
    old_end: u32,
    new_end: u32,
) Error!void
```

**Recommendation:** Both. Incremental opt-in, existing path unchanged.

### Q5: ParserSnapshot Comparison
**Must match:** `n_containers`, each container's ch/indent, `pivot_line.type`, `html_block_type`
**Can differ:** `current_block` offset, `last_line_has_list_loosening_effect`

**Recommendation:** Explicit field-by-field comparison (no heuristics).

### Q6-Q7: Convergence Guarantees
- **Proven correct:** If exit_state == next entry_state, block phase output is stable.
- **Inline phase:** May need secondary pass for refs to changed ref defs (but block structure is fixed).

**Recommendation:** Convergence is sound for block phase. Document edge cases carefully.

---

## Reuse of Existing Infrastructure

| Component | Status | Reusable For |
|-----------|--------|-------------|
| `fence_map.zig` kernel | Existing | Layer 1 boundary detection (fenced code ranges) |
| `heading_scan.zig` kernel | Existing | ATX heading boundaries |
| `block_scan.zig` kernel | Existing | Block-level marker detection |
| `content_hash.zig` kernel | Existing | Per-chunk change detection |
| `DocumentEngine` | Existing | Container for incremental state |
| LSP `apply_document_changes` | Existing | Already computes edit ranges |

**New components needed:**
- `boundary_scan.zig` kernel (compose fence_map + blank line detection)
- `ParserSnapshot` struct (snapshot state at boundaries)
- Chunk tree data structure (sorted Vec<Chunk> by byte_range)
- Edit propagation logic (loop with convergence detection)

---

## Next Steps

1. **Brainstorm session** — Resolve 7 design questions above (consensus on Q1-Q7)
2. **Prototype** — Implement ParserSnapshot serialization + comparison (smallest unit)
3. **Benchmark** — Measure actual reparse time at 1MB, 5MB with full md4c (validate problem)
4. **boundary_scan kernel** — Build SIMD kernel for safe cut points
5. **Chunk tree MVP** — Implement on branch, measure convergence behavior on real documents
6. **Integration** — Wire incremental `updateRange()` into Rust LSP layer

**Effort estimate:** 4-6 weeks for MVP (boundary_scan kernel + chunk tree + convergence detection).

---

## References

- **Comprehensive research:** `/docs/research/incremental-parsing-sota-2026.md`
- **Design brainstorm:** `/docs/research/incremental-md4c.md` (section "Open Design Questions")
- **Memory notes:** `/docs/MEMORY.md` (section "Incremental md4c Block-Level Reparse")
