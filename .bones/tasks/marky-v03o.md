---
id: marky-v03o
title: 'Write ExtractionRenderer for md4c: single-pass heading/link/wikilink extraction with byte offsets'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---



## Goal
Write a Zig ExtractionRenderer that implements the md4c Renderer vtable to extract headings, markdown links, and wiki links in a single parse pass, producing output compatible with the Rust ScanBackend result types.

## Context
- md4c Renderer vtable: enterBlock/leaveBlock/enterSpan/leaveSpan/text callbacks (zig/src/md4c/types.zig)
- HtmlRenderer (zig/src/md4c/html_renderer.zig) is the reference implementation pattern
- Target output: HeadingResult (text, offset, level), LinkResult (offset, text, target, link_type) — matching markymark-core/src/scanner.rs
- md4c text callback receives content from parser internal buffer, not direct source pointers — byte offsets must be computed via progressive source scanning

## Design: Progressive Source Scan for Byte Offsets

md4c processes blocks in document order. The ExtractionRenderer maintains a scan cursor into the source text and searches forward for each extracted element:
- Headings: search for #{level} text (ATX) from cursor
- Markdown links: search for [text](target) from cursor
- Wiki links: search for [[target]] or [[target|alias]] from cursor

## Implementation Steps
1. Create extraction_renderer.zig with struct, vtable, init/deinit
2. Implement heading extraction (enterBlock .h + text + leaveBlock .h)
3. Implement link extraction (enterSpan .a + text + leaveSpan .a)
4. Implement wiki link extraction (enterSpan .wikilink + leaveSpan .wikilink)
5. Code block tracking (skip extraction inside code fences)
6. Public extractFromMarkdown() function
7. Comprehensive unit tests

## Success Criteria
- ExtractionRenderer extracts ATX headings with correct text, level, byte offset
- ExtractionRenderer extracts markdown links with correct text, target, byte offset
- ExtractionRenderer extracts wiki links with correct target, alias, byte offset
- Code blocks do not produce false extractions
- Byte offsets are accurate
- All unit tests pass
- Existing md4c smoke tests still pass
- No memory leaks

## Design

## Goal
Write a Zig ExtractionRenderer that implements the md4c Renderer vtable to extract headings, markdown links, and wiki links in a single parse pass, producing output compatible with the Rust ScanBackend result types (markymark-core/src/scanner.rs).

## Context
- md4c Renderer vtable: enterBlock/leaveBlock/enterSpan/leaveSpan/text callbacks (zig/src/md4c/types.zig:118-145)
- HtmlRenderer (zig/src/md4c/html_renderer.zig) is the reference implementation pattern
- Public API: root.zig:97 renderWithRenderer(text, allocator, options, renderer) — this is how ExtractionRenderer plugs in
- Target output must match: HeadingResult (text, offset, level), LinkResult (offset, text, target, link_type) from markymark-core/src/scanner.rs:40-71
- md4c text callback receives content from parser internal buffer (inlines.zig:28-49), NOT direct source pointers
- enterBlock(.h, data=level, flags) — flags includes BLOCK_SETEXT_HEADER (types.zig:244) for setext headings
- Callback order: enterBlock → processLeafBlock → text/enterSpan/leaveSpan → leaveBlock (containers.zig:169-180)

## Design: Offset Recovery via Pattern Search

md4c processes blocks in document order (containers.zig processAllBlocks iterates block_bytes linearly). The ExtractionRenderer maintains a forward-only scan cursor into the source text. On each element completion (leaveBlock/leaveSpan), it searches FORWARD from the cursor for the element's source pattern:

**Heading offsets (ATX):**
Search forward from cursor for a line containing N consecutive '#' followed by space, where N = heading level. Do NOT search for heading text (entity decoding may alter it). Record offset of first '#'.

**Heading offsets (Setext):**
Detect via flags parameter: enterBlock(.h, data=level, flags) where flags & BLOCK_SETEXT_HEADER != 0. Search forward for line followed by '===' (level 1) or '---' (level 2) underline. Record offset of first text line.

**Link offsets (inline [text](url)):**
Search forward from cursor for '[' character. Validate by checking for '](' after the opening bracket. Record offset of '['.

**Link offsets (autolink <url>):**
Detect via SpanDetail.autolink == true. Search forward for '<' character followed by url text.

