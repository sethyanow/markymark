---
id: marky-lkj.12
title: Bidirectional find-references for structured document keys
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-lkj.11]
parent: marky-lkj
---



## Context
Epic success criteria #5: "find-references works across markdown <-> structured documents (wiki-link to JSON key)." The LSP references handler only handles Heading and XmlTag variants — falls through to _ => return Ok(None) for all others. No support for:
- From structured doc key → find all markdown wiki-links referencing it
- From markdown wiki-link to structured key → find the key definition

Cross-doc resolution (marky-lkj.8) already works for go-to-definition via ResolvedTarget::KeyPath. This task extends the reverse direction.

## Requirements
- Add SymbolAtPosition::KeyEntry variant (may share with hover task)
- In references handler, when cursor is on a structured doc key:
  - Search all markdown documents for wiki-links whose target resolves to this key path
  - Return locations of those wiki-links
- In references handler, when cursor is on a wiki-link that resolves to a KeyPath:
  - Return the key definition location in the structured doc
- MCP find-references tool should also support structured doc positions

## Acceptance Criteria
- Cursor on JSON key → returns markdown wiki-links referencing [[file#key.path]]
- Cursor on wiki-link [[config.json#database.host]] → returns key location in config.json
- Tests for JSON, YAML, TOML cross-references
- Existing tests pass (zero regression)
- cargo clippy clean
