---
id: marky-0mr.4
title: 'PR#39 review: fix memory safety in blocks.zig'
status: closed
type: bug
priority: 0
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---



Fix three memory safety issues in zig/src/md4c/blocks.zig:

**T1-2: Memory leak on allocation failure during ref def duplication (line ~817)**
If dest_dupe or title_dupe allocator.dupe() fails, label_dupe (and dest_dupe if title fails) are leaked — no errdefer cleanup. Fix: add errdefer self.allocator.free(label_dupe) after label alloc, errdefer self.allocator.free(dest_dupe) after dest alloc, before title alloc.

**T2-6: Silent catch {} on buffer appends in consumeRefDefsFromCurrentBlock (line ~786)**
Both buffer.append('\n') and buffer.appendSlice(...) use catch {}. OOM causes incomplete or missing ref def parsing with no indication. Fix: change to catch return to stop processing on failure.

**T3-14: Fragile BlockHeader pointer arithmetic (line ~186-196)**
BlockHeader access uses hardcoded len - size instead of computed top_off, with a possibly-redundant runtime check. Fix: use computed top_off when forming the pointer and add std.debug.assert for bounds validation.

Source: PR #39 review — CodeRabbit
