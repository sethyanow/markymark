---
id: marky-ck1
title: 'Task 3: Implement search-workspace MCP tool'
status: closed
type: feature
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-waw]
parent: marky-9mo
---




## Design

## Goal
Add search-workspace MCP tool that queries the enriched DocumentIndex for full-text + frontmatter + property searches with ranked results.

## Context
Tasks 1 & 2 complete: DocumentIndex now stores frontmatter, aliases, properties, block_refs, and RealmIndex tracks journal dates. CoreEngine has SearchSymbols and SemanticSearch patterns to follow. The new tool needs a CoreOperation variant + CoreOperationResult variant + runtime engine implementation + MCP handler.

**CRITICAL FILE SIZE SITUATION:**
- markymark-mcp/src/runtime_engine.rs: 1246 lines (HARD STOP already violated — must split)
- markymark-mcp/src/lib.rs: 779 lines (will exceed 1000 with 3 more tools — split now)
- This task MUST split runtime_engine.rs as part of implementation.

## Architecture

### Step 1: Add CoreOperation::SearchWorkspace + CoreOperationResult::WorkspaceSearchResults (markymark-core/src/engine.rs)

Add WorkspaceSearchResult struct to engine.rs BEFORE the CoreOperation enum:
```rust
#[derive(Debug, Clone)]
pub struct WorkspaceSearchResult {
    pub uri: DocumentUri,
    pub title: String,              // first H1 text, or filename from URI (strip .md, convert _ to space)
    pub score: f32,                 // 0.0-1.0: title match=1.0, heading match=0.8, frontmatter/property match=0.6, no-query filter match=1.0
    pub matched_fields: Vec<String>,// e.g. ["title", "frontmatter:status", "property:type", "heading"]
    pub frontmatter_preview: Vec<(String, String)>, // first 3 frontmatter k/v (value as string regardless of type)
    pub property_preview: Vec<(String, String)>,    // first 3 Logseq property k/v
    pub tags: Vec<String>,          // all #tag values
    pub is_journal: bool,
    pub journal_date: Option<(u16, u8, u8)>,
}
```

Add to CoreOperation enum:
```rust
/// Search workspace documents by text, frontmatter, and property queries.
SearchWorkspace {
    /// Free-text search query. Case-insensitive substring match against title, heading text, frontmatter values, property values.
    /// None means no text filter — return all docs matching other filters.
    query: Option<String>,
    /// Filter: only include docs where frontmatter key = value (case-insensitive, exact key match, substring value match).
    frontmatter_filter: Option<(String, String)>,
    /// Filter: only include docs where Logseq property key = value (case-insensitive exact key, substring value).
    property_filter: Option<(String, String)>,
    /// Filter: only include docs that have this tag (case-insensitive, exact tag name match after #).
    tag_filter: Option<String>,
    /// Realm to search. Defaults to "default" when None.
    realm: Option<String>,
    /// Max results to return. 0 = return empty (not an error). Default 20 when not set. Max 100 (clamp silently).
    limit: u32,
}
```

Add to CoreOperationResult enum:
```rust
WorkspaceSearchResults {
    realm: String,
    query: Option<String>,
    results: Vec<WorkspaceSearchResult>,
}
```

### Step 2: Split markymark-mcp/src/runtime_engine.rs FIRST (1246 lines → ~900 + new modules)

HARD STOP: runtime_engine.rs is 1246 lines. Split BEFORE adding new operation:

Extract to:
- markymark-mcp/src/engine/search.rs — SearchSymbols, SemanticSearch, future SearchWorkspace
- markymark-mcp/src/engine/graph.rs — DependencyGraph, ConnectionGraph helpers
- markymark-mcp/src/engine/export.rs — ExportIndex, stat helpers, dto rendering
- markymark-mcp/src/engine/realm_ops.rs — CreateRealm, DestroyRealm, AddRoot, RemoveRoot, RealmStats
- markymark-mcp/src/engine/references.rs — FindReferences, Rename
- markymark-mcp/src/engine/outline.rs — GetOutline
- markymark-mcp/src/engine/mod.rs — RuntimeEngine struct, execute(), helpers (DEFAULT_REALM, index_root, unindex_root, fuzzy_match helpers)
- runtime_engine.rs → remove, replace with engine/ directory

Each extracted file must be under 400 lines. engine/mod.rs must be under 500 lines.

### Step 3: Implement SearchWorkspace in markymark-mcp/src/engine/search.rs

Add function:
```rust
pub(crate) fn execute_search_workspace(
    state: &std::sync::RwLockReadGuard<'_, RealmMap>,
    query: Option<String>,
    frontmatter_filter: Option<(String, String)>,
    property_filter: Option<(String, String)>,
    tag_filter: Option<String>,
    realm_name: Option<String>,
    limit: u32,
) -> CoreOperationResult
```

