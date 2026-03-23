---
id: marky-raq2
title: 'Defense-in-depth: computeSectionOffsets overflow check in blob.zig'
status: closed
type: task
priority: 4
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


Code review nitpick: computeSectionOffsets relies on caller precondition (validated header). Adding runtime overflow check would be defense-in-depth.

## Design

## Goal
Change computeSectionOffsets in blob.zig to return ?SectionOffsets (or error) instead of relying on a debug-only assertion, so release builds get a defined failure path instead of wrapping u32 arithmetic.

## Why (Gap Analysis)
Currently computeSectionOffsets (blob.zig:135) has a precondition: header must come from a validated blob. The debug-mode guard calls computeBlobSize and asserts non-null. In ReleaseFast/ReleaseSmall builds, this assertion is stripped, and the u32 multiplications (e.g., header.heading_count * @sizeOf(BlobHeading)) could theoretically wrap. All current callers enforce the precondition via computeBlobSize, so this is defense-in-depth, not a bug fix.

## Effort Estimate
1-2 hours

## Success Criteria
- [ ] computeSectionOffsets signature changed to return ?SectionOffsets
- [ ] Function calls computeBlobSize at the top and returns null if it returns null
- [ ] All callers updated to handle the ?SectionOffsets return (orelse return error or orelse unreachable for validated paths)
- [ ] Existing tests in blob.zig pass unchanged
- [ ] Zig test suite passes: zig build test
- [ ] Rust-side from_blob.rs compute_offsets remains unchanged (it operates on already-validated headers)
- [ ] No new test needed (existing computeSectionOffsets tests cover correctness; overflow path already tested by computeBlobSize overflow test)

## Implementation Checklist
- [ ] In blob.zig: change fn signature from computeSectionOffsets(header: ScanBlobHeader) SectionOffsets to pub fn computeSectionOffsets(header: ScanBlobHeader) ?SectionOffsets
- [ ] Add computeBlobSize call at top of function body; if null, return null
- [ ] Remove the if (std.debug.runtime_safety) block (now redundant)
- [ ] Keep the u32 arithmetic unchanged (guaranteed safe after computeBlobSize check)
- [ ] In document.zig (serializeState, line 502): update call site — const offsets = blob.computeSectionOffsets(header) orelse return error.OutOfMemory;
- [ ] Search for any other callers with: grep -rn computeSectionOffsets zig/src/
- [ ] Update all callers consistently
- [ ] Run zig build test to verify all tests pass

## Key Considerations (SRE Review)

**Edge Case: Test Code Callers**
Test code in blob.zig calls computeSectionOffsets directly (line 300). Update these to use orelse unreachable (or .? shorthand) since test headers are hardcoded and valid.

**Edge Case: Performance Impact**
computeBlobSize does 6 multiplications and additions in u64 space plus one comparison. This runs once per blob deserialization — negligible compared to text pool parsing. No performance concern.

**Safety: No Behavior Change for Valid Inputs**
For all valid inputs (computeBlobSize returns non-null), the function returns the same SectionOffsets as before. The only new behavior is returning null for invalid inputs that previously caused UB in release mode.

**Reference Implementation**
- computeBlobSize (blob.zig:100-118) already returns ?u32 — follow the same pattern
- validateBlob (blob.zig:175-198) shows how callers handle optional returns

## Anti-patterns
- Do NOT use unreachable in production callers — use orelse return error
- Do NOT remove the u32 arithmetic (it's correct after the computeBlobSize check)
- Do NOT change the SectionOffsets struct itself
- Do NOT add a separate error type — ?SectionOffsets (optional) is sufficient
