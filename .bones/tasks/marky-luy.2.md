---
id: marky-luy.2
title: Investigate arena reuse via DocumentArena::reset for re-parse
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-luy
---


Currently re-parsing a document drops the old DocumentIndex (and its arena) and creates a new one. For frequently edited documents in active LSP sessions, arena reuse via reset() could avoid repeated allocation/deallocation. Investigate whether a DocumentIndex::reindex(self, ast) -> Self method would provide measurable benefits. Related: M-HOTPATH, M-THROUGHPUT guidelines.
