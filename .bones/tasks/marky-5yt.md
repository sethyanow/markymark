---
id: marky-5yt
title: Replace 'static lifetime with self_cell/ouroboros for Ast and DocumentIndex
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---



The self-referential arena pattern in Ast and DocumentIndex uses 'static lifetime markers that can technically leak through inner &'static str fields. While all public accessors return references tied to &self (preventing escape in typical usage), the inner types allow extracting arena-borrowed strings past struct lifetime. Replace with self_cell or ouroboros to enforce the lifetime constraint statically. Low priority — current design is safe in practice since all callers go through &self.
