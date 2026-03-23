---
id: marky-lkj.11
title: LSP hover for structured document keys
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---



## Context
Epic success criteria: "LSP hover on structured doc keys shows value type and full path." Currently SymbolAtPosition enum has no variant for structured doc keys (only Heading, WikiLink, MarkdownLink, XmlTag). symbol_at_position() only checks markdown DocumentIndex entries. Hovering on a key inside a JSON/YAML/TOML file returns nothing.

## Requirements
- Add SymbolAtPosition::KeyEntry variant (or similar) for structured doc keys
- Extend symbol_at_position() to check structured document indexes when the URI maps to a structured doc
- Hover handler should display: key full path, value type (ValueKind), depth, and parent context
- Example hover on "database.host" in TOML: shows "Key: database.host (String, depth 1)"

## Acceptance Criteria
- Hovering on a key in a JSON file shows value type and full key path
- Hovering on a key in a YAML file shows value type and full key path
- Hovering on a key in a TOML file shows value type and full key path
- Tests for each format
- Existing tests pass (zero regression)
- cargo clippy clean
