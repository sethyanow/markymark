---
id: marky-vma
title: 'Address PR #33 review comments (BRZA epic)'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---

## Context

PR #33 (feature/mark-brza → main) received review comments from greptile, coderabbit, and Copilot.
The branch is in RED ZONE scope-guard (16k lines, 56 commits, 33 new files).
Goal: address all P1/P2 items before merge.

## Branch & PR

- Branch: `feature/mark-brza`
- PR: https://github.com/sethyanow/markymark/pull/33
- Worktree: /Volumes/code/markymark/.worktrees/feature-mark-brza

## P1 — Bugs (fix first)

### 1. Missing score>0 filter on batch path (runtime_engine.rs:367)
File: `markymark-mcp/src/runtime_engine.rs` around line 367
Problem: batch path returns ALL matches including zero/negative scores.
Fallback path filters by `m.score > 0` — inconsistent behavior.
Fix: add `.filter(|m| m.score > 0)` before `.filter_map` in the `Ok(ranked)` arm.

### 2. YAML indent wrong for list items (yaml_scan.zig:148)
File: `zig/src/kernels/formats/yaml_scan.zig` around line 148
Problem: `indent` is computed before stripping the `- ` list prefix.
For `  - host: a`, emits indent=2 (dash column) instead of indent=4 (key column).
Also makes `block_base_indent` too small — block scalars inside lists may bleed into siblings.
Fix: recompute effective indent AFTER consuming `- ` (and following whitespace).
The diff looks like:
```zig
if (buf[p] == '-' and p + 1 < line_end and buf[p + 1] == ' ') {
    const list_prefix_start = p;
    p += 2;
    while (p < line_end and (buf[p] == ' ' or buf[p] == '\t')) : (p += 1) {}
    indent += p - list_prefix_start;  // adjust indent to key column
```

### 3. Empty [] section header ambiguous with global section (ini_scan.zig:137)
File: `zig/src/kernels/formats/ini_scan.zig` around line 137
Problem: `[]` sets section_len=0 but section_offset!=0, ambiguous with (0,0) global-section sentinel.
Fix: after computing slen, add `if (slen == 0) return false;` before the maxInt check.

## P2 — Quality/Perf

### 4. Test comment claims 0x4f9f2cab but never asserts it (runtime_engine.rs:1263)
File: `markymark-mcp/src/runtime_engine.rs` around line 1263
Fix: add `assert_eq!(fnv1a32(b"hello"), 0x4f9f2cab);` or remove the claim from the docstring.

### 5. Eager alloc of all candidate names before scoring (runtime_engine.rs:292)
File: `markymark-mcp/src/runtime_engine.rs` around line 292
Problem: `candidates.push((heading.text.to_string(), ...))` clones every heading name before scoring.
Old code only cloned matched names. Real allocation regression at scale.
Fix: store `candidates` as `Vec<(&str, DocumentUri, Range)>` borrowing from the indexes,
derive `candidate_refs` from that, then only clone strings for the final ranked results.
(Requires lifetime annotations — may need a let binding to extend index lifetime.)

### 6. top_k = candidates.len() defeats heap optimization (runtime_engine.rs:305)
File: `markymark-mcp/src/runtime_engine.rs` around line 305
Problem: Zig min-heap top-k is O(n log k) but with k=n it degrades to O(n log n) — same as sort.
Fix: cap at a constant (e.g., `const TOP_K_LIMIT: usize = 100;`) or make it configurable.
Note: this is a perf-only issue, correctness is fine.

## P3 — Safety/Style (do if time permits)

### 7. ? operator inside unsafe block (scan.rs:511)
File: `markymark-kernels/src/scan.rs` around line 501-511
Fix: hoist `let query_len_u32 = u32::try_from(query.len()).map_err(...)?;` before the unsafe block.

### 8. unsafe impl Sync relies on external RwLock (embed.rs:94)
File: `markymark-kernels/src/embed.rs` around line 94
The Sync safety comment says 'the RwLock in RuntimeEngine ensures...' — external invariant.
Consider: add a struct-level doc warning that Sync is only safe under external read-write locking.

### 9. Benchmark fixture double-allocates (brza_kernels.rs:344)
File: `markymark-kernels/benches/brza_kernels.rs` around line 344
Fix: store `Vec<String>` in fixture, derive `Vec<&str>` inside the bench closure (not in OnceLock).

## Nitpicks (optional, docs only)

- `docs/agent-patterns-raw.md:27,32`: 'FULL text' → 'Full-text', 'markdown' → 'Markdown'
- `docs/research/vscode-extension-design.md:295`: add `text` language to ASCII diagram code fence
- `docs/research/wasm-zig-feasibility.md:60`: 'markdown' → 'Markdown'

## Suggested Attack Order

1. P1 bugs (3 items) — score filter, YAML indent, INI empty section
2. P2 assert-the-constant (1 item, trivial)
3. P2 top_k cap (1 item, trivial)
4. P2 eager alloc (1 item, involves lifetimes, careful)
5. P3 ? in unsafe (trivial style)
6. P3 bench double-alloc
7. Nitpick docs (optional)

## After fixing

Run: cargo nextest (full suite)
Check: cargo clippy --workspace --all-targets
Then: git push, reply to review comments on PR #33
