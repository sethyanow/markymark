# Semantic Index and Block Model Investigation

**Date:** 2026-03-05
**Scope:** Comprehensive analysis of markymark's semantic indexing, search infrastructure, and document index structure to inform Block model design.

---

## 1. SemanticEntry Struct

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/types.rs`

```rust
/// Semantic metadata for a heading-level search entry.
#[derive(Debug, Clone)]
pub struct SemanticEntry {
    /// Document URI containing this entry.
    pub doc_uri: DocumentUri,
    /// Heading text used as semantic label.
    pub heading: String,
    /// Markdown heading level (1-6).
    pub heading_level: u8,
    /// Section start position.
    pub section_start: Position,
    /// Section end position.
    pub section_end: Position,
}
```

**Key findings:**
- SemanticEntry is **heading-centric**: it encapsulates a heading and its section bounds
- No embedded vector storage in the struct itself; vectors are stored separately in `ZigEmbeddingIndex`
- `section_start` and `section_end` define the range of content "under" the heading (from the heading start to the next heading start, or EOF)
- No `embedding_input` field in SemanticEntry; instead, the embedding is derived from the heading text and stored by ID in the Zig index

---

## 2. build_document_plan() Function

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/ops_add_remove.rs` (lines 22–95)

**Purpose:** Converts a markdown document's headings into a plan for semantic indexing (internal staging before embedding).

```rust
struct EntryPlan {
    id: String,                    // e.g., "file://doc.md#heading-slug#0"
    embedding_input: String,       // The heading text to be embedded
    entry: SemanticEntry,
}

struct DocumentPlan {
    uri: DocumentUri,
    ids: Vec<String>,              // All entry IDs for this document
    token_set: BTreeSet<u32>,      // Token hashes for duplicate detection
    entries: Vec<EntryPlan>,
}

fn build_document_plan(uri: &DocumentUri, index: &DocumentIndex) -> DocumentPlan
```

**Algorithm:**

1. **If document has no headings or all headings are blank:**
   - Create a **fallback entry** with ID format: `{uri}#fallback`
   - Fallback text is derived from the document filename (without extension)
   - Fallback heading level = 1
   - `section_start` and `section_end` = `Position::new(0, 0)` (document start)

2. **If document has non-blank headings:**
   - For each heading with non-empty text:
     - ID format: `{uri}#{heading.slug}#{index}`
     - `embedding_input`: heading text as-is
     - `section_start`: heading's range start
     - `section_end`: heading's range end (note: range is the heading line only, NOT the section below)
   - Collect token hashes from all non-blank headings for duplicate detection

**Critical detail:** `section_start` and `section_end` in the built entries represent only the heading range itself, not the full section (which would extend to the next heading). This is updated later during search to provide a full range.

---

## 3. SemanticIndex Structure

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/mod.rs` (lines 17–52)

```rust
pub struct SemanticIndex {
    provider: Arc<dyn EmbeddingProvider>,           // Embedding model (external, e.g., Claude)
    index: ZigEmbeddingIndex,                       // Zig-based vector index
    entries_by_id: HashMap<String, SemanticEntry>, // Entry metadata by stable ID
    doc_to_ids: HashMap<DocumentUri, Vec<String>>,// Doc → list of entry IDs
    doc_token_sets: HashMap<DocumentUri, BTreeSet<u32>>, // For duplicate detection
}
```

**Key methods:**
- `entry_count()`: Returns `self.entries_by_id.len()`
- `add_document()`: Embeds all headings and adds vectors + metadata to the index
- `update_document()`: Incrementally updates entries (diffs old vs new headings by text)
- `remove_document()`: Removes all entries for a document
- `search()`: Embeds query, calls `search_with_embedding()`
- `search_with_embedding()`: In-memory vector search using `ZigEmbeddingIndex`
- `detect_duplicates()`: Computes Jaccard similarity over token hashes

---

## 4. Search Implementation: RealmIndex::semantic_search()

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/realm/mod.rs` (lines 871–884)

```rust
#[cfg(feature = "embeddings")]
pub async fn semantic_search(
    &self,
    query: &str,
    top_k: u32,
    min_score: f32,
) -> Result<Vec<SearchResult>, EmbedError>
```

