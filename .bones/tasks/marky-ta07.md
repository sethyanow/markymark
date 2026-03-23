---
id: marky-ta07
title: convert_result missing blob bounds validation before slicing
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

md4c.rs convert_result slices blob[text_start..text_end] using FFI offsets without bounds checks. If Zig returns inconsistent offsets, Rust panics on OOB slice and crashes LSP/MCP. The from_blob path validates via pool_str; this FFI extract path does not. Source: codex review.

## Design

## Goal

Add bounds validation to convert_result in md4c.rs before slicing blob with FFI-provided offsets.

## Root Cause

md4c.rs:151-156 (headings) and 173-183 (links) compute text_start/text_end and target_start/target_end from FFI struct fields, then slice blob directly:
\`\`\`rust
let text_start = h.text_offset as usize;
let text_end = text_start + h.text_length as usize;
let text = std::str::from_utf8(&blob[text_start..text_end]) // panics on OOB
\`\`\`

If Zig returns inconsistent offsets (parser bug, memory corruption), Rust panics on out-of-bounds slice and crashes the LSP/MCP process. The from_blob pathway validates via pool_str (checks off + len > pool.len()), but this FFI extract path has no validation.

## Effort Estimate

1-2 hours (add bounds checks + negative test)

## Implementation Checklist

- [ ] Create a helper function \`safe_blob_slice(blob: &[u8], start: usize, len: usize) -> Result<&[u8], KernelError>\` that:
  - Checks \`start.checked_add(len)\` for overflow
  - Checks \`start + len <= blob.len()\` for bounds
  - Returns \`Err(KernelError::InternalError(-101))\` on failure (new error code, distinct from -100 UTF-8 error)
  - Returns \`Ok(&blob[start..start+len])\` on success
- [ ] Replace heading blob slice at line 156:
  \`\`\`rust
  let text_bytes = safe_blob_slice(blob, text_start, h.text_length as usize)?;
  let text = std::str::from_utf8(text_bytes)
      .map_err(|_| KernelError::InternalError(-100))?
      .to_owned();
  \`\`\`
- [ ] Replace link text blob slice at line 179:
  \`\`\`rust
  let text_bytes = safe_blob_slice(blob, text_start, l.text_length as usize)?;
  let text = std::str::from_utf8(text_bytes)
      .map_err(|_| KernelError::InternalError(-100))?
      .to_owned();
  \`\`\`
- [ ] Replace link target blob slice at line 182:
  \`\`\`rust
  let target_bytes = safe_blob_slice(blob, target_start, l.target_length as usize)?;
  let target = std::str::from_utf8(target_bytes)
      .map_err(|_| KernelError::InternalError(-100))?
      .to_owned();
  \`\`\`
- [ ] Add test: \`test_oob_heading_offset_returns_error\` — forge a CMd4cResult with text_offset past blob end, verify InternalError(-101)
- [ ] Add test: \`test_oob_link_offset_returns_error\` — forge a CMd4cResult with target_offset past blob end, verify InternalError(-101)
- [ ] Add test: \`test_overflow_offset_returns_error\` — forge offset + length that overflows usize, verify InternalError(-101)
- [ ] Run \`cargo nextest -p markymark-kernels\` — all tests pass
- [ ] Run \`cargo clippy --workspace --all-targets\` — clean

## Success Criteria

- [ ] safe_blob_slice helper validates both overflow and bounds
- [ ] All 4 blob slice sites in convert_result use safe_blob_slice
- [ ] OOB offset test returns KernelError, not panic
- [ ] Overflow test returns KernelError, not panic
- [ ] Existing extract_md4c tests still pass (valid offsets unaffected)
- [ ] \`cargo nextest -p markymark-kernels\` clean
- [ ] \`cargo clippy --workspace --all-targets\` clean

## Key Considerations (SRE Review)

**Edge Case: text_offset = 0, text_length = 0 (empty text)**
Valid edge case for headings with empty display text. safe_blob_slice must accept start=0, len=0 and return empty slice. This is handled by: 0 + 0 = 0 <= blob.len() for any non-empty blob, and blob[0..0] is valid.

**Edge Case: text_blob_len = 0 (empty blob)**
Already handled by existing guard at line 134: if out.text_blob_len == 0, blob = &[]. Any nonzero offset in a heading/link struct would then fail bounds check correctly.

**Edge Case: usize overflow**
On 32-bit platforms (unlikely for LSP, but possible): text_offset(u32) + text_length(u32) can exceed u32::MAX. Using checked_add prevents this.

**Error code convention**
Existing code uses KernelError::InternalError(-100) for UTF-8 errors. Use -101 for bounds errors to distinguish in logs. Document this in a comment near the helper.

**Defense-in-depth, not fixing a known bug**
The Zig side always produces valid offsets for correctly-formed markdown. This validation is a safety net for:
- Future parser bugs that produce inconsistent blob layouts
- Memory corruption in the Zig allocator
- Fuzz testing uncovering edge cases

## Anti-patterns
- Do NOT add bounds checks inline (duplicated 4 times) — extract helper
- Do NOT use panic! or unwrap — return KernelError for graceful degradation
- Do NOT change the blob format or Zig side — fix is purely in Rust consumer
