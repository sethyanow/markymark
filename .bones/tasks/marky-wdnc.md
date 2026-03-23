---
id: marky-wdnc
title: 'Zig engine doc and guard improvements (PR #41 nitpick bundle)'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
---

PR #41 round 2/3 CodeRabbit nitpicks consolidated into one track.

**H1: Document -5 error code (exports.zig:57-61)**
Doc comment lists return codes 0, -1, -3, -4 but NOT -5 (blob size overflow, 
line 78). Add one line: "///  -5  — blob size overflow (exceeds u32 max)"

**H2: @intCast overflow guards in serializeState (document.zig:491-497)**
5 @intCast casts on engine.headings.len, .links.len, .tags.len, 
.block_ids.len, .line_starts.len without prior bounds check. Traps in safe 
builds if >u32::MAX elements. Physically impossible (400 GB+ RAM required).
Pure defense-in-depth. Add guard block or accept risk.

**H3: readHeader/writeHeader precondition docs (blob.zig:199-210)**
No doc comment noting input slice must be >= @sizeOf(ScanBlobHeader) bytes.
Add brief precondition docs. Functions rely on Zig slice bounds checking 
(panic in safe builds on undersized input).

**H4: Named constant for 256 fence limit (document.zig:228-234)**
Magic number 256 for fence_buf stack allocation. Extract to 
FENCE_MAP_MAX = 256 constant with doc comment explaining the limit.

NOTE: writeStruct/readStruct was already fixed by marky-pk33 (now fallible
with error.OutOfRange). That finding is resolved.
