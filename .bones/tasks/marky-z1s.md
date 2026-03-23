---
id: marky-z1s
title: 'fix(semantic): account for stale vectors when computing fetch_k'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

SemanticIndex::remove_document only removes metadata, not vectors from the Zig index. search() computes fetch_k = top_k * 4 but stale vectors can dominate raw hits, causing too few active results. Fix: pass active entry count to compute_fetch_k and scale by stale ratio.
