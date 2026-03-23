---
id: marky-4cu
title: 'Fix Zig doc/behavior mismatches flagged in PR #18 review'
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
---

Copilot review found several doc comments that don't match implementation behavior in Zig kernel code. Batch fix all in one pass:

1. exports_embed.zig:69 — zig_embedding_index_search docs say returns count, but impl returns 0 and uses written out-parameter
2. shared/entities.zig:22 — docs say -1 for zero length, but impl returns 0 (no-op success)  
3. reference/entities_ref.zig:38 — same doc mismatch as #2
4. reference/similarity_ref.zig:14 — cosine docs mention -2.0 on null but impl doesn't check (validated at C ABI boundary)
5. reference/similarity_ref.zig:48 — jaccard docs mention -1.0 on null but impl doesn't check (validated at C ABI boundary)

For #4 and #5, update docs to state null checks happen at the C ABI wrapper level, not in the reference implementations.
