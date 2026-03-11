---
id: marky-pgxq
title: get-diagnostics misreports indexed structured docs as missing
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

handle_get_diagnostics_file uses get_document() which only returns markdown indexes. Indexed structured files (JSON/YAML/TOML) incorrectly hit the None branch and return 'document not indexed'. Should use get_any_document() and return empty diagnostics for non-markdown formats (diagnostics only apply to markdown). Source: Codex review. File: markymark-mcp/src/engine/diagnostics.rs:40-44
