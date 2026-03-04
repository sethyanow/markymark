---
title: Supported Formats
description: All file formats and Markdown flavors supported by markymark
---

markymark indexes more than plain Markdown. Structured configuration files like
JSON, YAML, and TOML get document outlines, hover information, and symbol search —
the same core intelligence applied to a different surface.

## Markdown

**Extensions:** `.md`, `.markdown`

Full feature set: diagnostics, go-to-definition, find-references, hover, rename,
completion, document symbols, and workspace symbol search. Wiki-link syntax
(`[[page]]`, `[[page#heading]]`) is supported natively, including Obsidian and
Logseq conventions.

Markdown files are the primary format — every LSP and MCP capability is available.

### Extracted elements

markymark extracts these elements from Markdown during indexing:

| Element | Syntax | Notes |
|---------|--------|-------|
| Headings | `## Title` | Level, slug, and position tracked |
| Wiki links | `[[page]]`, `[[page\|alias]]` | Obsidian/Logseq syntax, optional heading anchor |
| Markdown links | `[text](url#anchor)` | Optional heading anchor for cross-references |
| Tags | `#tag` | Logseq-style inline tags |
| XML tags | `<agent>...</agent>` | Custom semantic elements with attributes |
| Block IDs | `^block-id` | Obsidian document-internal anchors |
| Block references | `((uuid))` | Logseq cross-document block refs |
| Code spans | `` `symbol` `` | Inline code cross-references with optional symbol kind |
| Tasks | `- [ ]` / `- [x]` | Checkbox state and description tracked |
| Embeds | `![[target]]` | Obsidian file transclusion |
| Callouts | `> [!note] Title` | Admonition type and optional title |
| Query blocks | `{{query ...}}` | Logseq datalog queries |
| Link definitions | `[label]: url "title"` | Reference-style link targets |
| Frontmatter | YAML / TOML block | Document metadata key-value pairs |
| Properties | `key:: value` | Logseq inline properties |

## Structured formats

Structured documents share a common set of features: document outline (symbol
hierarchy), hover (value kind and key path), and workspace symbol search. They
do **not** support find-references or rename.

| Format | Extensions | Comments | Notes |
|--------|-----------|----------|-------|
| JSON | `.json` | No | Standard JSON |
| JSONC | `.jsonc` | `//` and `/* */` | Trailing commas allowed |
| JSON5 | `.json5` | `//` and `/* */` | Unquoted keys, single-quoted strings |
| JSONL | `.jsonl` | Per-line | Each line indexed as `[index].key` |
| YAML | `.yaml`, `.yml` | `#` | Anchors and aliases supported |
| TOML | `.toml` | `#` | Dotted keys expanded in outline |
| Dotenv | `.env` | `#`, `;` | Flat key=value pairs |
| INI | `.ini`, `.cfg` | `#`, `;` | Sections, both `=` and `:` separators |

### What works

- **Document symbols** — hierarchical key outline in your editor's symbol panel
- **Hover** — shows the value kind (string, number, array, etc.) and full key path
- **Workspace symbols** — keys are searchable alongside Markdown headings

### What doesn't

- **Find references** — not available for structured keys across files
- **Rename** — not available for structured keys
- **Diagnostics** — Markdown-specific checks (broken links, duplicate headings) do
  not apply to structured formats
- **Completion** — not available for structured formats
