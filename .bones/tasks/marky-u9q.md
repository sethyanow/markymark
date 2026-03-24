---
id: marky-u9q
title: 'Task 1: Direct arena decode — from_engine_result_direct bypasses EngineExtraction'
status: active
type: task
priority: 2
owner: Seth
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

- [ ] `EngineResult` exposes `text_blob() -> &[u8]` accessor and typed slice accessors (`headings() -> &[CEngineHeading]`, `links() -> &[CEngineLink]`, etc.)
- [ ] `read_blob_str(blob, offset, len) -> Result<&str, KernelError>` is pub in markymark-kernels
- [ ] `DocumentIndex::from_engine_result_direct(&EngineResult, fm, aliases) -> Result<Self, KernelError>` builds index reading text_blob directly into arena
- [ ] `index_from_engine_result` in LSP state uses the direct path instead of `to_extraction()`
- [ ] Parity test: direct path produces identical DocumentIndex content as old path (headings, links, tags, code spans, block_ids, tasks, embeds, callouts, block_refs, query_blocks, link_definitions, properties, xml_tags)
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
  Create a DocumentEngine from a multi-element markdown doc containing ALL element types:
  headings (multiple levels), wiki links (with/without alias, with/without heading anchor),
  markdown links (with/without anchor), tags, code spans, block_ids, tasks (checked + unchecked),
  embeds, callouts (with/without title), block_refs, query_blocks, link_definitions (with/without
  title), properties, xml_tags.
  Get EngineResult, build DocumentIndex via BOTH paths (old `from_engine_result_with_frontmatter`
  and new `from_engine_result_direct`). Compare ALL accessor outputs: headings count + slugs +
  text + levels, wiki link targets + aliases + heading anchors, markdown link urls + anchors +
  text, tag names, code span text, etc. Every element type must be compared.
- **Expected:** compile error — `from_engine_result_direct` doesn't exist yet

### Step 2: GREEN — Add EngineResult accessors
**File:** `markymark-kernels/src/engine_ffi.rs`

**Accessors on `impl EngineResult`:**
- `text_blob(&self) -> &[u8]`: extracts blob slice from `self.raw.text_blob` / `self.raw.text_blob_len`
  using same null/len check as `convert_engine_result` lines 500-512 (null ptr → empty slice,
  otherwise unsafe `from_raw_parts`).
- Typed slice accessors wrapping private `ptr_slice` internally — keeps C struct iteration
  inside markymark-kernels:
  - `headings(&self) -> Result<&[CEngineHeading], KernelError>`
  - `links(&self) -> Result<&[CEngineLink], KernelError>`
  - `code_spans(&self) -> Result<&[CEngineCodeSpan], KernelError>`
  - `tags(&self) -> Result<&[CEngineTag], KernelError>`
  - `block_ids(&self) -> Result<&[CEngineBlockId], KernelError>`
  - `tasks(&self) -> Result<&[CEngineTask], KernelError>`
  - `embeds(&self) -> Result<&[CEngineEmbed], KernelError>`
  - `callouts(&self) -> Result<&[CEngineCallout], KernelError>`
  - `block_refs(&self) -> Result<&[CEngineBlockRef], KernelError>`
  - `query_blocks(&self) -> Result<&[CEngineQueryBlock], KernelError>`
  - `link_definitions(&self) -> Result<&[CEngineLinkDefinition], KernelError>`
  - `properties(&self) -> Result<&[CEngineProperty], KernelError>`
  - `xml_tags(&self) -> Result<&[CEngineXmlTag], KernelError>`
  - `line_starts(&self) -> Result<&[u32], KernelError>`
  - `token_estimate(&self) -> u32`
  - `content_hash(&self) -> u64`
  - `generation(&self) -> u64`

**Public helper:**
- `pub fn read_blob_str(blob: &[u8], offset: u32, length: u32) -> Result<&str, KernelError>`
  — wraps `safe_text_blob_slice` + `from_utf8`, returns `&str` (borrowed, not owned).

**NOTE:** `as_raw() -> &CEngineResult` already exists on EngineResult (line 440). Do NOT add
a duplicate `c_result()`.

