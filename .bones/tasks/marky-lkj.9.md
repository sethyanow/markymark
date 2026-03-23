---
id: marky-lkj.9
title: 'LSP: DocumentSymbols and hover for structured documents'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


Update LSP server to provide DocumentSymbol responses for structured docs (key hierarchy as symbols). Add hover handler showing value type and full key path. Requires ServerState to handle non-markdown URIs via get_any_document.
