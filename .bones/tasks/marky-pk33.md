---
id: marky-pk33
title: 'PR#41 follow-up: FFI safety hardening — exports.zig bounds check + blob.zig fallibility'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

## Design

## Goal

Harden FFI boundary with two targeted safety improvements on the Zig side.

## Items

### C1: exports.zig u32 intCast guard (defense-in-depth)

File: zig/src/engine/exports.zig:77
Code: out_len.* = @intCast(data.len);
Issue: If blob exceeds u32 range, @intCast traps in debug/safety builds. FFI functions
must not panic — return error codes.
Fix: Add bounds check: if (data.len > std.math.maxInt(u32)) return @as(i32, -5);

Practical reachability: Very low. Blob size = 64B header + N*struct_size + text_pool.
At 100K headings * 60B each = ~6MB. Would need ~71M headings to exceed 4GB. Not
reachable in practice, but the contract should hold.

### C2: blob.zig writeStruct/readStruct fallibility

File: zig/src/engine/blob.zig:213,220
Issue: pub functions use std.debug.assert for bounds. In ReleaseFast, asserts are stripped.
Used cross-module (document.zig:538,561,578,595 and document_test.zig:157), so must stay pub.
Fix: Change to fallible (return error union). Callers in document.zig serializeState
already handle errors — thread through the new error.

### DISMISSED: Semgrep nosemgrep alignment

engine.rs and md4c.rs already have complete SAFETY + nosemgrep coverage (verified by
grep). GHAS still flags them because GitHub code scanning may not honor inline nosemgrep
comments in the diff view. This is a platform limitation, not a code issue. No action
needed unless we want to add a .semgrep.yml rule override at project level.

### DISMISSED: md4c.rs u32 truncation

Previously analyzed and dismissed in PR#40 review (see MEMORY.md "Dismissed Findings").
Theoretical only — 4GB markdown files don't exist. Not UB on truncation.

## Effort Estimate

2-3 hours

## Success Criteria

- [ ] marky_engine_get_blob returns -5 for blobs exceeding u32::MAX bytes (test with mocked data)
- [ ] writeStruct returns error on out-of-bounds offset
- [ ] readStruct returns error on out-of-bounds offset
- [ ] All callers of writeStruct/readStruct handle the error
- [ ] zig build test passes for engine + blob tests
- [ ] Existing golden blob roundtrip test still passes
- [ ] cargo test -p markymark-kernels passes (Rust side unaffected)

## Implementation Checklist

- [ ] exports.zig:75-78: Add bounds check before @intCast
- [ ] blob.zig:213: Change writeStruct signature to return !void
- [ ] blob.zig:220: Change readStruct signature to return !T (or error union)
- [ ] Replace std.debug.assert with runtime bounds check + error return
- [ ] Update all callers in document.zig serializeState to propagate errors
- [ ] Update blob test at line 305 to handle new error return
- [ ] Add test for writeStruct with out-of-bounds offset
- [ ] Add test for readStruct with out-of-bounds offset

## Anti-patterns

- Do NOT make writeStruct/readStruct private (used in document.zig)
- Do NOT use @panic for bounds violations (FFI must not panic)
- Do NOT change the blob format or header structure
