---
id: marky-ix3.1
title: 'Phase A-3: Surface code spans via LSP workspaceSymbol, hover, MCP search-symbols, and RealmIndex cross-doc index'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-vsh2, marky-2yzz]
parent: marky-ix3
---




Phase A-3 of ix3 epic. Surface code spans (extracted in A-1/A-2) to end users. Populate RealmIndex code_span_to_docs cross-doc index, add lookup_code_span(). Wire LSP workspaceSymbol to include code span matches. Add CodeSpan variant to SymbolAtPosition for hover support (shows cross-doc references). Add code span candidates to MCP search-symbols fuzzy matching. Depends on marky-vsh2 (wiring) and marky-2yzz (string interning). 7 steps: RealmIndex populate, cleanup, lookup, LSP symbol, LSP hover, MCP search, tests.

## Design

## Goal

Surface code spans extracted by Phase A-2 to end users via three channels:
1. **RealmIndex cross-doc index** — populate `code_span_to_docs` on add_document, clean up on remove
2. **LSP workspaceSymbol** — return code span matches alongside headings/tags/xml_tags
3. **LSP hover** — show cross-document references when cursor is on a code span
4. **MCP search-symbols** — include code span text as fuzzy match candidates

## Prerequisites (both DONE)

- marky-vsh2 (Phase A-2): DocumentIndex.code_spans() accessor exists, all 3 construction paths populate it
- marky-2yzz (n7wx L1): RealmIndex has lasso interner, code_span_to_docs declared as Spur-keyed HashMap, ResolvedCodeSpan type in realm/types.rs

## Steps

### Step 1: RealmIndex — populate code_span_to_docs in add_document

**File:** `markymark-index/src/realm/mod.rs` (line ~129, after tag population)

Add code span population loop following the heading/block/tag pattern:

```rust
// Populate cross-doc code span index (Spur-keyed, dedup by text per document)
let mut seen_code_spans = HashSet::new();
for cs in index.code_spans() {
    if seen_code_spans.insert(cs.text) {
        let text_spur = self.interner.get_or_intern(cs.text);
        self.code_span_to_docs
            .entry(text_spur)
            .or_default()
            .push((
                uri.clone(),
                ResolvedCodeSpan {
                    text: cs.text.to_string(),
                    range: cs.range,
                    start_byte: cs.start_byte,
                    end_byte: cs.end_byte,
                },
            ));
    }
}
```

**Dedup rationale:** Same code span text (e.g. `HashMap`) may appear 10x in one doc. Cross-doc index needs (text, uri) dedup — one entry per unique text per document.

### Step 2: RealmIndex — clean up code_span_to_docs in remove_from_cross_doc_indexes

**File:** `markymark-index/src/realm/mod.rs` (line ~225, inside `AnyDocumentIndex::Markdown` arm)

Add cleanup following the tag cleanup pattern:

```rust
let mut seen_cs = std::collections::HashSet::new();
for cs in md_idx.code_spans() {
    if seen_cs.insert(cs.text) {
        if let Some(spur) = self.interner.get(cs.text) {
            if let Some(entries) = self.code_span_to_docs.get_mut(&spur) {
                entries.retain(|(u, _)| u.as_str() != key);
                if entries.is_empty() {
                    self.code_span_to_docs.remove(&spur);
                }
            }
        }
    }
}
```

### Step 3: RealmIndex — add lookup_code_span method

**File:** `markymark-index/src/realm/mod.rs` (after lookup_block, ~line 297)

```rust
/// Look up documents containing a code span by text across all markdown documents.
pub fn lookup_code_span(&self, text: &str) -> Vec<(DocumentUri, ResolvedCodeSpan)> {
    self.interner
        .get(text)
        .and_then(|spur| self.code_span_to_docs.get(&spur))
        .cloned()
        .unwrap_or_default()
}
```

Also remove the `#[allow(dead_code)]` on `code_span_to_docs` field.

### Step 4: LSP workspaceSymbol — add code span iteration

**File:** `markymark-lsp/src/server.rs` (line ~877, after XML tag loop, before `if symbols.is_empty()`)

