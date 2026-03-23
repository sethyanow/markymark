---
id: marky-e5pa
title: Enforce file:// URI validation in get-diagnostics tool handler
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

get-diagnostics accepts any URI scheme via DocumentUri::new() instead of using parse_file_uri() like other file-scoped tools (get-outline, find-references, export-index). A request with https://... falls through to core lookup and returns a generic error instead of the consistent non_file_uri contract. Fix: use parse_file_uri() at diagnostics.rs:20. Source: Codex review.