**Link offsets (reference [text][ref]):**
Same as inline — search for '[' character. Reference links also start with '['. The ExtractionRenderer does not need to distinguish inline from reference after offset recovery.

**Wiki link offsets:**
Search forward from cursor for '[[' pattern. Record offset of first '['.

**Why forward-only works:** md4c callback order matches source order. Each element appears later in the source than the previous one. Advancing cursor past each found element prevents matching the wrong duplicate.

## Result Types (Zig)

Location: zig/src/md4c/extraction_renderer.zig

```zig
pub const ExtractedHeading = struct {
    text: []const u8,     // owned copy of heading text (allocator)
    offset: u32,          // byte offset of '#' (ATX) or first text byte (setext) in source
    level: u8,            // 1-6
};

pub const ExtractedLink = struct {
    text: []const u8,     // owned copy of display text (allocator)
    target: []const u8,   // owned copy of href/URL (allocator)
    offset: u32,          // byte offset of '[' (links) or '[[' (wiki) in source
    is_wiki: bool,        // true for [[wiki]] links
};

pub const ExtractionResult = struct {
    headings: []ExtractedHeading,
    links: []ExtractedLink,
    allocator: Allocator,

    pub fn deinit(self: *ExtractionResult) void {
        for (self.headings) |h| {
            self.allocator.free(h.text);
        }
        self.allocator.free(self.headings);
        for (self.links) |l| {
            self.allocator.free(l.text);
            self.allocator.free(l.target);
        }
        self.allocator.free(self.links);
    }
};
```

## Implementation Steps

### Step 1: Create extraction_renderer.zig skeleton
- Location: zig/src/md4c/extraction_renderer.zig
- ExtractionRenderer struct fields:
  - src_text: []const u8 (original source for offset scanning)
  - allocator: Allocator
  - headings: std.ArrayListUnmanaged(ExtractedHeading) = .{}
  - links: std.ArrayListUnmanaged(ExtractedLink) = .{}
  - scan_cursor: u32 = 0 (forward-only position in source)
  - in_heading: bool = false
  - heading_level: u8 = 0
  - heading_is_setext: bool = false
  - heading_text_buf: std.ArrayListUnmanaged(u8) = .{} (accumulates text)
  - in_link: bool = false
  - in_image: bool = false (to SKIP image spans)
  - link_is_wiki: bool = false
  - link_is_autolink: bool = false
  - link_text_buf: std.ArrayListUnmanaged(u8) = .{}
  - link_href_buf: std.ArrayListUnmanaged(u8) = .{}
  - in_code_block: bool = false
- init(allocator, src_text) → ExtractionRenderer
- deinit(self, allocator) — free all ArrayLists and owned strings
- renderer(self) → Renderer — returns vtable-based interface (same pattern as HtmlRenderer:59-61)
- const vtable: Renderer.VTable with 5 function pointers (same pattern as HtmlRenderer:63-69)

### Step 2: Implement enterBlock/leaveBlock callbacks
enterBlockImpl:
- .h → set in_heading=true, heading_level=@truncate(data), heading_is_setext=(flags & BLOCK_SETEXT_HEADER != 0), clear heading_text_buf
- .code → set in_code_block=true
- all others → no-op

leaveBlockImpl:
- .h → finalize heading (see Step 2a), set in_heading=false
- .code → set in_code_block=false
- all others → no-op

Step 2a — finalizeHeading:
- owned_text = heading_text_buf.toOwnedSlice(allocator)
- offset = findHeadingOffset(src_text, scan_cursor, heading_level, heading_is_setext)
- if offset found: update scan_cursor past the heading
- if offset NOT found: use scan_cursor as fallback (do NOT crash)
- append ExtractedHeading{text, offset, level} to headings list

findHeadingOffset(src, cursor, level, is_setext):
- if is_setext: search from cursor for line followed by '===' (level 1) or '---' (level 2); return offset of text line start
- if ATX: search from cursor for N '#' characters followed by ' ' (allowing leading whitespace and blockquote '>' prefixes); return offset of first '#'

### Step 3: Implement enterSpan/leaveSpan callbacks
enterSpanImpl:
- .a → if NOT in_image: set in_link=true, link_is_wiki=false, link_is_autolink=detail.autolink, copy detail.href into link_href_buf
- .wikilink → set in_link=true, link_is_wiki=true, link_is_autolink=false, copy detail.href into link_href_buf
- .img → set in_image=true (images are NOT extracted as links)
- all others → no-op

