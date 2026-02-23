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
| `from_ast()` | Tree-sitter AST + regex extraction | MCP batch indexing |

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
| `tag_to_docs` | Tag name | Find documents with a tag |
| `code_span_to_docs` | Code span text | Cross-references to code symbols |
| `key_path_to_docs` | Key path | Structured document key lookup |

Cross-document HashMap keys use string interning (`lasso::Rodeo`) to avoid
duplicate allocations. A slug like `getting-started` appearing in 10 documents
is stored once and referenced by a compact `Spur` token.

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
2. `textDocument/didChange` — Document Engine re-parses, old index replaced
3. `textDocument/didClose` — document removed from the realm

A 75ms debounce coalesces rapid keystrokes into a single reparse cycle. For MCP,
documents are indexed in batch when a workspace root is added via `add-root`.