Algorithm:
1. `let realm_key = realm_name.as_deref().unwrap_or(DEFAULT_REALM);`
2. Get realm or return `CoreOperationResult::Error(CoreError::Message(format\!("realm does not exist: {realm_key}")))`
3. Clamp limit: `let limit = limit.min(100) as usize;`
4. If limit == 0: return `CoreOperationResult::WorkspaceSearchResults { realm: realm_key.to_string(), query, results: vec\![] }`
5. Iterate `realm.index.iter_documents()` for `(uri, doc)`:
   a. **Filter phase** (AND logic — all active filters must match):
      - frontmatter_filter: find frontmatter entry where `entry.key.eq_ignore_ascii_case(key)` AND `entry.value.to_string().to_lowercase().contains(&value.to_lowercase())`
      - property_filter: find property entry where key case-insensitive match AND value contains (case-insensitive)
      - tag_filter: find tag entry where `tag.name.eq_ignore_ascii_case(filter)`
      - Frontmatter list values (e.g. aliases: [a, b, c]): check if ANY element contains the filter value
   b. **Score phase** (only if filters passed):
      - Extract title: `doc.headings().iter().find(|h| h.level == 1).map(|h| h.text.to_string()).unwrap_or_else(|| uri_to_title(uri))`
      - If query is None: score = 1.0, matched_fields = []
      - If query is Some(q): case-insensitive substring search (q.to_lowercase()):
        * title contains q → score = 1.0, push "title" to matched_fields
        * any heading contains q → max(score, 0.8), push "heading"
        * any frontmatter value contains q → max(score, 0.6), push "frontmatter:{key}"
        * any property value contains q → max(score, 0.6), push "property:{key}"
        * If score still 0.0 after query: SKIP this doc (query not matched)
   c. Build WorkspaceSearchResult from doc data
6. Collect results, sort by score DESC then URI ASC (deterministic for ties)
7. Take first `limit` results
8. Return `CoreOperationResult::WorkspaceSearchResults { ... }`

Helper functions (private to search.rs):
```rust
fn uri_to_title(uri: &DocumentUri) -> String
// Extract filename from URI path, strip .md/.mdx, convert _ and - to space, titlecase first word
// e.g. "file:///vault/journals/2024_01_15.md" → "2024 01 15"
// e.g. "file:///vault/notes/project_design.md" → "project design"

fn frontmatter_value_to_string(value: &FrontmatterValue) -> String
// Convert FrontmatterValue to display string for filtering and preview
// List variants: join with ", "
// Bool: "true"/"false"
// Number: numeric string
```

### Step 4: Add MCP tool handler in markymark-mcp/src/tools/search.rs (new tools/ submodule)

Split lib.rs SIMULTANEOUSLY with adding search-workspace tool:
- markymark-mcp/src/tools/search.rs — get_outline_tool, search_symbols_tool, semantic_search_tool, search_workspace_tool (NEW)
- markymark-mcp/src/tools/outline.rs — export_index_tool
- markymark-mcp/src/tools/refs.rs — find_references_tool, rename_tool
- markymark-mcp/src/tools/realm.rs — create_realm_tool, destroy_realm_tool, add_root_tool, remove_root_tool, realm_stats_tool
- markymark-mcp/src/tools/mod.rs — re-exports all tool fns
- markymark-mcp/src/lib.rs — server struct, list_tools, serve_stdio, helper fns only (under 300 lines)

Tool JSON schema (add to list_tools):
```json
{
  "name": "search-workspace",
  "description": "Search workspace documents by free text, frontmatter, properties, or tags. Returns ranked results with metadata preview.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "Free-text search (case-insensitive substring). Optional." },
      "frontmatter_filter_key": { "type": "string" },
      "frontmatter_filter_value": { "type": "string" },
      "property_filter_key": { "type": "string" },
      "property_filter_value": { "type": "string" },
      "tag_filter": { "type": "string", "description": "Tag name without # prefix" },
      "realm": { "type": "string" },
      "limit": { "type": "integer", "description": "Max results (0-100, default 20)" }
    }
  }
}
```

Validation in handler:
- frontmatter_filter_key and frontmatter_filter_value must both be present or both absent (return tool_error if only one)
- Same for property_filter_key/value
- If all of query, frontmatter_filter, property_filter, tag_filter are absent: treat as "list all" (no error, just return up to limit docs)

Output format (text body):
```
Found N results in realm "default"
---
[score: 0.95] vault/notes/ProjectX.md
  Title: Project X Design
  Tags: [project, active]
  Frontmatter: status=active, priority=high
  Properties: (none)
  Journal: false
  Matched: title, frontmatter:status

[score: 0.80] vault/journals/2024_01_15.md
  Title: 2024 01 15
  Tags: [daily]
  Frontmatter: (none)
  Properties: type=daily
  Journal: true (2024-01-15)
  Matched: property:type
---
```

If 0 results: return "No results found in realm \"default\" for query: [query text]"

### Step 5: Tests (TDD — RED first, all tests must fail before implementation)

**markymark-mcp/src/engine/search.rs tests module** (or tests/search_workspace_tests.rs):

test_search_workspace_case_insensitive_query:
  Doc with H1 "Project Alpha", query="project alpha" (lowercase) → result found with score=1.0, matched_fields contains "title"
  Bug caught: case-sensitive match silently drops results

test_search_workspace_returns_empty_for_no_matches:
  Realm with 2 docs (titles "Foo" and "Bar"), query="nonexistent_xyz" → results is empty vec
  Bug caught: query matching returns wrong docs