**Behavior:**

1. Acquires lock on inner `TokioMutex<SemanticIndex>`
2. Calls `guard.search(query, top_k, min_score).await`

**Warning:** This holds the mutex for the duration of embedding, which can be slow. Callers who hold an outer lock should use `semantic_index_arc()` instead and manage locking themselves.

---

## 5. SemanticIndex::search_with_embedding()

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/ops_update_search.rs` (lines 243–283)

**Purpose:** Fast in-memory search using a pre-computed query embedding (avoids re-embedding under a lock).

```rust
pub fn search_with_embedding(
    &self,
    query_embedding: &[f32],
    top_k: u32,
    min_score: f32,
) -> Result<Vec<SearchResult>, EmbedError>
```

**Algorithm:**

1. Return empty if `top_k == 0` or no entries exist
2. Clamp `min_score` to [0.0, 1.0]
3. Compute `fetch_k` = slightly larger k to account for stale entries in Zig index
4. Call `self.index.search(query_embedding, fetch_k)` (Zig FFI)
5. For each candidate:
   - Skip if score < `min_score`
   - Look up entry metadata from `entries_by_id` (skip if missing — stale entry)
   - Build `SearchResult` with doc URI, heading, level, score, and section range
   - Return when output reaches `top_k` results

**Returns:** `Vec<SearchResult>` (see definition below)

---

## 6. SearchResult Struct

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/types.rs`

```rust
/// Semantic search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Matched document URI.
    pub doc_uri: DocumentUri,
    /// Matched heading text.
    pub heading: String,
    /// Matched heading level.
    pub heading_level: u8,
    /// Similarity score.
    pub score: f32,
    /// Source range for the matched heading/section.
    pub section_range: Range,
}
```

**Cross-transport representation:** `SemanticSearchMatch` in core (lines 33–46 of engine.rs):

```rust
pub struct SemanticSearchMatch {
    pub doc_uri: DocumentUri,
    pub heading: String,
    pub heading_level: u8,
    pub score: f32,
    pub section_range: Range,
    pub section_preview: String,  // Short snippet from the section
}
```

---

## 7. RealmIndex Search Methods

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/realm/mod.rs`

### 7.1 lookup_heading()

```rust
pub fn lookup_heading(&self, slug: &str) -> Vec<(DocumentUri, ResolvedHeading)>
```

- Cross-document heading lookup by slug
- Returns all documents that define the slug

### 7.2 lookup_block()

```rust
pub fn lookup_block(&self, id: &str) -> Option<(DocumentUri, ResolvedBlock)>
```

- Cross-document block ID lookup (Obsidian `^block-id`)
- Returns at most one result (IDs are unique)

### 7.3 lookup_code_span()

```rust
pub fn lookup_code_span(&self, text: &str) -> Vec<(DocumentUri, ResolvedCodeSpan)>
```

- Cross-document code span lookup (inline code text)
- Returns all documents containing the span

### 7.4 tag_counts()

```rust
pub fn tag_counts(&self) -> Vec<(String, usize)>
```

- Returns all tags with usage counts across the realm

### 7.5 search_key_paths()

```rust
pub fn search_key_paths(
    &self,
    query: &str,
) -> Vec<(DocumentUri, String, String, ValueKind, Range)>
```

- Searches structured documents (YAML, JSON, TOML) for key paths
- Returns (uri, path, key, value_kind, range) tuples

---

## 8. DocumentIndex: What's Indexed Per Document

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/document/mod.rs` and `types.rs`

**DocumentIndex stores (for a single markdown document):**

