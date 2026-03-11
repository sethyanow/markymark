---
id: marky-gb1
title: 'Task 4: Implement selective merge for tags, markdown_links, and xml_tags extractors'
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-rmx]
parent: marky-77i
---




## Design

## Goal
Implement incremental selective merge for the 3 remaining independent extractors (tags, markdown_links, xml_tags), extract incremental logic to incremental.rs, and extract diagnostics logic to diagnostics.rs — all in one task to bring state.rs from 1702 lines to ~975 lines.

## Decision: Option B (extract both incremental.rs + diagnostics.rs)
SRE refinement determined that extracting only incremental.rs leaves ~1114 lines. Extracting diagnostics.rs additionally (~1h extra) meets the under-1000 criterion at ~975 lines.

## Critical Findings from SRE Analysis
- Tags have NO range info from extract_tags() — cannot incrementally merge, always full rebuild (pass None as override)
- MarkdownLink/XmlTag have NO byte offsets — neighbor_window not available, use range-only checks
- XmlTagOwned Vec<(String, String)> vs XmlTagEntry HashMap — duplicate keys: last-write-wins (matches HTML semantics, acceptable)
- from_ast_with_overrides_opt growing to 5 Option params — use IncrementalOverrides struct to prevent wrong-position None bugs
- apply_document_changes: consolidate 4 separate get_document() calls into 1 snapshot

## Architecture (6 ordered steps)

### Step 1: Promote owned types to types.rs
File: markymark-index/src/document/types.rs
Add as pub structs (currently local inside from_ast_with_overrides_opt):
- pub struct TagOwned { pub name: String }
- pub struct MarkdownLinkOwned { pub text: String, url: String, anchor: Option<String>, range: Range }
- pub struct XmlTagOwned { pub tag_name: String, attributes: Vec<(String, String)>, is_self_closing: bool, is_unclosed: bool, range: Range }
Sort XmlTagOwned attributes by key in extract_xml_tags_owned for deterministic ordering.

### Step 2: Add IncrementalOverrides struct + extend from_ast_with_overrides_opt
File: markymark-index/src/document/mod.rs
Replace the 2-param (wiki_links, blocks) signature with IncrementalOverrides struct:
pub struct IncrementalOverrides {
    pub wiki_links: Option<Vec<WikiLinkOwned>>,
    pub blocks: Option<Vec<BlockOwned>>,
    pub tags: Option<Vec<TagOwned>>,          // always None (tags have no range, skip optimization)
    pub markdown_links: Option<Vec<MarkdownLinkOwned>>,
    pub xml_tags: Option<Vec<XmlTagOwned>>,
}
fn from_ast_with_overrides_opt(ast: Ast, overrides: IncrementalOverrides) -> Self
Remove local struct definitions (now in types.rs).
Update all callers (from_ast_with_wiki_links, from_ast_with_blocks, from_ast_with_wiki_links_and_blocks).
Also export IncrementalOverrides from the index crate (pub use in lib.rs or mod.rs).

### Step 3: Create markymark-lsp/src/incremental.rs
Move from state.rs to incremental.rs (module-level pub fns, NOT methods):
- range_intersects_edit, range_is_after_edit_start, range_within_neighbor_window
- IncrementalByteBounds, incremental_byte_bounds, position_was_clamped
- WikiLinkOwned helpers: wiki_link_affected_by_edits, wiki_links_need_update, any_edit_starts_at_or_after_last_wiki_link, extract_wiki_links_owned, merge_incremental_wiki_links
- BlockOwned helpers: block_affected_by_edits, blocks_need_update, any_edit_starts_at_or_after_last_block, extract_blocks_owned, merge_incremental_blocks
- build_markdown_index_incremental (updated to handle all 5 extractors)

Add new helpers for MarkdownLink and XmlTag (tags always None — no helper needed):

MarkdownLink (range-only, no byte offsets — skip neighbor_window):
fn markdown_link_affected_by_edits(ml: &MarkdownLinkOwned, edits: &[InputEdit]) -> bool {
    edits.iter().any(|e| range_intersects_edit(ml.range, e) || range_is_after_edit_start(ml.range, e))
}
fn markdown_links_need_update(old: &[MarkdownLinkOwned], edits: &[InputEdit]) -> bool {
    if edits.is_empty() { return false; }
    old.iter().any(|ml| markdown_link_affected_by_edits(ml, edits))
        || any_edit_starts_at_or_after_last_markdown_link(old, edits)
}
fn any_edit_starts_at_or_after_last_markdown_link(old: &[MarkdownLinkOwned], edits: &[InputEdit]) -> bool
fn extract_markdown_links_owned(ast: &Ast) -> Vec<MarkdownLinkOwned>
fn merge_incremental_markdown_links(old: &[MarkdownLinkOwned], new: &[MarkdownLinkOwned], edits: &[InputEdit]) -> Vec<MarkdownLinkOwned>
  Logic: same as wiki_links merge — keep old entries not affected by edits, take new entries from affected regions, dedup by range.start.

XmlTag (range-only, no byte offsets — same pattern as MarkdownLink):
fn xml_tag_affected_by_edits, xml_tags_need_update, any_edit_starts_at_or_after_last_xml_tag
fn extract_xml_tags_owned(ast: &Ast) -> Vec<XmlTagOwned>  (sort attributes by key for determinism)
fn merge_incremental_xml_tags(old, new, edits) -> Vec<XmlTagOwned>