test_search_workspace_title_match_scores_1_0:
  Doc with H1 exactly matching query → score = 1.0
  Doc with heading (H2) matching query → score = 0.8 (not 1.0)
  Bug caught: title and heading scoring swapped

test_search_workspace_frontmatter_filter_exact_key:
  Two docs: one has frontmatter status=active, one has frontmatter statue=active (typo)
  Filter key="status" → only exact match doc returned ("statue" doc excluded)
  Bug caught: partial key match returning wrong docs

test_search_workspace_frontmatter_filter_case_insensitive_value:
  Doc has frontmatter status="Active", filter value="active" → doc included
  Bug caught: case-sensitive value comparison drops valid results

test_search_workspace_frontmatter_list_value_any_element_matches:
  Doc has frontmatter aliases=["Project X", "Proj X", "PX"]
  Filter key="aliases", value="proj x" → doc included (case-insensitive, matches second element)
  Bug caught: list values collapsed to string, partial match fails

test_search_workspace_property_filter:
  Two docs: type:: daily and type:: note
  Filter property key="type", value="daily" → only daily doc returned
  Bug caught: property filter not applied

test_search_workspace_tag_filter_case_insensitive:
  Doc has tag #Project (uppercase P), filter tag="project" → doc included
  Bug caught: case-sensitive tag matching

test_search_workspace_multiple_filters_and_logic:
  Doc A: frontmatter status=active, tag #project
  Doc B: frontmatter status=active, tag #daily
  Filter frontmatter status=active AND tag=project → only Doc A returned
  Bug caught: OR instead of AND logic for multiple filters

test_search_workspace_respects_limit:
  10 docs all matching query, limit=3 → exactly 3 results returned, in score DESC order
  Bug caught: limit not applied, or sorted wrong direction

test_search_workspace_limit_zero_returns_empty:
  3 docs in realm, limit=0 → empty results, no error
  Bug caught: limit=0 causes panic or returns all docs

test_search_workspace_empty_realm_returns_empty:
  Empty realm (no docs indexed), any query → empty results, no error
  Bug caught: iter_documents on empty realm panics

test_search_workspace_no_query_no_filter_returns_all_up_to_limit:
  5 docs in realm, no query, no filters, limit=10 → all 5 docs returned, score=1.0 each
  Bug caught: no-filter path broken or no-query path errors

test_search_workspace_sort_descending_score_then_uri_ascending:
  3 docs: scores 0.6, 1.0, 0.8 → returned in order 1.0, 0.8, 0.6
  Two docs with same score=0.8: sorted by URI ascending (deterministic)
  Bug caught: unstable sort, non-deterministic output across runs

## Success Criteria
- [ ] WorkspaceSearchResult struct in markymark-core/src/engine.rs with all fields
- [ ] CoreOperation::SearchWorkspace with query (Optional), frontmatter_filter (Optional), property_filter (Optional), tag_filter (Optional), realm (Optional), limit (u32)
- [ ] CoreOperationResult::WorkspaceSearchResults variant added
- [ ] runtime_engine.rs SPLIT into engine/ submodule — no single file exceeds 500 lines
- [ ] lib.rs SPLIT into tools/ submodule — lib.rs under 300 lines after split
- [ ] execute_search_workspace: case-insensitive substring matching, AND filter logic, score DESC + URI ASC sort, limit clamped to 100
- [ ] MCP search-workspace tool added with all 8 params and formatted text output
- [ ] 13+ new tests written FIRST (RED), then GREEN
- [ ] All existing workspace tests pass (no regressions)
- [ ] cargo fmt --check clean
- [ ] cargo clippy --workspace --all-targets clean
- [ ] No unwrap/expect in new code
- [ ] No TODO without issue number in new code

## Anti-Patterns
- NO write operations in search-workspace (read-only per epic anti-pattern)
- NO Dataview query DSL (epic anti-pattern: keep filters simple key=value)
- NO full-text content indexing (search titles, headings, metadata only — not raw markdown bytes)
- NO separate crate for search (extend existing markymark-mcp per epic anti-pattern)
- NO unwrap/expect in new code
- NO TODO without issue number in new code
- NO runtime_engine.rs over 400 lines (must split — already at 1246, a HARD STOP)
- NO lib.rs over 300 lines after split
- NO case-sensitive string comparisons for user-facing query/filter fields
- NO panic on limit=0, empty realm, or missing realm (return appropriate error/empty)

## Edge Cases to Handle
- limit=0: return empty WorkspaceSearchResults (no error)
- Empty realm (0 docs): return empty WorkspaceSearchResults (no error)
- Missing realm name: return CoreError with realm name in message
- query=None with filters: score=1.0 for all matching docs, sort by URI ascending
- Frontmatter list values (aliases: [a, b, c]): filter checks if ANY element matches (case-insensitive substring)
- Score ties: secondary sort by URI ascending for deterministic output
- limit > 100: silently clamp to 100 (do not error)
- frontmatter_filter_key present but frontmatter_filter_value absent (or vice versa): tool_error (not a panic)
