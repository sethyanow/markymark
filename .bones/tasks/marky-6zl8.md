---
id: marky-6zl8
title: 'FFI bridge: C ABI exports for md4c ExtractionRenderer and Rust bindings'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-v03o]
parent: marky-0mr
---




## Goal
Create C ABI export functions wrapping the ExtractionRenderer and corresponding safe Rust FFI bindings in markymark-kernels, enabling Rust code to call the single-pass md4c extraction pipeline.

## Context
- ExtractionRenderer (zig/src/md4c/extraction_renderer.zig) produces ExtractionResult with owned strings and byte offsets — 25 tests passing, zero leaks
- Existing FFI pattern (c_adapter.zig → scan.rs): caller-allocated buffers, fixed-size C structs, call_scan_ffi retry
- md4c extraction differs: unknown result count, variable-length strings → Zig must allocate, Rust reads and frees
- Single function returns both headings and links (single parse pass, no double-parse waste)
- build.zig currently has md4c as test-only; must add to libmarky_kernels.a static library

## Design: Zig-Allocates FFI Pattern

### C ABI Types (Zig side)
```zig
// All text packed into a single contiguous blob
// Struct offsets point into this blob, not into source text
const CMd4cHeading = extern struct {
    source_offset: u32,  // byte offset of '#' (ATX) or text start (setext) in source
    text_offset: u32,    // offset into text_blob for heading text
    text_length: u16,    // length in text_blob
    level: u8,
    _padding: u8,
};

const CMd4cLink = extern struct {
    source_offset: u32,  // byte offset of '[' or '[[' in source
    text_offset: u32,    // offset into text_blob for display text
    text_length: u16,    // length in text_blob
    target_offset: u32,  // offset into text_blob for href/target
    target_length: u16,  // length in text_blob
    is_wiki: u8,         // 1 for [[wiki]] links, 0 otherwise
    _padding: u8,
};

const CMd4cResult = extern struct {
    headings: ?[*]CMd4cHeading,
    headings_count: u32,
    links: ?[*]CMd4cLink,
    links_count: u32,
    text_blob: ?[*]const u8,  // concatenated heading texts + link texts + targets
    text_blob_len: u32,
};
```

### C ABI Functions
```zig
export fn marky_md4c_extract(text: ?[*]const u8, len: u32, out: ?*CMd4cResult) i32;
// Returns: 0=success, -1=null pointer, -3=parse error
// Allocates headings/links/text_blob arrays via page_allocator (or c_allocator)

export fn marky_md4c_free(result: ?*CMd4cResult) void;
// Frees all three allocations (headings, links, text_blob)
```

### Rust FFI Bindings (markymark-kernels)
```rust
#[repr(C)]
struct CMd4cHeading { source_offset: u32, text_offset: u32, text_length: u16, level: u8, _padding: u8 }

#[repr(C)]  
struct CMd4cLink { source_offset: u32, text_offset: u32, text_length: u16, target_offset: u32, target_length: u16, is_wiki: u8, _padding: u8 }

#[repr(C)]
struct CMd4cResult { headings: *mut CMd4cHeading, headings_count: u32, links: *mut CMd4cLink, links_count: u32, text_blob: *const u8, text_blob_len: u32 }

extern "C" {
    fn marky_md4c_extract(text: *const u8, len: u32, out: *mut CMd4cResult) -> i32;
    fn marky_md4c_free(result: *mut CMd4cResult);
}
```

### Memory Ownership
1. Rust calls marky_md4c_extract with source text pointer
2. Zig runs ExtractionRenderer, packs results into CMd4cResult (3 allocations: headings array, links array, text blob)
3. Rust reads results, creates owned Strings from text_blob slices
4. Rust calls marky_md4c_free to release all Zig-side memory
5. Rust owns the converted data, Zig owns nothing after free

## Implementation Steps

### Step 1: Create zig/src/md4c/exports.zig
- Define CMd4cHeading, CMd4cLink, CMd4cResult extern structs
- Implement marky_md4c_extract: validate pointers → run extractFromMarkdown → pack results into CMd4cResult
- Implement marky_md4c_free: free headings/links/text_blob allocations
- Use std.heap.page_allocator for FFI allocations (same as existing c_adapter pattern)

