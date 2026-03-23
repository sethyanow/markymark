---
id: marky-waw
title: 'Task 2: Wire block-refs and journal detection into DocumentIndex'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-khy]
parent: marky-9mo
---





## Design

## Goal
Add Logseq block-reference wiring and journal page detection to the DocumentIndex layer. This completes the parser/index layer before Task 3 (new MCP tools) can build on it.

## Effort Estimate
6-8 hours total (4 sections, each 1-2 hours)

## Context
- Task 1 (marky-khy) completed: frontmatter, properties, aliases wired into DocumentIndex.
- Parser already has \`extract_block_refs()\` at extract.rs:261 — returns Vec<BlockRef<'a>> with uuid field
- RealmIndex already has \`block_to_location\` HashMap for BlockId (^block-id), but NOT for BlockRef ((uuid))
- ConnectionGraph has a BlockRef RefKind defined but is not used anywhere yet
- No journal page detection exists anywhere in the codebase

## What BlockRef vs BlockId means
- BlockId (\`^id\`) = definition site — where a block is labeled (Obsidian/Logseq)
- BlockRef (\`((uuid))\`) = outgoing reference — one block referencing another by UUID (Logseq)
- RealmIndex.block_to_location already indexes BlockIds (definition sites)
- BlockRefs are the OUTGOING links from a doc to another doc's block UUID
- Resolution: BlockRef uuid -> lookup in block_to_location -> returns (file, range)

## Requirements from Epic
- Logseq journal pages detected by path pattern, queryable by date (Requirement 4)
- ((block-uuid)) references resolved to target blocks across graph (Requirement 5)

## Implementation

### 1. Wire BlockRef extraction into DocumentIndex (follows Task 1 pattern)

**Study first:** Read markymark-index/src/document/mod.rs:97-311 (from_ast method).
Follow the same owned→arena-alloc pattern as wiki_links_owned (lines 159-174 + 249-258).

**Parser already has:**
\`\`\`rust
// markymark-parser/src/extract.rs:261
pub fn extract_block_refs<'a>(arena: &'a Bump, content: &str) -> Vec<BlockRef<'a>>
// BlockRef has: uuid: &'arena str, range: Range

// markymark-parser/src/ast.rs:163
pub fn extract_block_refs<'a>(&'a self) -> Vec<BlockRef<'a>>
\`\`\`

**Step 1: Add to document/types.rs (after existing types, before XmlTagEntry):**
\`\`\`rust
pub struct BlockRefEntry<'arena> {
    pub uuid: &'arena str,
    pub range: Range,
}
\`\`\`

**Step 2: Add BlockRefOwned to from_ast() owned struct list (after XmlTagOwned, line ~140):**
\`\`\`rust
struct BlockRefOwned { uuid: String, range: Range }
\`\`\`

**Step 3: Extract owned data before arena move (after xml_tags_owned extraction):**
\`\`\`rust
let block_refs_owned: Vec<BlockRefOwned> = ast.extract_block_refs()
    .into_iter()
    .map(|r| BlockRefOwned { uuid: r.uuid().to_string(), range: r.range() })
    .collect();
\`\`\`

**Step 4: Add field to DocumentDependent (document/mod.rs:36-49, after properties field):**
\`\`\`rust
block_refs: &'a [BlockRefEntry<'a>],
\`\`\`

**Step 5: Arena-allocate in dependent closure (after properties allocation):**
\`\`\`rust
let block_refs = {
    let mut v = BumpVec::new_in(arena);
    for r in &block_refs_owned {
        v.push(BlockRefEntry {
            uuid: arena_alloc_str(arena, &r.uuid),
            range: r.range,
        });
    }
    v.into_bump_slice()
};
\`\`\`

**Step 6: Include in DocumentDependent construction (in the ..DocumentDependent{ } initializer):**
\`\`\`rust
block_refs,
\`\`\`

**Step 7: Add public accessor after properties() method:**
\`\`\`rust
pub fn block_refs<'a>(&'a self) -> &'a [BlockRefEntry<'a>] {
    self.borrow_dependent().block_refs
}
\`\`\`

**Step 8: Update Debug impl** to include block_refs count (match the existing pattern for frontmatter/properties).

### 2. Add journal page detection to RealmIndex

Journal detection is path-based, not content-based. Detect in realm layer, keep DocumentIndex path-agnostic.

**File: markymark-index/src/realm.rs**

**Step 1: Add fields to RealmIndex struct (after existing fields):**
\`\`\`rust
// Journal page index: (year, month, day) -> [DocumentUri]
// Key is tuple not string to enable range queries by year/month
date_to_docs: BTreeMap<(u16, u8, u8), Vec<DocumentUri>>,
// Reverse: uri -> detected date for remove_document cleanup
uri_to_date: HashMap<String, (u16, u8, u8)>,
\`\`\`

**Step 2: Initialize in RealmIndex::new():**
\`\`\`rust
date_to_docs: BTreeMap::new(),
uri_to_date: HashMap::new(),
\`\`\`

**Step 3: Add private helper function (above impl block):**
\`\`\`rust
/// Detect Logseq journal date from URI filename stem.
/// Matches YYYY_MM_DD.md and YYYY-MM-DD.md patterns.
/// Returns None for any filename that doesn't exactly match a valid date.
/// Configurable separator: '_' (Logseq default) or '-' (ISO 8601).
fn detect_journal_date(uri: &str) -> Option<(u16, u8, u8)> {
    // Extract filename stem (strip path and .md extension)
    let filename = uri.rsplit('/').next()?;
    let stem = filename.strip_suffix(".md").or_else(|| filename.strip_suffix(".markdown"))?;
    // Stem must be exactly 10 chars: YYYY_MM_DD or YYYY-MM-DD
    if stem.len() != 10 {
        return None;
    }
    // Split on single separator char at positions 4 and 7
    // Positions: 0123-45-67  where - can be _ or -
    let sep = stem.chars().nth(4)?;
    if sep != '_' && sep != '-' {
        return None;
    }
    if stem.chars().nth(7)? != sep {
        return None; // mixed separators not allowed
    }
    let y: u16 = stem[0..4].parse().ok()?;
    let m: u8 = stem[5..7].parse().ok()?;
    let d: u8 = stem[8..10].parse().ok()?;
    // Validate ranges
    if !(1900..=2100).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}
\`\`\`

**Why this approach over splitn:** splitn(3, separator) can't enforce that ONLY the separator chars exist at positions 4 and 7. Direct byte-position checking on the stem is O(1) and unambiguous.

**Step 4: In add_document(), after existing cross-doc index population:**
\`\`\`rust
// Detect and index journal pages
if let Some(date) = detect_journal_date(uri.as_str()) {
    self.date_to_docs.entry(date).or_default().push(uri.clone());
    self.uri_to_date.insert(uri.as_str().to_string(), date);
}
\`\`\`

**Step 5: In remove_document() / remove_from_cross_doc_indexes(), add cleanup:**
\`\`\`rust
if let Some(date) = self.uri_to_date.remove(key) {
    if let Some(docs) = self.date_to_docs.get_mut(&date) {
        docs.retain(|u| u.as_str() != key);
        if docs.is_empty() {
            self.date_to_docs.remove(&date);
        }
    }
}
\`\`\`

**Step 6: Add public accessor:**
\`\`\`rust
/// Returns all journal documents for a given year and month, sorted by day.
pub fn lookup_journal_by_month(&self, year: u16, month: u8) -> Vec<(DocumentUri, u8)> {
    let start = (year, month, 1u8);
    let end = (year, month, 31u8);
    self.date_to_docs
        .range(start..=end)
        .flat_map(|((_, _, d), uris)| uris.iter().map(move |u| (u.clone(), *d)))
        .collect()
}

/// Returns the detected journal date for a URI, or None if not a journal page.
pub fn journal_date(&self, uri: &DocumentUri) -> Option<(u16, u8, u8)> {
    self.uri_to_date.get(uri.as_str()).copied()
}
\`\`\`

### 3. Wire block-refs into find-references MCP tool

**File: markymark-mcp/src/lib.rs** (find_references_tool, currently at line ~362)
**File: markymark-mcp/src/dto.rs** (response types)

The find-references tool currently resolves wiki links and headings. Add block-ref resolution:

**Step 1: Check existing FindReferencesResponse DTO in dto.rs. Add new variant or field:**
\`\`\`rust
// In FindReferencesResponse or a new BlockRefLocation struct:
#[derive(Serialize, Deserialize)]
pub struct BlockRefLocation {
    pub uri: String,
    pub uuid: String,
    pub range: Range,  // position of the ((uuid)) in the referring doc
}
\`\`\`

**Step 2: Extend find-references response to include block_ref_backrefs:**
\`\`\`rust
// In the FindReferencesResponse struct, add field:
pub block_ref_backrefs: Vec<BlockRefLocation>,
\`\`\`

**Step 3: In find_references_tool(), after existing resolution logic:**
\`\`\`rust
// If the target looks like a block UUID (non-empty, alphanumeric+dashes),
// search all docs in the realm for block_refs() that contain this UUID.
let block_ref_backrefs: Vec<BlockRefLocation> = if is_valid_block_uuid(&target) {
    realm.iter_documents()
        .flat_map(|(uri, idx)| {
            idx.block_refs()
                .iter()
                .filter(|r| r.uuid == target)
                .map(|r| BlockRefLocation {
                    uri: uri.as_str().to_string(),
                    uuid: r.uuid.to_string(),
                    range: r.range,
                })
                .collect::<Vec<_>>()
        })
        .collect()
} else {
    vec![]
};
\`\`\`

**Step 4: Add helper:**
\`\`\`rust
fn is_valid_block_uuid(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && s.chars().all(|c| c.is_alphanumeric() || c == '-')
}
\`\`\`

### 4. Tests (TDD — write tests first, run RED, implement, run GREEN)

**File: markymark-index/src/document/tests.rs**

Write these tests BEFORE implementation. Run \`cargo nextest -p markymark-index\` to verify RED.

- \`test_block_refs_stored_in_document_index\`: Parse \`"Some text ((abc-123-def)) more text"\`, verify block_refs() returns 1 entry with uuid="abc-123-def"
  - Bug this catches: extract_block_refs called but result dropped (same bug pattern as Task 1 frontmatter gap)

- \`test_multiple_block_refs_all_returned\`: Parse content with \`((uuid-1)) text ((uuid-2))\`, verify block_refs() returns 2 entries with both UUIDs
  - Bug this catches: only first block ref extracted, Vec truncated early

- \`test_no_block_refs_returns_empty_slice\`: Parse \`"# Heading\nno block refs here"\`, verify block_refs() returns &[]
  - Bug this catches: uninitialized field or wrong default, panics instead of empty

- \`test_block_ref_range_matches_position\`: Parse \`"((abc-123))"\`, verify range.start == 0 and range.end matches byte position of closing ))
  - Bug this catches: range computation off by one or using character count instead of byte offset

- \`test_malformed_block_ref_not_extracted\`: Parse \`"((missing-close"\` and \`"unclosed (( bracket"\`, verify block_refs() returns empty
  - Bug this catches: parser accepting unclosed patterns, producing garbage UUIDs

- \`test_block_ref_uuid_with_special_separators\`: Parse \`"((550e8400-e29b-41d4-a716-446655440000))"\` (UUID v4 format), verify uuid field matches exactly
  - Bug this catches: UUID normalization or truncation at special chars

**File: markymark-index/src/realm.rs (tests module)**

- \`test_journal_date_detected_underscore_separator\`: detect_journal_date("journals/2024_01_15.md") returns Some((2024, 1, 15))
  - Bug this catches: function returns None when it should match

- \`test_journal_date_detected_dash_separator\`: detect_journal_date("journals/2024-01-15.md") returns Some((2024, 1, 15))
  - Bug this catches: only one separator format supported

- \`test_journal_date_rejected_for_non_journal_filename\`: detect_journal_date("notes/meeting.md") returns None
  - Bug this catches: function matching any file with date-like substring

- \`test_journal_date_rejected_for_suffix_filename\`: detect_journal_date("journals/2024_01_15_extra_notes.md") returns None
  - Bug this catches: stem length not validated, greedy split matching extra suffix

- \`test_journal_date_rejected_for_mixed_separators\`: detect_journal_date("journals/2024-01_15.md") returns None
  - Bug this catches: accepting inconsistent separator usage

- \`test_journal_date_rejected_for_invalid_month\`: detect_journal_date("journals/2024_13_01.md") returns None (month 13 invalid)
  - Bug this catches: no range validation, accepting out-of-range dates

- \`test_realm_indexes_journal_by_date\`: Add journal doc with URI "journals/2024_01_15.md", call lookup_journal_by_month(2024, 1), verify result contains the URI with day=15
  - Bug this catches: detection runs but not stored in date_to_docs

- \`test_realm_lookup_journal_by_month_multiple\`: Add 3 journal docs for Jan 2024 and 2 for Feb 2024, lookup Jan -> 3 results, lookup Feb -> 2 results
  - Bug this catches: BTreeMap range query off by one, returns wrong month

- \`test_realm_remove_journal_doc_cleans_up_date_index\`: Add journal doc, remove it, verify lookup returns empty
  - Bug this catches: remove_document doesn't clean date_to_docs, causing stale entries

## Key Considerations (SRE Edge Cases)

### BlockRef Extraction

**Malformed Unclosed Patterns:**
The parser's extract_block_refs uses regex. If it accepts \`((missing-close\` without closing )), it would produce garbage UUIDs. Verify the regex requires closing )) before implementing. If not, the test test_malformed_block_ref_not_extracted will catch it RED.

**Same UUID Multiple Times in One Doc:**
If a doc references \`((abc))\` twice, block_refs() should return 2 entries (not deduplicated). Deduplication is the caller's concern. The arena allocation loop makes no assumptions.

**UUID Length:**
No length validation on UUID — intentional. Logseq UUIDs can be arbitrary. is_valid_block_uuid in MCP caps at 256 chars to prevent pathological inputs.

**Ordering of Block Refs:**
block_refs() order follows extraction order (document order), consistent with all other DocumentDependent slices.

### Journal Date Detection

**Filename-Only Matching (Not Directory-Based):**
The detect_journal_date function uses the filename stem only, not the parent directory. This matches Logseq behavior (journals/ is conventional but not enforced). A file at \`daily/2024_01_15.md\` IS detected as a journal page. This is intentional — Logseq users can configure their journal directory.

**Stem Length = 10 is Critical:**
detect_journal_date MUST check stem.len() == 10 before accessing byte ranges. Stems like \`24_01_15\` (2 digit year) or \`2024_01_15_extra\` must return None. The byte-range approach [0..4], [5..7], [8..10] will panic on short strings without the length guard.

**BTreeMap Range Query:**
\`date_to_docs.range((year, month, 1)..=(year, month, 31))\` correctly scans within a month because BTreeMap orders tuples lexicographically. This avoids linear scan.

**Concurrent Indexing:**
RealmIndex is not thread-safe (no Arc<Mutex>). Concurrent add_document calls would be UB. This matches existing behavior — no change needed.

**Journal Doc Replacement (Re-add Same URI):**
If add_document is called twice with same URI (document update), remove_from_cross_doc_indexes is called first. Verify that the date cleanup in remove_from_cross_doc_indexes runs BEFORE re-insertion, or the date_to_docs entry will have duplicate URIs.

### find-references MCP Tool

**Block UUID Input Ambiguity:**
The find-references tool currently takes a document URI + position. The block-ref lookup requires a UUID string. Clarify: the tool should look up block refs for the block_id at the cursor position in the source doc, OR accept a UUID directly. Check existing tool signature in markymark-mcp/src/lib.rs:362-396 and follow the existing input pattern.

**Cross-Doc Iteration Cost:**
iter_documents() scans all docs in the realm. For large workspaces (10k+ docs), this is O(n) per find-references call. This is acceptable for now (same as existing find-references behavior). No optimization needed yet.

**UUID Not Found:**
If no doc contains a BlockRef with the requested UUID, return empty block_ref_backrefs vec, not an error. This is consistent with how find-references handles unresolved wiki links.

## Anti-Patterns

- NO unwrap/expect in new code (use ?, if let, map, or structured None propagation)
- NO TODOs in implementation code
- NO hardcoded Logseq journal date format — stem length check + separator validation makes format configurable by the date's own structure
- NO new crates — no chrono dependency, use simple tuple (y, m, d) for dates
- NO modifying existing DocumentDependent fields — only ADD new fields
- NO re-implementing extract_block_refs — call ast.extract_block_refs() directly
- NO deduplication of block_refs in DocumentIndex — callers decide
- NO regex for journal date detection — use direct byte-position parsing (O(1), no backtracking risk)
- NO iterator scan for find-references block-refs unless workspace size is confirmed bounded

## Success Criteria

**Block Refs:**
- [ ] block_refs() returns all ((uuid)) entries from a parsed Logseq document (verified by test_block_refs_stored_in_document_index)
- [ ] block_refs() returns multiple entries when doc has multiple ((uuid)) refs (verified by test_multiple_block_refs_all_returned)
- [ ] Documents without block refs return &[] from block_refs() (verified by test_no_block_refs_returns_empty_slice)
- [ ] Range field in BlockRefEntry matches byte position of ((uuid)) in source (verified by test_block_ref_range_matches_position)
- [ ] Malformed ((missing-close patterns are NOT extracted (verified by test_malformed_block_ref_not_extracted)
- [ ] UUID v4 format (with dashes) preserved exactly (verified by test_block_ref_uuid_with_special_separators)

**Journal Detection:**
- [ ] detect_journal_date returns Some for YYYY_MM_DD.md filenames (verified by test_journal_date_detected_underscore_separator)
- [ ] detect_journal_date returns Some for YYYY-MM-DD.md filenames (verified by test_journal_date_detected_dash_separator)
- [ ] Non-journal filenames return None (verified by test_journal_date_rejected_for_non_journal_filename)
- [ ] Filenames with extra suffix (YYYY_MM_DD_extra.md) return None (verified by test_journal_date_rejected_for_suffix_filename)
- [ ] Mixed separators (2024-01_15.md) return None (verified by test_journal_date_rejected_for_mixed_separators)
- [ ] Invalid month values (month=13) return None (verified by test_journal_date_rejected_for_invalid_month)
- [ ] Realm indexes journal docs and lookup_journal_by_month returns them (verified by test_realm_indexes_journal_by_date)
- [ ] Multiple docs per month returned with correct days (verified by test_realm_lookup_journal_by_month_multiple)
- [ ] Removing a journal doc cleans date_to_docs (verified by test_realm_remove_journal_doc_cleans_up_date_index)

**MCP Integration:**
- [ ] find-references response includes block_ref_backrefs field
- [ ] Querying for a known block UUID returns all docs that reference it
- [ ] Querying for unknown UUID returns empty block_ref_backrefs (not an error)

**Quality Gates:**
- [ ] All 15 new tests passing (6 block-ref + 9 journal)
- [ ] All 419 existing tests still passing (no regressions)
- [ ] cargo fmt --check clean
- [ ] cargo clippy --workspace --all-targets clean
- [ ] Pre-commit hooks passing
