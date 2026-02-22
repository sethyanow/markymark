---
title: Supported Formats
description: All file formats and markdown flavors supported by markymark
---

markymark indexes more than plain markdown. Structured configuration files like
JSON, YAML, and TOML get document outlines, hover information, and symbol search —
the same core intelligence applied to a different surface.

## Markdown

**Extensions:** `.md`, `.markdown`

Full feature set: diagnostics, go-to-definition, find-references, hover, rename,
completion, document symbols, and workspace symbol search. Wiki-link syntax
(`[[page]]`, `[[page#heading]]`) is supported natively, including Obsidian and
Logseq conventions.

Markdown files are the primary format — every LSP and MCP capability is available.

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
- **Workspace symbols** — keys are searchable alongside markdown headings

### What doesn't

- **Find references** — not available for structured keys across files
- **Rename** — not available for structured keys
- **Diagnostics** — markdown-specific checks (broken links, duplicate headings) do
  not apply to structured formats
- **Completion** — not available for structured formats
