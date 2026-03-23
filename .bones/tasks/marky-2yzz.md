---
id: marky-2yzz
title: 'Layer 1: Introduce lasso string interner in RealmIndex'
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-n7wx]
---



## Design

## Goal
Replace all .to_string() allocations in RealmIndex cross-doc indexes with lasso string interning. HashMap keys become Spur (u32). Internal storage uses Spur; public API resolves to &str at query boundaries.

## Effort Estimate
8-10 hours (single task, steps are sequential and tightly coupled)

## Key Design Decisions (Resolved During SRE Review)

**D1: Rodeo vs ThreadedRodeo** — Use `Rodeo` (not ThreadedRodeo). RealmIndex is never shared across threads (`&mut self` for mutations). Rodeo is simpler, smaller, and requires `&mut self` for interning which matches add_document's signature. ThreadedRodeo's internal locking is unnecessary overhead.

**D2: docs HashMap key** — Keep `docs: HashMap<String, ...>` with String keys. URI strings are long (file:///path/to/file.md) and unique per document. Interning them saves nothing (no dedup) and adds lookup overhead. Only intern short, repeated strings: slugs, tag names, block IDs.

**D3: Public API boundary** — ResolvedHeading/ResolvedBlock keep `pub text: String` and `pub slug: String` fields. Internal HashMap keys use Spur. On `lookup_heading()`, resolve Spur→&str via `rodeo.resolve()` and `.to_string()` into the returned ResolvedHeading. This means we still allocate Strings on query, but we eliminate allocations during add_document (the hot path called every 75ms). Queries (lookup_heading, tag_counts) are cold paths called only on user interaction.

**D4: Interner lifetime** — Rodeo is owned by RealmIndex. It grows monotonically (never shrinks). For a 10K-doc vault with ~500K unique slugs/tags/blocks, interner holds ~25MB. Acceptable for long-running LSP. If memory pressure becomes an issue, consider periodic rebuild (create new Rodeo, re-intern all live documents).

## Implementation

### Step 1: Add lasso dependency
- `cargo add lasso -p markymark-index`
- Import: `use lasso::{Rodeo, Spur};`
- No feature gating needed

### Step 2: Write failing tests (TDD)
Tests in markymark-index/tests/realm_index.rs (existing test file):

- `test_interned_add_removes_slug_duplication`: Add doc with 3 headings sharing slug "intro". Verify slug_to_headings has exactly 1 key (not 3). Verify all 3 entries accessible via lookup_heading("intro").
- `test_remove_then_readd_same_content`: Add doc, remove doc, add same doc again. Verify lookup_heading returns correct results (interner retains old Spur, no corruption).
- `test_cross_doc_same_slug`: Add 2 docs with same slug "overview". Verify lookup_heading returns both entries. Remove first doc. Verify lookup_heading returns only second.
- `test_lookup_heading_returns_correct_strings`: Add doc with heading "Hello World" (slug "hello-world"). Call lookup_heading("hello-world"). Verify returned ResolvedHeading has text=="Hello World" and slug=="hello-world" (String values correctly resolved from Spur).
- `test_tag_counts_after_interning`: Add 2 docs with overlapping tags. Verify tag_counts() returns correct (name, count) pairs with String names.
- `test_block_lookup_returns_correct_id`: Add doc with block ID "my-block". Verify lookup_block("my-block") returns ResolvedBlock with id=="my-block".
- `test_remove_document_no_string_allocation`: Add and remove doc. Verify cross-doc maps are empty (remove uses Spur lookup, no String allocation needed internally).

### Step 3: Add interner to RealmIndex
File: markymark-index/src/realm/mod.rs

```rust
pub struct RealmIndex {
    interner: Rodeo,  // NEW: string interner for cross-doc keys
    docs: HashMap<String, (DocumentUri, AnyDocumentIndex)>,  // String key kept (D2)
    slug_to_headings: HashMap<Spur, Vec<(DocumentUri, ResolvedHeading)>>,  // CHANGED
    block_to_location: HashMap<Spur, Vec<(DocumentUri, ResolvedBlock)>>,  // CHANGED
    tag_to_docs: HashMap<Spur, Vec<DocumentUri>>,  // CHANGED
    key_path_to_docs: HashMap<String, Vec<DocumentUri>>,  // UNCHANGED (structured doc paths are unique, no dedup benefit)
    // ... date_to_docs, uri_to_date unchanged
}
```

Note: key_path_to_docs stays String because structured doc key paths (e.g. "settings.theme.color") have low repetition across documents.

### Step 4: Update add_document
File: markymark-index/src/realm/mod.rs:72-137

Replace:
```rust
// OLD: 3 String allocations per heading
let resolved = ResolvedHeading {
    text: entry.text.to_string(),
    slug: entry.slug.to_string(),
    ...
};
self.slug_to_headings
    .entry(entry.slug.to_string())  // 3rd allocation\!
    .or_default()
    .push((uri.clone(), resolved));
```

With:
```rust
// NEW: 0 String allocations for HashMap key, 2 for ResolvedHeading values
let slug_spur = self.interner.get_or_intern(entry.slug);
let resolved = ResolvedHeading {
    text: entry.text.to_string(),  // Still needed for public API (D3)
    slug: entry.slug.to_string(),  // Still needed for public API (D3)
    ...
};
self.slug_to_headings
    .entry(slug_spur)  // Spur key: 4 bytes, no heap allocation
    .or_default()
    .push((uri.clone(), resolved));
```

Same pattern for blocks and tags:
```rust
let id_spur = self.interner.get_or_intern(id);
let tag_spur = self.interner.get_or_intern(tag.name);
```

Net savings per heading: 1 String allocation (HashMap key). Per block: 1. Per tag: 1.
Total savings for 50-heading doc: ~52 fewer String allocations per edit.

### Step 5: Update remove_from_cross_doc_indexes
File: markymark-index/src/realm/mod.rs:170-244

Replace String collection + lookup:
```rust
// OLD: N String allocations just to collect keys for lookup
let slugs: Vec<String> = md_idx.headings().iter().map(|h| h.slug.to_string()).collect();
for slug in &slugs {
    if let Some(entries) = self.slug_to_headings.get_mut(slug) { ... }
}
```

With Spur lookup (zero allocation):
```rust
// NEW: Zero allocations — Spur lookup is O(1)
for entry in md_idx.headings() {
    if let Some(spur) = self.interner.get(entry.slug) {
        if let Some(entries) = self.slug_to_headings.get_mut(&spur) {
            entries.retain(|(u, _)| u.as_str() \!= key);
            if entries.is_empty() {
                self.slug_to_headings.remove(&spur);
            }
        }
    }
}
```

Same pattern for blocks and tags. This eliminates ALL String allocations in remove path.

### Step 6: Update query methods
File: markymark-index/src/realm/mod.rs:276-295

```rust
pub fn lookup_heading(&self, slug: &str) -> Vec<(DocumentUri, ResolvedHeading)> {
    // Intern lookup — returns None if slug never seen (no allocation)
    self.interner.get(slug)
        .and_then(|spur| self.slug_to_headings.get(&spur))
        .cloned()
        .unwrap_or_default()
}

pub fn lookup_block(&self, id: &str) -> Option<(DocumentUri, ResolvedBlock)> {
    self.interner.get(id)
        .and_then(|spur| self.block_to_location.get(&spur))
        .and_then(|entries| entries.first().cloned())
}

pub fn tag_counts(&self) -> Vec<(String, usize)> {
    self.tag_to_docs
        .iter()
        .map(|(spur, uris)| (self.interner.resolve(spur).to_string(), uris.len()))
        .collect()
}
```

### Step 7: Update RealmIndex::new()
Add `interner: Rodeo::default()` to constructor and Default impl.

### Step 8: Run full test suite
`cargo nextest run -p markymark-index && cargo nextest run -p markymark-lsp`
Verify zero clippy warnings: `cargo clippy --workspace --all-targets`

## Success Criteria
- [ ] HashMap keys for slug_to_headings, block_to_location, tag_to_docs are Spur (verified by type system)
- [ ] Zero String allocations in remove_from_cross_doc_indexes for key lookups (no .to_string() or .collect::<Vec<String>>())
- [ ] 7 new tests pass covering: dedup, cross-doc, remove-readd, query correctness, tag counts, block lookup, remove cleanup
- [ ] All existing tests pass (cargo nextest)
- [ ] Zero clippy warnings (cargo clippy --workspace --all-targets)
- [ ] lookup_heading, lookup_block, tag_counts return identical results as before (correctness parity verified by existing + new tests)

## Anti-Patterns (FORBIDDEN)
- ❌ NO using ThreadedRodeo (unnecessary: RealmIndex is single-threaded, Rodeo is simpler and lighter)
- ❌ NO interning URI strings (low dedup, high overhead — URIs stay as String keys in docs HashMap)
- ❌ NO exposing Spur in public API (callers should not depend on lasso types)
- ❌ NO using unwrap() on interner.get() (returns Option — handle None gracefully, it means the string was never interned)
- ❌ NO interning key_path_to_docs keys (structured doc paths have low repetition, no benefit)
- ❌ NO removing the existing ResolvedHeading/ResolvedBlock String fields (public API stability — D3)

## Key Considerations (SRE Review)

**Edge Case: Empty string interning**
lasso handles empty strings correctly (`Rodeo::get_or_intern("")` returns a valid Spur). No special handling needed.

**Edge Case: Interner memory growth**
Rodeo never deallocates interned strings. For a 10K-doc vault with ~500K unique slugs/tags/blocks (avg 20 chars each), interner holds ~10MB. Acceptable for LSP lifetime. If vault is closed and reopened, RealmIndex is reconstructed (new Rodeo). Document the growth characteristic in a code comment.

**Edge Case: Same slug across documents**
`get_or_intern("overview")` returns the same Spur regardless of which document contributed it. The Vec in `slug_to_headings[spur]` holds entries from all documents. This is the correct dedup behavior.

**Edge Case: Remove then re-add**
After removing a document, its Spur keys remain in the Rodeo (strings are never deallocated). Re-adding a document with the same slugs returns the same Spurs. This is correct — the interner acts as an append-only pool.

**Performance Note: Query path still allocates**
`lookup_heading()` returns `Vec<(DocumentUri, ResolvedHeading)>` with cloned Strings. This is acceptable because queries are cold paths (user interaction), not hot paths (every keystroke). The savings are in add_document/remove_document which run every 75ms.

**Reference: Existing pattern**
No existing interner in the codebase. This is a new pattern. Follow lasso docs: https://docs.rs/lasso/latest/lasso/

## Files
- markymark-index/Cargo.toml (add lasso)
- markymark-index/src/realm/mod.rs (main changes: struct fields, add_document, remove_from_cross_doc_indexes, query methods)
- markymark-index/src/realm/types.rs (NO changes — ResolvedHeading/ResolvedBlock keep String fields)
- markymark-index/tests/realm_index.rs (7 new tests)
