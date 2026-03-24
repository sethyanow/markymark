---
id: marky-u9q
title: 'Task 1: Direct arena decode — from_engine_result_direct bypasses EngineExtraction'
status: open
type: task
priority: 2
parent: marky-8d8
---

## Context

Phase 3 (marky-8d8) of Engine Pipeline v2 (marky-zsys). First seam task — internal optimization.

Currently `index_from_engine_result` in the LSP state module does two copies:
1. `result.to_extraction()` → reads CEngineResult.text_blob into owned Strings (EngineExtraction)
2. `from_engine_result_inner(&extraction, ...)` → copies those Strings into bumpalo arena

This task eliminates copy 1 by reading text_blob directly into the arena. The EngineResult
owns the CEngineResult (Zig-allocated), which is valid until EngineResult is dropped. During
the build, we borrow text_blob, decode via `safe_text_blob_slice` + `str::from_utf8`, and
`arena_alloc_str` the result — one copy total.

**Blocked by:** marky-686 (Phase 2, closed)
**Unlocks:** Measurable speedup on the hot path. Foundation for Phase 3b (lifetime parameterization).

## Requirements

From parent sub-epic marky-8d8:
- R5: Direct arena decode — bypass EngineExtraction owned Strings, read text_blob into arena

## Success Criteria

- [ ] `EngineResult` exposes `text_blob() -> &[u8]` and `c_result() -> &CEngineResult` accessors
- [ ] `DocumentIndex::from_engine_result_direct(&EngineResult, fm, aliases, source)` builds index reading text_blob directly into arena
- [ ] `index_from_engine_result` in LSP state uses the direct path instead of `to_extraction()`
- [ ] Parity test: direct path produces identical DocumentIndex content as old path (headings, links, tags, etc.)
- [ ] Old `from_engine_result_with_frontmatter` path remains available (not deleted — other consumers may use it)
- [ ] All existing tests pass (1289+ workspace)

## Anti-Patterns

- NO unsafe lifetime extensions — all borrows of text_blob are scoped to the build function
- NO removing EngineExtraction or convert_engine_result (other consumers may use them; cleanup is a separate task)
- NO changing DocumentIndex public API (self_cell, accessors, entry types all unchanged)
- NO duplicating the link-parsing logic from from_engine_result_inner (extract shared helpers if needed)

## Implementation

### Step 1: RED — Write parity test
**File:** `markymark-index/src/document/from_engine.rs` (tests module)
- `test_from_engine_result_direct_parity`:
  Create a DocumentEngine from a multi-element markdown doc (headings, links, tags, code spans),
  get EngineResult, build DocumentIndex via BOTH paths (old `from_engine_result_with_frontmatter`
  and new `from_engine_result_direct`), compare all accessor outputs (headings count, slug values,
  link targets, tag names, etc.)
- **Expected:** compile error — `from_engine_result_direct` doesn't exist yet

### Step 2: GREEN — Add EngineResult accessors
**File:** `markymark-kernels/src/engine_ffi.rs`
- Add `text_blob(&self) -> &[u8]` to `impl EngineResult`: extracts blob slice from `self.raw`
  using same null/len check as `convert_engine_result` lines 500-512
- Add `c_result(&self) -> &CEngineResult` to `impl EngineResult`: returns `&self.raw`
- Also add a public `read_blob_str` function: `(blob: &[u8], offset: u32, length: u32) -> Result<&str, KernelError>`
  — wraps `safe_text_blob_slice` + `from_utf8`, returns `&str` instead of owned `String`

### Step 3: GREEN — Implement from_engine_result_direct
**File:** `markymark-index/src/document/from_engine.rs`
- New constructor on `impl DocumentIndex`:
  `fn from_engine_result_direct(result: &EngineResult, fm: Vec<FrontmatterOwnedEntry>, aliases: Vec<String>, source: String) -> Self`
- Inside: get `blob = result.text_blob()`, `raw = result.c_result()`
- Use `ptr_slice` to iterate C struct arrays (headings, links, etc.)
- For each text field: `read_blob_str(blob, offset, length)?` → `arena_alloc_str(arena, s)`
- Mirror the structure of `from_engine_result_inner` but read from C structs + blob directly
  instead of from EngineExtraction owned Strings
- Handle links: parse wiki vs markdown the same way convert_engine_result does (check `is_wiki` flag)
- Content blocks: still use tree-sitter via `extract_content_blocks(&source)` — same as current path

### Step 4: GREEN — Wire LSP state to use direct path
**File:** `markymark-lsp/src/state/mod.rs`
- In `index_from_engine_result`: replace `result.to_extraction()` + `from_engine_result_with_frontmatter`
  with `DocumentIndex::from_engine_result_direct(&result, frontmatter, aliases)`
- The source text for content blocks: pass from `build_markdown_index_via_engine`'s `text` parameter

### Step 5: Verify — Run all tests
- `cargo nextest -p markymark-index` — index tests pass including parity test
- `cargo nextest` — full workspace passes (1289+)

### Step 6: Final verification and commit

## Key Considerations

- **ptr_slice is pub in engine_ffi.rs** — markymark-index can use it to iterate C struct arrays
  directly. Alternatively, EngineResult can expose typed accessors (headings(), links(), etc.).
  Prefer the accessor approach to keep C struct details inside markymark-kernels.
- **Link parsing is split:** `convert_engine_result` splits links into wiki_links and
  markdown_links based on `is_wiki` flag, and wiki links get further parsed (target/alias/heading
  from the combined text). This logic must be replicated or extracted into a shared helper.
- **Content blocks still use tree-sitter:** `from_engine_result_inner` calls
  `extract_content_blocks(&source)` which is a tree-sitter parse. This doesn't change — tree-sitter
  blocks aren't extracted from the Zig engine. The source text must be passed through.
- **Error handling:** `read_blob_str` can fail on invalid UTF-8 or out-of-bounds offsets.
  `from_engine_result_direct` should propagate these as Result, not panic.
  The caller (`index_from_engine_result`) already handles errors.
