---
id: marky-i0b9
title: 'Refactor: extract server.rs helper functions to reduce file below 1000 lines'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

server.rs is at 1031 lines (above 1000-line hard stop). Extract free helper functions (lines 885-983) to a helpers module: resolved_target_to_location, iter_realm_documents, XmlHoverStats, xml_hover_stats, structured_key_hover_markdown. This brings server.rs to ~930 lines.
