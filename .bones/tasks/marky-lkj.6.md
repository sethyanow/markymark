---
id: marky-lkj.6
title: 'Index layer: StructuredDocumentIndex + AnyDocumentIndex + RealmIndex integration'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


Create StructuredDocumentIndex that wraps StructuredAst, producing outline and symbol data from key entries. Create AnyDocumentIndex enum (Markdown | Structured) to replace DocumentIndex in RealmIndex. Update RealmIndex to accept AnyDocumentIndex. Update file discovery to index structured documents alongside markdown. Must maintain all existing markdown functionality (zero regression).