1. **HeadingEntry** — headings with level, text, slug, range
2. **BlockEntry** — block IDs (Obsidian `^id`)
3. **WikiLinkEntry** — wiki-style links (`[[target]]`, `[[target|label]]`)
4. **TagEntry** — inline tags (`#tag`, `#nested/tag`)
5. **MarkdownLinkEntry** — markdown links (`[text](url)`)
6. **XmlTagEntry** — XML-style tags (`<tag>...</tag>`)
7. **CodeSpanEntry** — inline code spans (`` `code` ``)
8. **FrontmatterEntry** — YAML/TOML frontmatter key-value pairs
9. **PropertyEntry** — Logseq-style properties
10. **BlockRefEntry** — block references (`((uuid))`)
11. **EmbedEntry** — embeds (`![[target]]`)
12. **TaskEntry** — task/checkbox items (`- [ ] task`)
13. **CalloutEntry** — Obsidian callout blocks (`> [!type]`)
14. **QueryBlockEntry** — Logseq query blocks (`{{query ...}}`)
15. **LinkDefinitionEntry** — link definitions (`[label]: url "title"`)

**Lookup methods:**
- `headings()` → `&[HeadingEntry]`
- `heading_by_slug(slug)` → `Option<&HeadingEntry>`
- `block_by_id(id)` → `Option<&BlockEntry>`
- `toc()` → `&[TocEntry]` (flat table of contents)
- `outline()` → `&OutlineNode` (hierarchical outline tree)
- `wiki_links()`, `tags()`, `markdown_links()`, `xml_tags()`, `code_spans()`, `frontmatter()`, etc.

---

## 9. How Consumers Use the Index

### 9.1 LSP Server (markymark-lsp)

**Navigation/Symbols:**
- `symbol_at_position()` in `state/navigation.rs`: Identifies what symbol (heading, block, tag, etc.) is at the cursor
- `symbols.rs`: Converts heading outline and XML tags to LSP `DocumentSymbol` entries
- Calls `RealmIndex::lookup_heading()`, `lookup_block()`, `lookup_code_span()` for cross-doc references

**Lookups:**
- `RealmIndex::get_document(uri)` → `Option<&DocumentIndex>` (single document)
- `RealmIndex::iter_documents()` → iterate all markdown documents

### 9.2 MCP Server (markymark-mcp)

**Search Tools:**

1. **search-symbols:** Fuzzy search on heading text + symbol names across documents
   - Calls `CoreOperation::SearchSymbols` → engine searches heading text

2. **semantic-search:** Vector-based semantic search over heading sections
   - Calls `CoreOperation::SemanticSearch` → `RealmIndex::semantic_search(query, top_k, min_score)`
   - Returns `SemanticSearchMatch` with doc, heading, level, score, section range, and preview

3. **search-workspace:** Full-text search with optional frontmatter/property/tag filters
   - Filters on document frontmatter, Logseq properties, tags
   - Searches across headings, links, blocks, etc.

4. **search-for-pattern:** Regex pattern search across documents

**Document Access:**
- `RealmIndex::iter_all_documents()` → iterate markdown + structured documents
- `RealmIndex::get_any_document(uri)` → `Option<&AnyDocumentIndex>` (markdown or structured)

---

## 10. Tree-sitter Markdown Node Types

Location: `markymark-parser/src/types/elements.rs`

**Supported element types:**
- `"atx_heading"` and `"setext_heading"` → Heading
- `"paragraph"` → Paragraph
- `"list_item"` → ListItem
- Other node kinds are parsed but mapped to placeholder `Element::Other`

**Blocks supported via index entries:**
- Lists with task items (`` - [ ] task ``)
- Callout blocks (`` > [!type] content ``)
- Query blocks (`` {{query ...}} ``)
- Code blocks (fence or indented)

**Inline elements indexed:**
- Wiki links (`[[...]]`)
- Markdown links (`[text](url)`)
- Inline code (`` `code` ``)
- Tags (`#tag`)
- XML tags
- Block references (`((uuid))`)

---

## 11. Duplicate Detection: detect_duplicates()

Location: `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/ops_update_search.rs` (lines 286–326)

```rust
pub fn detect_duplicates(&self, threshold: f32) -> Vec<DuplicateMatch>
```

**Algorithm:**

1. Collect all document URIs from `doc_token_sets`
2. For each pair (i < j):
   - Compute **Jaccard similarity** = |A ∩ B| / |A ∪ B| over token hash sets
   - Include pair if similarity ≥ threshold
3. Sort results by similarity (descending), then URI (ascending)

