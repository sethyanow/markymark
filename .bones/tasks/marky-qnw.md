---
id: marky-qnw
title: Fix entity hashes zero-length and capacity==0 edge cases in c_adapter.zig
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
---

PR #18 Copilot review (c_adapter.zig:293): zig_extract_entity_hashes has inconsistent zero-length handling:
- text_len==0 should be a no-op regardless of text_ptr/output_ids/capacity, but currently requires output_ids non-null
- capacity==0 returns -2 without setting written=0, breaking the 'writes as many as fit' contract

Fix: return early for text_len==0 before validating output params; set written=0 on capacity==0 path.