leaveSpanImpl:
- .a → if in_link (not in_image): finalize link (Step 3a), set in_link=false
- .wikilink → if in_link: finalize link, set in_link=false
- .img → set in_image=false
- all others → no-op

Step 3a — finalizeLink:
- owned_text = link_text_buf.toOwnedSlice(allocator)
- owned_target = link_href_buf.toOwnedSlice(allocator)
- offset = findLinkOffset(src_text, scan_cursor, link_is_wiki, link_is_autolink)
- update scan_cursor past the link
- append ExtractedLink{text, target, offset, is_wiki} to links list

findLinkOffset(src, cursor, is_wiki, is_autolink):
- if is_wiki: search from cursor for '[['; return offset of first '['
- if is_autolink: search from cursor for '<'; return offset of '<'
- else: search from cursor for '[' (skip any '!' before it for images already handled); return offset of '['

### Step 4: Implement text callback
textImpl:
- if in_code_block: return (skip all text inside code blocks)
- if in_heading: append content to heading_text_buf
- if in_link AND NOT in_image: append content to link_text_buf
- Note: accumulate ALL text types (.normal, .code, .entity, .html) — md4c delivers decoded text

### Step 5: Public extractFromMarkdown() function
Location: extraction_renderer.zig public function

```zig
pub fn extractFromMarkdown(
    text: []const u8,
    allocator: Allocator,
) !ExtractionResult {
    const input = helpers.skipUtf8Bom(text);
    var ext = ExtractionRenderer.init(allocator, input);
    errdefer ext.deinit(allocator);

    const flags = types.Flags{
        .tables = true,
        .strikethrough = true,
        .tasklists = true,
        .wiki_links = true, // CRITICAL: must enable wiki link parsing
    };
    try parser_mod.renderWithRenderer(input, allocator, flags, .{}, ext.renderer());

    return ExtractionResult{
        .headings = ext.headings.toOwnedSlice(allocator),
        .links = ext.links.toOwnedSlice(allocator),
        .allocator = allocator,
    };
}
```

### Step 6: Add pub import to root.zig
Add to zig/src/md4c/root.zig:
```zig
pub const extraction_renderer = @import("./extraction_renderer.zig");
```

### Step 7: Write unit tests in extraction_renderer.zig
Each test uses std.testing.allocator (detects leaks).

**Heading tests:**
- test_atx_heading_level_1: "# Hello\n" → [{text="Hello", level=1, offset=0}]
- test_atx_heading_levels_1_through_6: "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n" → 6 headings with correct levels
- test_atx_heading_byte_offset: "Some text\n\n# Heading\n" → offset of '#' (not 0)
- test_setext_heading_level_1: "Hello\n=====\n" → [{text="Hello", level=1}]
- test_setext_heading_level_2: "Hello\n-----\n" → [{text="Hello", level=2}]
- test_heading_with_inline_formatting: "# Hello **bold** world\n" → text includes "Hello bold world" (inline markup stripped by md4c)
- test_duplicate_headings_sequential_offsets: "# Same\n\n# Same\n" → two headings with different offsets (second offset > first)
- test_heading_in_blockquote: "> # Quoted\n" → [{text="Quoted", level=1}]
- test_empty_heading: "#\n" or "# \n" → heading with empty text (edge case)

**Link tests:**
- test_inline_link: "[click](https://example.com)\n" → [{text="click", target="https://example.com", is_wiki=false}]
- test_inline_link_byte_offset: "Hello [click](url)\n" → offset points to '['
- test_autolink: "<https://example.com>\n" → link extracted (autolink flag)
- test_reference_link: "[text][ref]\n\n[ref]: https://example.com\n" → [{text="text", target="https://example.com"}] (needs page_allocator due to ref_defs leak)
- test_link_with_title: "[text](url \"title\")\n" → text="text", target="url"
- test_image_not_extracted_as_link: "![alt](img.png)\n" → links list is EMPTY (images excluded)
- test_link_inside_heading: "# See [here](url)\n" → heading extracts text "See here", link also extracted separately

**Wiki link tests (require Options.wiki_links = true):**
- test_wiki_link: "[[Target]]\n" → [{text="Target", target="Target", is_wiki=true}]
- test_wiki_link_with_alias: "[[Target|Display]]\n" → [{text="Display", target="Target", is_wiki=true}]
- test_wiki_link_byte_offset: "Text [[Target]]\n" → offset of first '['

