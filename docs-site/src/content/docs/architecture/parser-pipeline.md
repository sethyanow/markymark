---
title: Parser Pipeline
description: How markymark parses markdown using tree-sitter and the md4c Zig engine
---

markymark uses two markdown parsers for different purposes, plus dedicated parsers
for structured formats.

## Two markdown parsers

### md4c (primary — LSP)

The primary path uses [md4c](https://github.com/mity/md4c), a fast CommonMark
parser accessed through Zig bindings in `markymark-kernels`. This runs on every
keystroke in your editor.

The LSP flow:

1. Editor sends `textDocument/didChange` with updated text
2. The **Document Engine** (a stateful Zig object per document) re-parses via md4c
3. The `ExtractionRenderer` callback extracts symbols: headings, links, tags,
   block IDs, code spans, tasks, and embeds
4. Results are serialized into a flat binary blob
5. Rust calls `DocumentIndex::from_blob()` to build the per-document index

### Tree-sitter (secondary — MCP batch, hover)

[Tree-sitter](https://tree-sitter.github.io/) provides a full syntax tree for:

- **MCP batch indexing** — `from_ast()` extracts frontmatter via tree-sitter,
  then delegates all content extraction to the Zig scan backend
- **Hover and go-to-definition** — precise AST node positions

The `Parser` struct in `markymark-parser/src/lib.rs` wraps tree-sitter and
produces an `Ast` — markymark's internal representation.

## Symbol extraction

Both parsers extract the same symbol types into `DocumentIndex`:

| Symbol | Example |
|--------|---------|
| Headings | `## Title` with slug, level, position |
| Wiki links | `[[target]]`, `[[target\|alias]]` |
| Markdown links | `[text](url#anchor)` |
| Tags | `#tag` |
| XML tags | Custom XML elements |
| Block IDs | `^block-id` |
| Code spans | `` `symbol` `` |
| Tasks | `- [ ]` / `- [x]` |
| Embeds | `![[target]]` |
| Frontmatter | YAML/TOML key-value pairs |
| Properties | `key:: value` (Logseq) |
| Callouts | `> [!type] Title` (Obsidian) |
| Query blocks | `{{query ...}}` (Logseq) |
| Link definitions | `[label]: url "title"` |
| Block references | `((uuid))` (Logseq) |

XML tags are a special case — md4c treats HTML as pass-through, so XML extraction
runs as a separate scan step after parsing.

## Structured format parsing

Non-markdown files take a separate path through `markymark-parser/src/structured/`:

| Format | Parser |
|--------|--------|
| JSON, JSONC | tree-sitter-json |
| JSON5 | `json5` crate |
| JSONL | Line-by-line JSON |
| YAML | tree-sitter-yaml |
| TOML | tree-sitter-toml-ng |
| `.env`, INI | Custom flat-format parser |

These produce a `StructuredDocumentIndex` that shares the `RealmIndex`
infrastructure but exposes key paths (e.g., `server.port`) instead of
markdown-specific symbols.

## The ScanBackend trait

`markymark-core/src/scanner.rs` defines `ScanBackend` — an abstraction over
symbol scanning with two implementations:

- **`Md4cScanBackend`** — md4c FFI via `markymark-kernels`
- **`ZigScanBackend`** — Zig SIMD scan backend

Both return the same result types, keeping the indexing layer parser-agnostic.
