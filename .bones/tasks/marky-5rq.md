---
id: marky-5rq
title: 'fix(index_serde): @alignCast panic on misaligned disk input'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
---

index_serde.zig lines 97-123: @alignCast without alignment guard panics in Debug/ReleaseSafe on misaligned input. Add @intFromPtr check before cast.