**Code block exclusion tests:**
- test_heading_in_code_block_not_extracted: "```\n# Not a heading\n```\n" → headings is EMPTY
- test_link_in_code_block_not_extracted: "```\n[not](a-link)\n```\n" → links is EMPTY

**Mixed document test:**
- test_mixed_document: doc with headings + links + wiki links → all extracted with correct offsets in ascending order

**Edge cases:**
- test_empty_input: "" → empty headings, empty links
- test_no_headings_or_links: "Just plain text.\n" → empty headings, empty links
- test_entity_in_heading: "# Hello &amp; World\n" → text="Hello & World" (md4c decodes entities), offset still correct (points to '#')

## Success Criteria
- [ ] 20+ unit tests pass via zig build test (all listed above)
- [ ] std.testing.allocator detects zero memory leaks in all tests
- [ ] Byte offsets verified against source text: src[offset] matches expected character ('#' for ATX headings, '[' for links, etc.)
- [ ] Existing 4 md4c smoke tests in root.zig still pass
- [ ] Existing SIMD kernel tests still pass (zig build test runs all)
- [ ] Images (![alt](url)) produce zero entries in links list
- [ ] Code blocks produce zero entries in headings/links lists
- [ ] Duplicate headings get distinct ascending offsets
- [ ] Entity-encoded headings (# &amp;) get correct text AND correct offset

## Key Considerations

**Entity decoding in text callback (CRITICAL):**
md4c decodes HTML entities before calling text callbacks. Source "# Hello &amp; World" produces text callback with "Hello & World". The ExtractionRenderer must NOT search source for decoded text. Instead, find heading offset by pattern (#{level} + space), not by text content match.

**Reference link dest comes from ref-def location:**
enterSpan(.a, .{.href=dest}) for reference links [text][ref] — dest is a slice from the ref-def at the bottom of the document, not near the link usage. Do NOT use href pointer for offset computation. Use '[' pattern search instead.

**Autolinks have different source syntax:**
<https://url> → enterSpan(.a, .{.href=url, .autolink=true}). Source pattern is '<url>' not '[text](url)'. Check detail.autolink flag and search for '<' instead of '['.

**Image nesting with links:**
![alt](img.png) triggers enterSpan(.img). Links INSIDE image alt text are suppressed by md4c (image_nesting_level). ExtractionRenderer should track in_image and skip .a spans while in_image=true.

**Blockquote-prefixed headings:**
"> # Hello" — search for '#' anywhere (not just line start) because blockquote '>' prefix precedes it. Forward-only scan + '#'+space is sufficient.

**Setext heading detection:**
enterBlock(.h, data=level, flags) — check flags & BLOCK_SETEXT_HEADER (types.zig:244, value 0x08). Setext headings: text line followed by '===' or '---'. Search for underline pattern.

**Scan cursor fallback:**
If forward scan fails to find expected pattern, use current scan_cursor as offset fallback. Log or track the failure but do NOT crash or return error. Partial results are better than no results.

**Memory ownership:**
All text/target strings in ExtractedHeading/ExtractedLink are OWNED by the ExtractionRenderer allocator. ExtractionResult.deinit() must free every string and every slice. Tests use std.testing.allocator which detects leaks.

**renderWithRenderer API:**
The parser requires an allocator for its internal buffer (used by processLeafBlock). This is separate from ExtractionRenderer allocator. Both can be the same allocator. The renderWithRenderer call signature is in parser.zig:253.

**Wiki links require flags.wiki_links = true:**
md4c only recognizes [[...]] syntax when wiki_links flag is set. extractFromMarkdown() must set this flag. Without it, [[link]] is treated as nested brackets.

## Anti-Patterns
- Do NOT modify md4c parser source files (blocks.zig, inlines.zig, links.zig, etc.)
- Do NOT modify the Renderer VTable signature in types.zig
- Do NOT add FFI exports or C ABI — this is pure Zig, no c_adapter.zig changes
- Do NOT search source for decoded text content — use structural patterns (# + space, [, [[, <)
- Do NOT use @panic or unreachable — use error returns or fallback values
- Do NOT assume text callback content points into source text — it comes from parser buffer (inlines.zig:39)
- Do NOT extract images as links — track in_image state from .img spans
- Do NOT skip deinit of ArrayListUnmanaged — every list and owned string must be freed
