---
id: marky-luy
title: '[EPIC] g9t remediation: arena conformance closeout'
status: open
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
---













Purpose: focused remediation umbrella for unresolved marky-g9t conformance gaps.

Definition of done (per user): match existing arena decisions.

Scope gates:
1) marky-bjj is a hard blocker milestone (types.rs split below 1000 LOC).
2) g9t.1-g9t.7 close only when each reopened criterion is evidenced in code/tests/bench output.
3) Architecture target follows recorded decisions:
   - parser/index arena model per dec-arena-001/002
   - hashbrown+bump allocator strategy per dec-arena-003
4) Benchmark evidence must include measurable memory/perf deltas for required metrics, not just LSP latency.

Relationship to g9t:
- marky-g9t remains parent feature epic/state anchor.
- marky-luy tracks conformance-closeout execution so we can finish without rewriting whole roadmap.

Execution phases:
Phase A: blocker (marky-bjj)
Phase B: parser/core conformance (g9t.1-3)
Phase C: index/realm conformance (g9t.4-5)
Phase D: transport adaptation (g9t.6)
Phase E: benchmark + cleanup + closeout (g9t.7)
