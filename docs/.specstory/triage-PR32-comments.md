# PR 32 — Comment Triage

**PR:** [feat: add index serde kernels and harden ffi safety](https://github.com/sethyanow/markymark/pull/32)  
**Triage date:** 2026-02-17

## Summary

| Category        | Count |
|----------------|-------|
| **Accept (fix)** | 5   |
| **Duplicate**    | 2   |
| **Reject**       | 1   |

All line-level comments target `zig/src/kernels/index_serde.zig`. Two reviewers (Greptile, Copilot) raised overflow-safety and doc-comment accuracy.

---

## Accepted — Fix in code

### 1. `getHeading`: overflow-safe offset (line 149)

- **Comment (Greptile + Copilot):** `i * @sizeOf(IndexHeading)` can overflow for large `i`.
- **Triage:** Valid. `init()` already uses checked math; accessors should too.
- **Fix:** Use `std.math.mul(usize, @as(usize, i), @sizeOf(IndexHeading)) catch return null` and add to base offset with checked add.

### 2. `getString`: overflow-safe bounds check (line 157)

- **Comment (Greptile + Copilot):** `offset + length` can overflow; comparison can be bypassed.
- **Triage:** Valid. Use checked add before comparing to `string_table_size`.
- **Fix:** `const end = std.math.add(u32, offset, @as(u32, length)) catch return null; if (end > self.header.string_table_size) return null;`

### 3. Struct size comments (lines 38, 58, 65)

- **Comment (Copilot):** IndexHeading, IndexTag, and IndexBlockId doc comments say "8 bytes" but `extern struct` layout is 12 bytes (e.g. 4+4+2+2 or 4+4+2+1+1).
- **Triage:** Valid. Comments are wrong; runtime uses `@sizeOf(...)` so behavior is correct; only docs need updating.
- **Fix:** Change "8 bytes" → "12 bytes" for IndexHeading, IndexTag, IndexBlockId.

---

## Duplicates (no extra action)

- **Copilot (line 157):** Same as #2 — `offset + length` overflow; same fix.
- **Copilot (line 149):** Same as #1 — getHeading offset overflow; same fix.

---

## Rejected

### Header.SIZE = 32 vs @sizeOf(Header) = 36 (line 35)

- **Comment (Copilot):** Header SIZE is 32 but field sum is 36; suggests `pub const SIZE: usize = @sizeOf(Header);`.
- **Triage:** Reject. The **serialized format** is intentionally 32 bytes. Only 32 bytes are written in `serialize_index` (`header_bytes[0..Header.SIZE]`). The in-memory `Header` has `_reserved: [4]u8` which is not part of the on-disk layout. Changing to `@sizeOf(Header)` would break the format and all readers. Keep `SIZE = 32` as the format constant.

---

## References

- Greptile review: 2 comments (lines 149, 157).
- Copilot review: 6 comments (lines 35, 40, 60, 67, 149, 157); 2 duplicates of Greptile, 1 rejected (Header SIZE), 3 doc-comment fixes.