### Step 2: Wire md4c into build.zig
- Add md4c source files to libmarky_kernels static library compilation
- Import exports.zig from c_adapter.zig (or add to exports_embed.zig pattern)
- Verify: zig build lib succeeds, zig build test still passes all md4c + kernel tests

### Step 3: Create markymark-kernels/src/md4c.rs
- Define #[repr(C)] mirror structs
- extern "C" block with FFI declarations
- Safe wrapper function: extract_md4c(text: &str) -> Result<Md4cExtractionResult, KernelError>
- Md4cExtractionResult with headings: Vec<Md4cHeading>, links: Vec<Md4cLink>
- Md4cHeading { text: String, offset: u32, level: u8 }
- Md4cLink { text: String, target: String, offset: u32, is_wiki: bool }

### Step 4: Add module to markymark-kernels/src/lib.rs
- pub mod md4c;
- Verify cargo build succeeds

### Step 5: Write Zig-side tests in exports.zig
- Test marky_md4c_extract with simple heading → verify CMd4cResult fields
- Test marky_md4c_extract with links → verify text_blob contains text/target
- Test marky_md4c_extract with null pointer → returns -1
- Test marky_md4c_free doesn't crash on valid result
- Test empty input → zero headings, zero links

### Step 6: Write Rust-side integration tests in markymark-kernels
- Test extract_md4c with heading → HeadingResult correct
- Test extract_md4c with links → LinkResult correct  
- Test extract_md4c with wiki links → is_wiki=true
- Test extract_md4c with mixed document → headings and links correct
- Test extract_md4c with empty input → empty results

### Step 7: Verify full build pipeline
- cargo build (links libmarky_kernels.a with md4c)
- cargo nextest (all existing + new tests pass)
- zig build test (all md4c + kernel tests pass)

## Success Criteria
- [ ] CMd4cHeading/CMd4cLink/CMd4cResult structs defined and aligned correctly
- [ ] marky_md4c_extract returns correct results for headings, links, wiki links
- [ ] marky_md4c_free releases all Zig-allocated memory
- [ ] Rust safe wrapper converts C results to owned Rust types correctly
- [ ] Null pointer input returns -1 (no crash)
- [ ] Empty input returns zero results (no crash)
- [ ] 5+ Zig-side FFI tests pass
- [ ] 5+ Rust-side integration tests pass
- [ ] cargo build succeeds (linker finds md4c symbols)
- [ ] All existing tests still pass (cargo nextest + zig build test)

## Anti-Patterns
- Do NOT modify extraction_renderer.zig — this task wraps it, doesn't change it
- Do NOT use the existing call_scan_ffi retry pattern — md4c extraction uses Zig-allocates pattern
- Do NOT store raw pointers into Rust-owned data across FFI — copy text immediately then free
- Do NOT use @panic or unreachable in FFI functions — return error codes
- Do NOT assume text_blob lifetime extends past marky_md4c_free — Rust must copy before calling free
- Do NOT pass std.testing.allocator across FFI — use page_allocator or c_allocator

## Design

## Goal
Create C ABI export functions wrapping the ExtractionRenderer and corresponding safe Rust FFI bindings in markymark-kernels, enabling Rust code to call the single-pass md4c extraction pipeline.

## Context
- ExtractionRenderer (zig/src/md4c/extraction_renderer.zig) produces ExtractionResult with owned strings and byte offsets — 25 tests passing, zero leaks
- Existing FFI pattern (c_adapter.zig → scan.rs): caller-allocated buffers, fixed-size C structs, call_scan_ffi retry
- md4c extraction differs: unknown result count, variable-length strings → Zig must allocate, Rust reads and frees
- Single function returns both headings and links (single parse pass, no double-parse waste)
- build.zig currently has md4c as test-only; must add to libmarky_kernels.a static library
- Existing exports pattern: c_adapter.zig comptime block imports exports_*.zig files; each export file uses page_allocator

## Design: Zig-Allocates FFI Pattern

### C ABI Types (Zig side — zig/src/md4c/exports.zig)

