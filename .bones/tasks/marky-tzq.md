---
id: marky-tzq
title: Switch LSP to TextDocumentSyncKind::INCREMENTAL
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9b]
---



## What
Change the LSP server from requesting full document text on every change to incremental content changes. This gives us edit ranges (start position, range length, new text) instead of the full document.

## Acceptance Criteria
- [ ] TextDocumentSyncKind::INCREMENTAL in initialize response
- [ ] did_change handler processes TextDocumentContentChangeEvent with range field
- [ ] ServerState applies text edits to stored document text correctly
- [ ] UTF-16 offset conversion handled (LSP uses UTF-16, Rust uses UTF-8 byte offsets)
- [ ] Multiple content changes in a single didChange are applied in order
- [ ] Fallback: if change has no range (full replacement), handle gracefully
- [ ] Test: apply incremental edits and verify stored text matches expected

## Risk
HIGH — UTF-16 ↔ UTF-8 byte offset conversion is the #1 source of bugs in LSP implementations. Must handle multi-byte characters (emoji, CJK) correctly. tower-lsp's PositionEncoding may help.

## Files
- markymark-lsp/src/server.rs (sync kind, did_change)
- markymark-lsp/src/state.rs (apply_text_edit method)
- markymark-lsp/src/convert.rs (position conversion utilities)
