---
id: marky-0jz
title: 'D: Vendor tree-sitter-md, selective inline skip via changed_ranges()'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-7dq]
parent: marky-77i
---




## Problem
tree-sitter-md 0.5.2 dual grammar: after block parse, iterates ALL ~500 inline/pipe_table_cell nodes. Each gets set_included_ranges() + parser.parse() FFI call (~13us each). Total inline: ~7ms even with old-tree reuse. This is the primary bottleneck — not lack of reuse, but N FFI calls where N = paragraph count.

## Implementation
1. Vendor tree-sitter-md 0.5.2 into workspace (copy bindings/rust/parser.rs + C sources, or [patch] section)
2. After block tree parse (line 306), compute changed_ranges:
   let changed = old_tree.block_tree.changed_ranges(&block_tree).collect::<Vec<_>>();
3. In inline iteration loop (lines 319-366), for each inline/pipe_table_cell node:
   - If old_tree exists AND node byte range doesn't overlap any changed range AND old inline tree exists: carry forward old inline tree directly, skip FFI call
   - Otherwise: parse normally (existing code)
4. Handle edge case: block structure change (paragraph added/removed) — fall back to full inline when inline node count differs

## Key Files
- tree-sitter-md-0.5.2/bindings/rust/parser.rs:319-366 (inner loop to modify)
- markymark-parser/src/lib.rs (Parser wrapper — must use vendored tree-sitter-md)
- markymark-parser/benches/incremental.rs (benchmark harness)

## Testing
- state_tests::benchmark_incremental_prose_edit_vs_full_rebuild should pass at >=2x (currently 1.2x)
- Update criterion benchmarks in markymark-parser/benches/incremental.rs
- Correctness: incremental must produce identical AST to full parse

## Expected Impact
Block stays at 5.7ms. Inline drops from 7.1ms to ~13us (1 parse instead of 500). Total: ~5.7ms vs 15.8ms = ~2.8x per-parse.

## Depends on
F (debounce) should land first — lower risk, higher immediate UX impact.
