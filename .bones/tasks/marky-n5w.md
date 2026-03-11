---
id: marky-n5w
title: 'P2: Eliminate eager alloc of candidate names before scoring in SearchSymbols'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

runtime_engine.rs:292 — candidates is Vec<(String, DocumentUri, Range)> which clones every heading name before scoring. Fix: store Vec<(&str, DocumentUri, Range)> borrowing from indexes, derive candidate_refs from that, then only clone strings for final ranked results. Requires lifetime annotations — may need a let binding to extend index lifetime.
