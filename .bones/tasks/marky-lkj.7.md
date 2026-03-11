---
id: marky-lkj.7
title: 'MCP runtime: index structured documents alongside markdown'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


Update RuntimeEngine to discover and index structured files (.json, .yaml, .yml, .toml, .env, .ini, .cfg, .jsonc, .jsonl). Expand is_markdown_path to is_indexable_path using DocumentKind::from_path. Parse structured files with parse_structured, wrap in StructuredDocumentIndex, add to realm via add_structured_document. Update get-outline, search-symbols, export-index, and realm-stats to include structured doc data.
