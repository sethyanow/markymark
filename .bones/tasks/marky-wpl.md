---
id: marky-wpl
title: 'fix(fence_map): buffer overflow - write before capacity check'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
---

fence_map.zig lines 129-142: out[count.*] written before count.* >= cap check. Swap order: check capacity, compute line_end, THEN write.