Updated build_markdown_index_incremental:
pub fn build_markdown_index_incremental(
    old_wiki_links: Option<&[WikiLinkOwned]>,
    old_blocks: Option<&[BlockOwned]>,
    old_markdown_links: Option<&[MarkdownLinkOwned]>,
    old_xml_tags: Option<&[XmlTagOwned]>,
    ast: Ast,
    pending_edits: &[InputEdit],
) -> DocumentIndex {
    if pending_edits.is_empty() { return DocumentIndex::from_ast(ast); }
    // compute merged_wiki_links, merged_blocks (existing logic)
    // compute merged_markdown_links (new, same pattern as wiki_links)
    // compute merged_xml_tags (new, same pattern as wiki_links)
    // tags: always None (no range info, skip)
    let overrides = IncrementalOverrides {
        wiki_links: merged_wiki_links,
        blocks: merged_blocks,
        tags: None,
        markdown_links: merged_markdown_links,
        xml_tags: merged_xml_tags,
    };
    DocumentIndex::from_ast_with_overrides_opt(ast, overrides)
}

### Step 4: Create markymark-lsp/src/diagnostics.rs
Move from state.rs:
- compute_diagnostics method body (and its helpers)
Shrinks state.rs by ~150 lines.

### Step 5: Update state.rs
- Replace all Self::wiki_link_*/block_* with incremental::* calls
- Update build_markdown_index_incremental call to pass old_markdown_links, old_xml_tags
- Consolidate the 4 get_document(uri) calls in apply_document_changes into 1 snapshot:
  let old_data = self.realm.get_document_index(uri).map(|idx| OldIndexData {
      wiki_links: idx.wiki_links().iter().map(WikiLinkOwned::from).collect(),
      blocks: idx.blocks_owned(),
      markdown_links: idx.markdown_links().iter().map(MarkdownLinkOwned::from).collect(),
      xml_tags: idx.xml_tags().iter().map(XmlTagOwned::from).collect(),
  });
- Add mod incremental; mod diagnostics; in lib.rs or state.rs header

### Step 6: Tests (TDD — RED first, then GREEN)

**markymark-index/src/document/tests.rs:**

test_tag_no_incremental_optimization_needed: Tags always full rebuild, verify tags() works correctly after from_ast_with_overrides_opt with tags: None
  Bug catches: None field incorrectly overrides tags with empty vec

test_markdown_link_override_reuses_when_provided: Pass markdown_links override with 2 links, verify index.markdown_links() returns those 2 without re-extracting
  Bug catches: override ignored, extraction runs anyway

test_xml_tag_override_reuses_when_provided: Same as above for xml_tags
  Bug catches: override ignored

test_incremental_overrides_all_five: Build with all 5 overrides set, verify all returned correctly
  Bug catches: IncrementalOverrides struct field ordering bug, wrong field used

**markymark-lsp/src/incremental.rs (tests module):**

test_markdown_links_need_update_false_when_no_edits: No pending edits -> false
  Bug catches: always returning true

test_markdown_links_need_update_true_when_link_intersects_edit: Edit overlapping link range -> true
  Bug catches: range_intersects_edit not called for markdown links

test_markdown_link_after_edit_start_triggers_update: Link starts after edit.start_byte -> true
  Bug catches: any_edit_starts_at_or_after_last_markdown_link not implemented

test_merge_incremental_markdown_links_keeps_unaffected: Two links, edit near link 1 only, merged result preserves link 2 from old
  Bug catches: purge logic too aggressive, drops unaffected entries

test_xml_tags_need_update_false_when_no_edits
test_xml_tag_affected_by_edits_detects_overlap
test_merge_incremental_xml_tags_preserves_attributes: Old xml tag has attributes, survives merge, attributes intact in correct order
  Bug catches: attribute Vec-to-HashMap conversion drops attributes

**Integration parity test (markymark-lsp/src/state.rs or tests module):**
test_incremental_matches_full_rebuild_for_all_extractors:
  1. Build full DocumentIndex for doc with: [[wiki_link]], ^block-id, #tag, [link](url), <div class='x'>
  2. Build incremental with same AST + zero pending edits (empty IncrementalOverrides)
  3. Assert wiki_links identical, blocks identical, tags identical, markdown_links identical, xml_tags identical
  Bug catches: any extractor silently dropped in the overrides path

## Success Criteria
- [ ] TagOwned, MarkdownLinkOwned, XmlTagOwned promoted to pub in types.rs
- [ ] IncrementalOverrides struct in markymark-index, used by from_ast_with_overrides_opt
- [ ] Incremental helpers for markdown_links and xml_tags in incremental.rs
- [ ] Tags correctly always full-rebuild (None override, no incremental logic)
- [ ] build_markdown_index_incremental handles all 5 extractors via IncrementalOverrides
- [ ] diagnostics.rs extracted from state.rs
- [ ] state.rs under 1000 lines after both extractions
- [ ] incremental.rs under 500 lines
- [ ] All 11+ new tests written FIRST (RED), then GREEN
- [ ] All existing workspace tests pass (no regressions)
- [ ] cargo fmt --check clean
- [ ] cargo clippy --workspace --all-targets clean
- [ ] No unwrap/expect in new code

## Anti-Patterns
- NO incremental optimization for tags (no range info available)
- NO neighbor_window for MarkdownLink/XmlTag (no byte offsets)
- NO 5 positional Option params — use IncrementalOverrides struct
- NO capturing old data after realm.remove_document
- NO creating incremental.rs over 500 lines
- NO unwrap/expect in new code