**Returns:** `Vec<DuplicateMatch>`

```rust
pub struct DuplicateMatch {
    pub doc_uri_a: DocumentUri,
    pub doc_uri_b: DocumentUri,
    pub similarity: f32,  // [0.0, 1.0]
}
```

---

## 12. Summary: What the Block Model Should Know

### Design Assumptions to Verify

1. **Headings are first-class search targets:** YES. SemanticEntry is heading-centric. Every heading becomes a searchable "block."
   - Fallback: documents with no headings create a single entry labeled by filename

2. **Section bounds are heading + content below:** PARTIALLY. `section_start`/`section_end` in SemanticEntry currently store only the heading range, not the full section. Full section semantics may need clarification in consumer code.

3. **Search returns heading-level results:** YES. `SearchResult` always includes heading text, level, and range. No paragraph-level or arbitrary-block-level search results.

4. **Duplicate detection uses token hashes:** YES. `doc_token_sets` stores token hashes (see `helpers::token_hashes()`). Jaccard similarity is computed over these sets.

5. **Cross-document lookups are slug-based (headings), ID-based (blocks), or text-based (code spans):** YES.
   - Headings: `lookup_heading(slug)` → `Vec<(DocumentUri, ResolvedHeading)>`
   - Blocks: `lookup_block(id)` → `Option<(DocumentUri, ResolvedBlock)>`
   - Code spans: `lookup_code_span(text)` → `Vec<(DocumentUri, ResolvedCodeSpan)>`

### For Block Model Design

**Implications:**

1. **If Block = heading-section pair:** The model should clarify how `section_end` is determined. Currently, it's the heading's own range end. Is this intentional, or should it extend to the next heading?

2. **If Block = any indexed element (heading, block ID, code span, tag, etc.):** Multiple entry types already support cross-document lookup; the semantic index only uses headings.

3. **Embedding scope:** Embeddings are per-heading. If finer granularity is desired (e.g., per-paragraph blocks), the embedding and indexing pipeline would need architectural changes.

4. **Incremental updates:** `SemanticIndex::update_document()` diffs by heading text; headings are matched across old/new via their text, not via stable ID. Restructured blocks would need similar diffing logic.

---

## 13. File Locations (Quick Reference)

| Component | Path |
|-----------|------|
| SemanticEntry, SearchResult | `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/types.rs` |
| build_document_plan, add_document, remove_document | `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/ops_add_remove.rs` |
| update_document, search, search_with_embedding, detect_duplicates | `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/ops_update_search.rs` |
| SemanticIndex struct | `/Volumes/code/markymark_worktrees/dev/markymark-index/src/semantic/mod.rs` |
| RealmIndex (search, lookup methods) | `/Volumes/code/markymark_worktrees/dev/markymark-index/src/realm/mod.rs` |
| DocumentIndex, HeadingEntry, BlockEntry, etc. | `/Volumes/code/markymark_worktrees/dev/markymark-index/src/document/mod.rs` and `types.rs` |
| Element, Heading, Paragraph, ListItem | `/Volumes/code/markymark_worktrees/dev/markymark-parser/src/types/elements.rs` |
| SemanticSearchMatch, CoreOperation | `/Volumes/code/markymark_worktrees/dev/markymark-core/src/engine.rs` |
| LSP search/navigation | `/Volumes/code/markymark_worktrees/dev/markymark-lsp/src/` |
| MCP search tools | `/Volumes/code/markymark_worktrees/dev/markymark-mcp/src/tools/search.rs` |

---

## Open Questions for Block Model Design

1. **Full section semantics:** Should `section_end` in SemanticEntry extend to the next heading, or remain as the heading's own end?
2. **Sub-heading blocks:** If a level-2 heading should be a "block" within a level-1 heading's section, how does the model handle nesting?
3. **Non-heading blocks:** Should paragraphs, code blocks, lists, callouts, or query blocks be first-class searchable blocks, or remain metadata within a heading's section?
4. **Embedding granularity:** Should embeddings be per-heading (current) or per-paragraph/block (would require significant changes)?
5. **Duplicate detection scope:** Should it remain heading-based (current) or expand to other block types?