### Step 3: GREEN — Implement from_engine_result_direct
**File:** `markymark-index/src/document/from_engine.rs`
- New constructor: `pub fn from_engine_result_direct(result: &EngineResult, fm: Vec<FrontmatterOwnedEntry>, aliases: Vec<String>) -> Result<Self, KernelError>`
- Returns `Result` (blob reads can fail on invalid UTF-8 / out-of-bounds — unlike `from_engine_result_inner` which takes pre-validated data)
- Inside: `let blob = result.text_blob();`
- Use typed accessor methods (`result.headings()?`, `result.links()?`, etc.) to iterate C struct
  slices — NOT `ptr_slice` directly (it's private to engine_ffi.rs)
- For each text field: `read_blob_str(blob, h.text_offset, h.text_length)?` → `arena_alloc_str(arena, s)`
- **Link parsing (critical — replicate `convert_engine_result` logic exactly):**
  - Check `l.is_wiki != 0` to split wiki vs markdown links
  - Wiki links: read target from blob, split on `#` for `(page, heading)`, compare text vs target
    for alias detection (`if text != target { Some(text) } else { None }`)
  - Markdown links: read target from blob, split on `#` for `(url, anchor)`
  - **These splits operate on `&str` borrows, but the arena needs owned copies.** Split first,
    then `arena_alloc_str` each part.
- **Task state encoding:** convert `t.state` byte to string: `b'x' | b'X' → "checked"`, else `"unchecked"`
- **Optional fields:** callout title (`title_length == 0 → None`), link definition title (same),
  xml tag flags (bool conversion from u8)
- **Content blocks: NOT extracted.** Current hot path (`index_from_engine_result` → `from_engine_result_with_frontmatter`) passes empty source and empty blocks. Match this behavior — no `extract_content_blocks` call. Source text threading is a separate enhancement.

### Step 4: GREEN — Wire LSP state to use direct path
**File:** `markymark-lsp/src/state/mod.rs`
- In `index_from_engine_result` (line 137): replace:
  ```rust
  let extraction = result.to_extraction().map_err(…)?;
  Ok(DocumentIndex::from_engine_result_with_frontmatter(&extraction, frontmatter, aliases))
  ```
  with:
  ```rust
  DocumentIndex::from_engine_result_direct(&result, frontmatter, aliases)
      .map_err(|e| format!("from_engine_result_direct failed: {e:?}"))
  ```
- No signature change to `index_from_engine_result` — it already returns `Result<DocumentIndex, String>`

### Step 5: Verify — Run all tests
- `cargo nextest -p markymark-index` — index tests pass including parity test
- `cargo nextest` — full workspace passes (1289+)

### Step 6: Final verification and commit

## Key Considerations

- **`ptr_slice` is PRIVATE in engine_ffi.rs** (line 485: `fn ptr_slice`, not `pub fn`). The
  skeleton originally claimed it was pub — WRONG. markymark-index cannot call it. Solution:
  typed accessor methods on EngineResult that wrap `ptr_slice` internally, keeping C struct
  iteration encapsulated in markymark-kernels.
- **`as_raw() -> &CEngineResult` already exists** on EngineResult (line 440). Do not add a
  duplicate `c_result()` accessor.
- **Link parsing is the highest-risk section.** `convert_engine_result` (lines 528-576) has
  non-trivial splitting: wiki links split target on `#` for page/heading, compare text vs
  target for alias detection; markdown links split on `#` for url/anchor. The direct path must
  replicate this exactly. The `&str` borrows from `read_blob_str` need to be split BEFORE
  arena allocation (can't arena_alloc_str the full target then split the arena copy — that
  would borrow from the arena, which is being mutated during construction).
- **Content blocks: NOT extracted in current hot path.** `index_from_engine_result` calls
  `from_engine_result_with_frontmatter` which passes `String::new()` as source and `Vec::new()`
  as raw_blocks. The direct path must match this — no content block extraction. The skeleton
  originally said "same as current path" for content blocks, but the current path doesn't
  extract them. Content block threading would be a behavior change (separate task).
- **Error model differs from `from_engine_result_inner`.** The inner function takes pre-validated
  `&EngineExtraction` (all strings already converted). The direct path reads raw blob data and
  can fail on invalid UTF-8 or out-of-bounds offsets. Must return `Result<Self, KernelError>`.
- **13 element types to handle.** CEngineResult has: headings, links, code_spans, tags, block_ids,
  tasks, embeds, callouts, block_refs, query_blocks, link_definitions, properties, xml_tags. Each
  needs its own iteration + blob decode + arena allocation block. Plus line_starts and token_estimate.
- **Wiki link end_byte calculation:** `from_engine_result_inner` (line 227-231) computes wiki link
  end_byte as `source_offset + target_len + text_len + 5` (with alias) or `+ 4` (without). This
  uses EngineWikiLink fields that were parsed from the raw link. The direct path computes from
  CEngineLink fields: `source_offset + target_length + text_length + overhead`. The overhead
  depends on whether it's an alias link (`[[target|alias]]` = 5 chars) or plain (`[[target]]` = 4).
- **Markdown link end_byte calculation:** `source_offset + text_len + target_len + 4` for `[text](url)`.

## Edge Cases & Failure Modes

- **Empty document (0 elements):** All typed accessors return empty slices. The direct path
  should produce an empty DocumentIndex — same as the old path. Parity test should include this.
- **Empty text_blob (text_blob_len == 0):** `text_blob()` returns `&[]`. Any `read_blob_str` call
  with non-zero offset would fail. This is correct — 0 elements means 0 blob reads.
- **Wiki link with empty alias:** `text == target` → alias is None. The direct path must use
  the same comparison (on blob-read `&str` values, not arena copies).
- **Callout/link_def with title_length == 0:** Maps to `None`. Must check BEFORE attempting
  blob read (0-length read at some offset could succeed but produces empty string — wrong semantic).
- **Task state encoding edge case:** What if `state` is neither `b'x'`, `b'X'`, nor `b' '`? Current
  code treats anything non-x/X as "unchecked". Match this behavior.
- **Large documents:** Many elements means many accessor calls and blob reads. Should not be a
  problem (all O(n) in element count) but parity test should include a doc with 50+ headings.

## Adversarial Failure Catalog

**Encoding Boundaries: read_blob_str**
- Assumption: Blob content at `[offset..offset+length]` is valid UTF-8
- Betrayal: Zig writes non-UTF-8 bytes (corrupted markdown, binary content)
- Consequence: `from_utf8` returns `Err` → `KernelError::InternalError(-100)` propagated
- Mitigation: Structural — `read_blob_str` returns `Result`, caller propagates. Matches existing
  `read_str` behavior. Both old and new paths fail identically on invalid UTF-8.

**Input Hostility: from_engine_result_direct — wiki link alias comparison ordering**
- Assumption: `text != target` compares the correct pair for alias detection
- Betrayal: Agent accidentally compares the split `page` (after `#` removal) against `text`
  instead of the full original `target` against `text`
- Consequence: Links with `target#heading` format would incorrectly detect alias (page != text)
  when text == target. Parity test catches this.
- Mitigation: Parity test with wiki link containing `#heading` is mandatory. Implementation note:
  compare text vs FULL target, THEN split target for page/heading.

**Temporal Betrayal: LSP test hook becomes dead code**
- Assumption: `should_force_engine_result_conversion_fail_for_tests` (state/mod.rs:146) guards
  against `to_extraction()` failure
- Betrayal: After wiring to direct path, `to_extraction()` is no longer called in the hot path.
  The hook no longer exercises a reachable failure mode.
- Consequence: Tests using `__force_conversion_fail` URI pattern silently stop testing the failure
  path. The direct path has its own error surface (blob read failures) that isn't covered by
  any test hook.
- Mitigation: In Step 4, repurpose or replace the hook to inject failure into
  `from_engine_result_direct`. Or add a separate test that exercises `from_engine_result_direct`
  error propagation with a corrupted blob. **Do not silently leave the dead hook.**

**Input Hostility: from_engine_result_direct — link end_byte arithmetic**
- Assumption: Wiki link overhead is 4 chars (`[[` + `]]`) without alias, 5 chars (`[[` + `|` + `]]`) with alias
- Betrayal: Zig engine produces a link where source_offset doesn't align with `[[` position
  (e.g., due to frontmatter masking offset). End_byte calculation is wrong.
- Consequence: WikiLinkEntry.end_byte is incorrect — downstream range-based operations (e.g.,
  edits, diagnostics) reference wrong byte positions. Parity test catches this if it compares
  start_byte and end_byte fields.
- Mitigation: Parity test must compare start_byte AND end_byte for every link entry. The
  calculation matches `from_engine_result_inner` lines 227-231 — same arithmetic, same risk.

## Log

- [2026-03-24T16:27:14Z] [Seth] SRE review (fresh session). Findings: (1) ptr_slice is private, not pub — skeleton claimed pub, fixed to typed accessor approach on EngineResult. (2) as_raw() already exists, removed duplicate c_result() proposal. (3) Content blocks NOT extracted in current hot path — skeleton wrongly said 'same as current path' for extract_content_blocks. (4) LSP test hook should_force_engine_result_conversion_fail_for_tests becomes dead code after wiring — must repurpose or replace. (5) Link parsing is highest-risk section — alias comparison ordering matters. Updated implementation steps, key considerations, edge cases, and adversarial failure catalog.