**CRITICAL: Fields ordered by alignment to avoid implicit padding holes.**
All u32 fields grouped, all u16 fields grouped, all u8 fields grouped.
Both Zig extern struct and Rust #[repr(C)] MUST use identical field order.

\`\`\`zig
const CMd4cHeading = extern struct {
    source_offset: u32,  // byte offset of '#' (ATX) or text start (setext) in source
    text_offset: u32,    // offset into text_blob for decoded heading text
    text_length: u32,    // length in text_blob (u32 not u16 — headings can be long)
    level: u8,           // 1-6
    _padding: [3]u8,     // explicit padding to 16-byte struct size
};
// comptime { std.debug.assert(@sizeOf(CMd4cHeading) == 16); }

const CMd4cLink = extern struct {
    source_offset: u32,  // byte offset of '[' or '[[' in source
    text_offset: u32,    // offset into text_blob for display text
    target_offset: u32,  // offset into text_blob for href/target
    text_length: u32,    // length in text_blob (u32 not u16 — URLs can be long)
    target_length: u32,  // length in text_blob
    is_wiki: u8,         // 1 for [[wiki]] links, 0 otherwise
    _padding: [3]u8,     // explicit padding to 24-byte struct size
};
// comptime { std.debug.assert(@sizeOf(CMd4cLink) == 24); }

// Pointers grouped first, then u32 counts — avoids internal padding on 64-bit.
const CMd4cResult = extern struct {
    headings: ?[*]CMd4cHeading,   // Zig-allocated array, freed by marky_md4c_free
    links: ?[*]CMd4cLink,         // Zig-allocated array, freed by marky_md4c_free
    text_blob: ?[*]const u8,      // concatenated decoded texts, freed by marky_md4c_free
    headings_count: u32,
    links_count: u32,
    text_blob_len: u32,
    _padding: u32,                // explicit padding to 40 bytes (8-byte alignment)
};
// comptime { std.debug.assert(@sizeOf(CMd4cResult) == 40); }
\`\`\`

### C ABI Functions
\`\`\`zig
const ffi_allocator = std.heap.page_allocator;

export fn marky_md4c_extract(text: ?[*]const u8, len: u32, out: ?*CMd4cResult) i32;
// Returns: 0=success, -1=null pointer, -3=parse error, -4=out of memory
// Flow:
//   1. Validate pointers (null → return -1)
//   2. Zero out *out immediately
//   3. Run extractFromMarkdown(text[0..len], ffi_allocator) — catch → return -3/-4
//   4. Build text_blob: calculate total size, allocate, memcpy each text/target
//   5. Allocate CMd4cHeading[count] and CMd4cLink[count]
//   6. Fill C structs with source_offset, level, is_wiki, and text_blob offsets
//   7. Deinit ExtractionResult (frees owned strings — already copied to blob)
//   8. Write pointers and counts to *out, return 0

export fn marky_md4c_free(result: ?*CMd4cResult) void;
// Flow:
//   1. If result null → return
//   2. Free headings: ffi_allocator.free(headings[0..headings_count])
//   3. Free links: ffi_allocator.free(links[0..links_count])
//   4. Free text_blob: ffi_allocator.free(@constCast(text_blob[0..text_blob_len]))
//   5. Zero out *result (prevent double-free reads)
// Note: page_allocator.free() requires exact slice from alloc. Counts stored in
//       CMd4cResult enable slice reconstruction.
\`\`\`

### Rust FFI Bindings (markymark-kernels/src/md4c.rs)
\`\`\`rust
#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cHeading {
    source_offset: u32,
    text_offset: u32,
    text_length: u32,
    level: u8,
    _padding: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMd4cLink {
    source_offset: u32,
    text_offset: u32,
    target_offset: u32,
    text_length: u32,
    target_length: u32,
    is_wiki: u8,
    _padding: [u8; 3],
}

#[repr(C)]
struct CMd4cResult {
    headings: *mut CMd4cHeading,
    links: *mut CMd4cLink,
    text_blob: *const u8,
    headings_count: u32,
    links_count: u32,
    text_blob_len: u32,
    _padding: u32,
}

extern "C" {
    fn marky_md4c_extract(text: *const u8, len: u32, out: *mut CMd4cResult) -> i32;
    fn marky_md4c_free(result: *mut CMd4cResult);
}

// Compile-time size assertions (must match Zig)
const _: () = assert!(std::mem::size_of::<CMd4cHeading>() == 16);
const _: () = assert!(std::mem::size_of::<CMd4cLink>() == 24);
const _: () = assert!(std::mem::size_of::<CMd4cResult>() == 40);
\`\`\`

### Memory Ownership
1. Rust calls marky_md4c_extract with source text pointer (borrowed, valid for call duration)
2. Zig runs ExtractionRenderer → packs results into CMd4cResult (3 allocations via page_allocator: headings array, links array, text blob)
3. Zig deinits ExtractionResult (frees owned strings — data already copied to blob)
4. Rust reads CMd4cResult: builds owned Strings from text_blob slices via std::str::from_utf8
5. Rust calls marky_md4c_free → Zig frees all 3 allocations, zeroes result struct
6. After free: Rust owns converted data, Zig owns nothing
- **Double-free prevention**: marky_md4c_free zeroes the result struct; calling free twice on zeroed struct is a no-op (all pointers null, all counts 0)
- **Known leak**: normalizeLabel (marky-i3fl) leaks small page_allocator pages when documents contain reference links or wiki links. Tolerated until marky-i3fl is fixed.

## Implementation Steps

### Step 1: Create zig/src/md4c/exports.zig
- Define CMd4cHeading, CMd4cLink, CMd4cResult extern structs (field order exactly as above)
- Add comptime size assertions: @sizeOf(CMd4cHeading) == 16, @sizeOf(CMd4cLink) == 24, @sizeOf(CMd4cResult) == 40
- Define module-level const: \`const ffi_allocator = std.heap.page_allocator;\`
- Implement marky_md4c_extract:
  1. Validate: text/out null → return -1; len == 0 → zero out *out, return 0
  2. Run extractFromMarkdown(text[0..len], ffi_allocator) — catch OutOfMemory → return -4, catch other → return -3
  3. Calculate text_blob_size: sum of all heading text lengths + all link text lengths + all link target lengths
  4. Allocate text_blob = ffi_allocator.alloc(u8, text_blob_size) — catch → deinit result, return -4
  5. Copy texts into blob: memcpy each heading.text, link.text, link.target; record blob offsets
  6. Allocate headings_arr = ffi_allocator.alloc(CMd4cHeading, count) — catch → free blob, deinit result, return -4
  7. Allocate links_arr = ffi_allocator.alloc(CMd4cLink, count) — catch → free headings, free blob, deinit result, return -4
  8. Fill C structs from ExtractionResult fields
  9. Deinit ExtractionResult (frees owned strings — safe because data copied to blob)
  10. Write to *out and return 0
- Handle zero results gracefully: if 0 headings, set headings=null, headings_count=0 (no allocation needed)
- Implement marky_md4c_free:
  1. Null check on result pointer
  2. Free each non-null allocation using ffi_allocator.free(ptr[0..count])
  3. For text_blob: @constCast required since blob is ?[*]const u8
  4. Zero out *result to prevent double-free

### Step 2: Wire md4c into build.zig and c_adapter.zig
- In zig/src/c_adapter.zig, add to the comptime block at line 24-31:
  \`_ = @import("md4c/exports.zig");\`
  This forces Zig to include the md4c export fn declarations in libmarky_kernels.a.
- In zig/build.zig, the md4c module will be pulled in transitively via c_adapter.zig's comptime import.
  The existing md4c_test_mod (lines 141-150) stays for test-only compilation.
- Remove the comment "not linked into libmarky_kernels.a yet" from build.zig line 139-140.
- Verify: \`zig build lib\` succeeds (md4c symbols appear in libmarky_kernels.a)
- Verify: \`zig build test\` still passes all md4c + kernel tests

### Step 3: Create markymark-kernels/src/md4c.rs
- Add #[repr(C)] mirror structs (exact field order matching Zig)
- Add compile-time size assertions using const_assert pattern
- Add extern "C" block with FFI declarations
- Implement safe wrapper:
  \`\`\`rust
  pub fn extract_md4c(text: &str) -> Result<Md4cExtraction, KernelError> {
      // SAFETY: text.as_ptr() valid for text.len() bytes (borrowed from &str).
      // out is stack-local, valid for duration. marky_md4c_extract reads text
      // and writes to out, retaining no pointers after return.
      // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
      let mut out: CMd4cResult = unsafe { std::mem::zeroed() };
      let rc = unsafe {
          marky_md4c_extract(text.as_ptr(), text.len() as u32, &mut out)
      };
      match rc {
          0 => {
              let result = convert_result(&out, text);
              // SAFETY: out was populated by marky_md4c_extract.
              // marky_md4c_free frees the 3 Zig-allocated arrays.
              unsafe { marky_md4c_free(&mut out) };
              result
          }
          -1 => Err(KernelError::InvalidInput),
          -3 => Err(KernelError::InternalError(-3)),
          -4 => Err(KernelError::InternalError(-4)),
          other => Err(KernelError::InternalError(other)),
      }
  }
  \`\`\`
- convert_result reads C arrays and text_blob, builds owned Rust types:
  - Md4cHeading { text: String, source_offset: u32, level: u8 }
  - Md4cLink { text: String, target: String, source_offset: u32, is_wiki: bool }
  - Md4cExtraction { headings: Vec<Md4cHeading>, links: Vec<Md4cLink> }
- text_blob → String conversion: use std::str::from_utf8(blob_slice).unwrap_or("").to_owned() — blob is UTF-8 (md4c outputs decoded UTF-8 text)

### Step 4: Add module to markymark-kernels/src/lib.rs
- Add \`pub mod md4c;\` after existing module declarations
- Add re-export: \`pub use md4c::{extract_md4c, Md4cExtraction, Md4cHeading, Md4cLink};\`
- Verify: \`cargo build\` succeeds (linker finds marky_md4c_extract and marky_md4c_free symbols)

### Step 5: Write Zig-side tests in exports.zig
All tests use std.testing.allocator where possible (detects leaks in test code).
FFI functions internally use page_allocator (same as production).

\`\`\`zig
test "md4c_extract: simple heading" {
    const input = "# Hello\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.headings_count);
    // Verify text_blob contains "Hello"
    const blob = result.text_blob.?[0..result.text_blob_len];
    const h = result.headings.?[0];
    try testing.expectEqualStrings("Hello", blob[h.text_offset..h.text_offset + h.text_length]);
    try testing.expectEqual(@as(u8, 1), h.level);
}

test "md4c_extract: inline link with text and target" {
    const input = "[click](https://example.com)\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const l = result.links.?[0];
    try testing.expectEqualStrings("click", blob[l.text_offset..l.text_offset + l.text_length]);
    try testing.expectEqualStrings("https://example.com", blob[l.target_offset..l.target_offset + l.target_length]);
    try testing.expectEqual(@as(u8, 0), l.is_wiki);
}

test "md4c_extract: null text pointer returns -1" {
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(null, 10, &result);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "md4c_extract: null out pointer returns -1" {
    const input = "# Hello\n";
    const rc = marky_md4c_extract(input.ptr, input.len, null);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "md4c_extract: empty input returns zero results" {
    const input = "";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, 0, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.headings_count);
    try testing.expectEqual(@as(u32, 0), result.links_count);
}

test "md4c_extract: wiki link" {
    const input = "[[Target]]\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    try testing.expectEqual(@as(u8, 1), result.links.?[0].is_wiki);
}

test "md4c_extract: double free is no-op" {
    const input = "# Test\n";
    var result: CMd4cResult = undefined;
    _ = marky_md4c_extract(input.ptr, input.len, &result);
    marky_md4c_free(&result);
    // Second free should be no-op (result zeroed by first free)
    marky_md4c_free(&result);
}

test "md4c_extract: entity-decoded heading text in blob" {
    const input = "# Hello &amp; World\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const h = result.headings.?[0];
    // md4c decodes entities: &amp; → &
    try testing.expectEqualStrings("Hello & World", blob[h.text_offset..h.text_offset + h.text_length]);
}
\`\`\`

### Step 6: Write Rust-side integration tests in markymark-kernels/src/md4c.rs
\`\`\`rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_heading() {
        let result = extract_md4c("# Hello\n").unwrap();
        assert_eq!(result.headings.len(), 1);
        assert_eq!(result.headings[0].text, "Hello");
        assert_eq!(result.headings[0].level, 1);
        assert_eq!(result.headings[0].source_offset, 0);
    }

    #[test]
    fn test_extract_link() {
        let result = extract_md4c("[click](https://example.com)\n").unwrap();
        assert_eq!(result.links.len(), 1);
        assert_eq!(result.links[0].text, "click");
        assert_eq!(result.links[0].target, "https://example.com");
        assert!(!result.links[0].is_wiki);
    }

    #[test]
    fn test_extract_wiki_link() {
        let result = extract_md4c("[[Target]]\n").unwrap();
        assert_eq!(result.links.len(), 1);
        assert!(result.links[0].is_wiki);
        assert_eq!(result.links[0].target, "Target");
    }

    #[test]
    fn test_extract_mixed_document() {
        let input = "# Title\n\nSome [link](url) text.\n\n## Section\n\nSee [[Wiki]] for details.\n";
        let result = extract_md4c(input).unwrap();
        assert_eq!(result.headings.len(), 2);
        assert_eq!(result.headings[0].text, "Title");
        assert_eq!(result.headings[1].text, "Section");
        assert_eq!(result.links.len(), 2);
        assert!(!result.links[0].is_wiki);
        assert!(result.links[1].is_wiki);
    }

    #[test]
    fn test_extract_empty_input() {
        let result = extract_md4c("").unwrap();
        assert!(result.headings.is_empty());
        assert!(result.links.is_empty());
    }

    #[test]
    fn test_extract_entity_decoded_text() {
        let result = extract_md4c("# Hello &amp; World\n").unwrap();
        assert_eq!(result.headings[0].text, "Hello & World");
    }

    #[test]
    fn test_text_is_valid_utf8() {
        // Unicode content must survive the FFI round-trip as valid UTF-8
        let result = extract_md4c("# Héllo Wörld\n").unwrap();
        assert_eq!(result.headings[0].text, "Héllo Wörld");
    }

    #[test]
    fn test_struct_sizes_match_zig() {
        // These are also checked at compile time but belt-and-suspenders
        assert_eq!(std::mem::size_of::<CMd4cHeading>(), 16);
        assert_eq!(std::mem::size_of::<CMd4cLink>(), 24);
        assert_eq!(std::mem::size_of::<CMd4cResult>(), 40);
    }
}
\`\`\`

### Step 7: Verify full build pipeline
- \`zig build lib\` succeeds (md4c symbols linked into libmarky_kernels.a)
- \`zig build test\` passes all md4c + kernel tests (existing 25 extraction tests + new FFI tests)
- \`cargo build\` succeeds (linker finds marky_md4c_extract, marky_md4c_free)
- \`cargo nextest\` passes (all existing + new md4c integration tests)

## Success Criteria
- [ ] CMd4cHeading (16 bytes), CMd4cLink (24 bytes), CMd4cResult (40 bytes) — comptime/const_assert size checks pass in both Zig and Rust
- [ ] marky_md4c_extract returns correct headings (text, level, source_offset) verified by Zig test
- [ ] marky_md4c_extract returns correct links (text, target, source_offset, is_wiki) verified by Zig test
- [ ] marky_md4c_extract returns correct entity-decoded text in blob (# &amp; → &) verified by Zig test
- [ ] marky_md4c_extract returns -1 for null text pointer (no crash)
- [ ] marky_md4c_extract returns -1 for null out pointer (no crash)
- [ ] marky_md4c_extract returns zero counts for empty input
- [ ] marky_md4c_free + second marky_md4c_free is no-op (double-free safety)
- [ ] Rust extract_md4c returns owned Strings with correct content
- [ ] Rust extract_md4c preserves UTF-8 Unicode content through FFI round-trip
- [ ] 8+ Zig-side FFI tests pass (via zig build test)
- [ ] 8+ Rust-side integration tests pass (via cargo nextest)
- [ ] cargo build succeeds (linker resolves md4c symbols)
- [ ] All existing tests still pass (cargo nextest + zig build test — zero regressions)

## Key Considerations (SRE Review)

**Struct Alignment (CRITICAL)**:
Fields are ordered by alignment (u32 before u16 before u8, pointers before scalars) to eliminate implicit padding holes. Both Zig extern struct and Rust #[repr(C)] produce identical layout when fields are in the same order. Compile-time size assertions on BOTH sides catch mismatches at build time, not at runtime.

**text_length Changed to u32 (from u16)**:
Original design used u16 for text_length (max 65535 bytes). A heading with heavy inline content or a very long URL could exceed this. Changed to u32 to eliminate the overflow risk. Cost: 4 extra bytes per heading struct, 8 extra bytes per link struct — negligible given typical document sizes.

**Memory Management — Slice Reconstruction for page_allocator.free()**:
page_allocator.free(slice) requires the exact slice returned by alloc. The counts stored in CMd4cResult (headings_count, links_count, text_blob_len) enable slice reconstruction: \`ffi_allocator.free(headings[0..headings_count])\`. This works because alloc(T, n) returns exactly n elements and free expects the same n.

**Double-Free Prevention**:
marky_md4c_free zeroes the entire CMd4cResult after freeing. A second call sees null pointers and zero counts → no-op. This is tested explicitly.

**normalizeLabel Memory Leak (marky-i3fl)**:
Documents with reference links or wiki links trigger the known normalizeLabel leak in the vendored md4c parser. Since FFI uses page_allocator (which allocates whole pages), leaked label buffers waste page-sized chunks. This is tolerated until marky-i3fl is fixed. Not a blocker for this task.

**Thread Safety**:
page_allocator is thread-safe. ExtractionRenderer uses no global state. Multiple threads calling marky_md4c_extract concurrently is safe — each call has independent allocations and state.

**Error Code Semantics**:
- 0: success
- -1: null pointer (invalid input)
- -3: parse error (md4c failed)
- -4: out of memory (allocation failed)
Codes -3/-4 are new to this project (existing FFI uses only 0/-1/-2). The -2 code (buffer too small) is NOT used because md4c extraction uses Zig-allocates pattern.

**errdefer Cascade in marky_md4c_extract**:
The packing code has 3 allocations (blob, headings, links). If a later allocation fails, earlier ones must be freed. Use explicit cleanup (not errdefer) since the ExtractionResult also needs deinit. Implement as:
1. Allocate blob → on fail: deinit ExtractionResult, return -4
2. Allocate headings → on fail: free blob, deinit ExtractionResult, return -4
3. Allocate links → on fail: free headings, free blob, deinit ExtractionResult, return -4
4. Success: copy data, deinit ExtractionResult, write *out

**Zero Results Edge Case**:
If document has 0 headings or 0 links, do NOT allocate empty arrays. Set pointer to null and count to 0. marky_md4c_free handles null pointers. If BOTH are zero and text_blob_len is 0, skip blob allocation too.

## Anti-Patterns
- ❌ Do NOT modify extraction_renderer.zig — this task wraps it, doesn't change it
- ❌ Do NOT use call_scan_ffi retry pattern — md4c uses Zig-allocates pattern, not caller-allocated buffers
- ❌ Do NOT store raw pointers into Rust-owned data across FFI — copy text immediately then free
- ❌ Do NOT use @panic or unreachable in FFI functions — return error codes
- ❌ Do NOT assume text_blob lifetime extends past marky_md4c_free — Rust must copy before calling free
- ❌ Do NOT pass std.testing.allocator across FFI boundary — use page_allocator for all FFI allocations
- ❌ Do NOT use .unwrap() or .expect() in Rust FFI wrapper — use Result and KernelError
- ❌ Do NOT omit SAFETY comments on unsafe blocks — follow existing scan.rs pattern with nosemgrep annotations
- ❌ Do NOT rely on implicit padding in extern structs — use explicit _padding fields and size assertions
- ❌ Do NOT allocate arrays for zero-count results — use null pointer with count=0
