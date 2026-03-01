---
title: Indexing
description: How markymark indexes documents and resolves cross-document references
---

markymark maintains two index levels: a per-document index storing every symbol
in a single file, and a per-realm index enabling cross-document lookups.

## DocumentIndex — per-document symbols

`DocumentIndex` (in `markymark-index/src/document/`) stores all extracted symbols
for one document using arena allocation (`bumpalo`) for minimal allocation overhead.

It can be built through three construction paths:

| Path | Source | Used by |
|------|--------|---------|
| `from_blob()` | Zig Document Engine binary blob | LSP (real-time) |
| `from_scan()` | `ScanBackend` trait calls | Standalone scanning |
| `from_ast()` | Tree-sitter frontmatter + Zig scan backend | MCP batch indexing |

All three produce the same `DocumentDependent` structure — typed entry slices for
headings, wiki links, markdown links, tags, XML tags, code spans, tasks, embeds,
frontmatter, properties, and more.

`DocumentIndex` uses `self_cell` so arena-allocated references remain valid for
the lifetime of the index without unsafe lifetime gymnastics.

## RealmIndex — cross-document lookups

`RealmIndex` (in `markymark-index/src/realm/`) aggregates document indexes for a
workspace. When a document is added or updated, it populates lookup tables:

| Table | Key | Purpose |
|-------|-----|---------|
| `slug_to_headings` | Heading slug | Find documents containing a heading |
| `block_to_location` | Block ID | Resolve `^block-id` references |
| `tag_to_docs` | Tag name | Find documents with a tag (lazy) |
| `code_span_to_docs` | Code span text | Cross-references to code symbols |
| `stem_to_uris` | File stem | Resolve wiki links by page name |
| `key_path_to_docs` | Key path | Structured document key lookup |
| `date_to_docs` | Journal date | Date-based document lookup (BTreeMap for range queries) |

### String interning

Cross-document HashMap keys use string interning via `lasso::Rodeo`. A slug
like `getting-started` appearing in 10 documents is stored once and referenced
by a compact `Spur` token. The interner grows monotonically — it never
deallocates strings during the LSP session lifetime. For a 10K-document vault
with ~500K unique slugs, tags, and block IDs, the interner holds roughly 10 MB.

File stems used in `stem_to_uris` are lowercased before interning, enabling
case-insensitive wiki link resolution via O(1) lookup.

### Lazy tag maintenance

The `tag_to_docs` index uses a lazy rebuild strategy to avoid patching overhead
during rapid edits. When `update_document()` detects a tag change, it sets a
`tags_dirty` flag instead of modifying the tag index immediately. The full tag
index is rebuilt from per-document contribution metadata the next time a
mutation needs it (`ensure_tags_clean()`). Read-only queries like `tag_counts()`
compute tag data directly from contributions when dirty, avoiding mutation
entirely.

## SemanticIndex — embedding search

`SemanticIndex` (in `markymark-mcp/src/engine/semantic/`) stores vector
embeddings of document sections for relevance-ranked search. It is gated
behind the `semantic-search` feature flag and supports two providers:

- **Voyage API** — cloud embeddings via the Voyage REST endpoint
- **Local ONNX** — offline embeddings via fastembed-rs

Sections are embedded in batch when a workspace root is added via `add-root`,
and re-embedded incrementally when documents change. The index uses
snapshot-then-rollback for atomicity — if embedding fails mid-document, no
partial entries are left behind.

## Cross-document resolution

The resolution module (`markymark-index/src/resolution.rs`) resolves link targets:

- **Wiki links** — `resolve_wiki_link()` finds documents by page name (stem
  matching), optionally resolving a heading anchor within
- **Markdown links** — `resolve_markdown_link()` tries path-relative resolution
  first, falling back to stem-only lookup
- **Block references** — `resolve_block_ref()` looks up `^id` across all documents

Path-relative resolution uses component-stack normalization rather than filesystem
`canonicalize()`, so it works without the target file existing on disk.

## Diagnostics

`compute_diagnostics()` in `markymark-index/src/diagnostics.rs` checks a document
against its realm and reports broken wiki links, broken markdown links, broken
heading anchors, and duplicate heading slugs. This function is shared between LSP
and MCP — both call the same code.

## Index updates

Index updates are event-driven through the LSP protocol, not filesystem watching:

1. `textDocument/didOpen` — document parsed and added to the realm
2. `textDocument/didChange` — Document Engine re-parses, index incrementally updated
3. `textDocument/didClose` — document removed from the realm

A 75ms debounce coalesces rapid keystrokes into a single reparse cycle. For MCP,
documents are indexed in batch when a workspace root is added via `add-root`.

### Incremental cross-document updates

When a document changes, `update_document()` diffs the old and new
`DocContribution` — a per-document snapshot of which interned keys (heading
slugs, block IDs, tags, code spans, file stem) that document contributed to
cross-document indexes.

- **Fast path:** If the contribution sets are identical (common for edits that
  don't change document structure), all cross-document index operations are
  skipped — only the stored `DocumentIndex` is swapped.
- **Slow path:** When contributions differ, only the changed entries are
  patched. Added slugs are inserted into `slug_to_headings`, removed slugs are
  cleaned out, and so on for blocks, code spans, and stems. Tags use the lazy
  strategy described above.
