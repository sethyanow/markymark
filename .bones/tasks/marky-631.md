---
id: marky-631
title: 'fix(index_serde): type mismatch usize/u32/u16 in offset arithmetic'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
---

index_serde.zig lines 17-20: padAfterStringTable, getHeading, getString mix usize/u32/u16 without casts - compile failure on Zig 0.15.