```rust
for cs in index.code_spans() {
    let cs_name = format!("`{}`", cs.text);
    if query.is_empty() || cs.text.to_lowercase().contains(&query) {
        let range = crate::convert::to_lsp_range(cs.range);
        #[expect(deprecated, reason = "...")]
        symbols.push(SymbolInformation {
            name: cs_name,
            kind: SymbolKind::VARIABLE,
            tags: None,
            deprecated: None,
            location: Location {
                uri: lsp_uri.clone(),
                range,
            },
            container_name: None,
        });
    }
}
```

**SymbolKind choice:** VARIABLE is the closest LSP kind for inline code references. FIELD, METHOD, etc. require knowledge of what the code span refers to (unavailable in Tier 1).

### Step 5: LSP hover — add CodeSpan variant to SymbolAtPosition

**File:** `markymark-lsp/src/state/navigation.rs`

Add to enum (line ~22):
```rust
/// An inline code span (backtick-delimited text).
CodeSpan(CodeSpanEntry<'a>),
```

Add to `symbol_at_position` (before the `None` return, ~line 101):
```rust
// Check code spans
for cs in index.code_spans() {
    if cs.range.contains(pos) {
        return Some(SymbolAtPosition::CodeSpan(cs.clone()));
    }
}
```

Add hover handler in `server.rs` (line ~650, before `StructuredKey` arm):
```rust
SymbolAtPosition::CodeSpan(cs) => {
    let mut lines = vec![format!("**`{}`** — inline code span", cs.text)];
    let refs = state.realm().lookup_code_span(cs.text);
    if refs.len() > 1 {
        lines.push(String::new());
        lines.push(format!("**Referenced in {} documents:**", refs.len()));
        for (ref_uri, _) in refs.iter().take(10) {
            lines.push(format!("- {}", ref_uri.as_str()));
        }
        if refs.len() > 10 {
            lines.push(format!("- ... and {} more", refs.len() - 10));
        }
    }
    lines.join("\n")
}
```

### Step 6: MCP search-symbols — add code span candidates

**File:** `markymark-mcp/src/engine/search.rs` (line ~53, after heading candidates loop)

```rust
// Collect code span candidates — borrow text from arena, no clone.
// Dedup by text within each document to avoid flooding results.
for (uri, index) in &docs {
    let mut seen = std::collections::HashSet::new();
    for cs in index.code_spans() {
        if seen.insert(cs.text) {
            candidates.push((Cow::Borrowed(cs.text), (*uri).clone(), cs.range));
        }
    }
}
```

### Step 7: Tests

**RealmIndex tests** (`markymark-index/src/realm/mod.rs` or separate test module):
- test_add_document_populates_code_spans — add doc with code spans, verify lookup_code_span returns them
- test_remove_document_cleans_code_spans — add then remove doc, verify lookup empty
- test_code_span_dedup_per_document — doc with same text 3x, verify 1 entry per doc
- test_code_span_cross_doc — two docs with same text, verify both in lookup result
- test_lookup_code_span_not_found — query for non-existent text returns empty vec

**LSP tests** (if test infrastructure supports it, otherwise integration-level):
- test_workspace_symbol_includes_code_spans — doc with code span, query matches, verify in results
- test_hover_on_code_span — hover at code span position, verify markdown contains text

**MCP tests** (markymark-mcp search test):
- test_search_symbols_includes_code_spans — verify code span text appears in fuzzy search candidates

## Files Changed

| File | Change |
|------|--------|
| `markymark-index/src/realm/mod.rs` | Steps 1-3: populate, cleanup, lookup_code_span |
| `markymark-lsp/src/state/navigation.rs` | Step 5: CodeSpan variant + detection |
| `markymark-lsp/src/server.rs` | Steps 4-5: workspaceSymbol + hover |
| `markymark-mcp/src/engine/search.rs` | Step 6: code span candidates |
| Test files (various) | Step 7: regression tests |

## Anti-Patterns

- NO cross-doc code span hover without dedup — same text 50x in one doc should show 1 entry per doc, not 50
- NO confidence scoring — Tier 1 is definite (backtick = code span, period)
- NO language_hint population — Tier 1 cannot determine language from backtick text alone
- NO code_span_count in realm-stats unless trivially available — avoid adding unused metrics
