---
id: marky-lzd5
title: 'ExtractionRenderer offset scan hardening: fence matching + paren URLs'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

PR #41 round 2 CodeRabbit findings (post marky-0rl6/4atp/pk33 fixes).

Three related bugs in extraction_renderer.zig offset scan functions:

**F1: Fence char/length tracking in ATX heading scan (findHeadingOffset:344-360)**
`in_fence = !in_fence` toggles on ANY 3+ fence chars. A backtick fence can be 
incorrectly closed by a tilde line (or vice versa). Must track opening fence 
char and length, only close on matching char with length >= opening length.

**F2: Same bug in link scan (findLinkOffset:433-449)**
Identical `in_fence = !in_fence` pattern. Same fix needed.

**F3: No fence tracking in setext heading scan (findHeadingOffset:300-340)**
Setext path scans raw text for `===`/`---` underlines with zero fence awareness.
Will match underlines inside code blocks, returning wrong offset.

**F4: Paren URL truncation in link scan (findLinkOffset:467-470)**
`while (end < src.len and src[end] != ')') : (end += 1) {}` stops at first `)`.
URLs with parens like `https://en.wikipedia.org/wiki/Foo_(bar)` produce wrong
end_offset. Must track paren depth. Handle backslash escapes.

All are offset-only issues — md4c extraction is correct, only the source 
position in the index is wrong. Affects LSP hover/goto-def ranges.

Existing test coverage: T1-5 tests cover basic fence skip (same char), but NOT
mismatched chars. "link inside heading" tests assert offsets but not paren URLs.

SRE Corner Cases:
- ```` (4 backticks) opened, ``` (3 backticks) should NOT close it
- ~~~ inside ``` fence should NOT toggle fence state
- Setext heading after code block containing `---` line
- Wikipedia URL: [link](https://en.wikipedia.org/wiki/Foo_(bar))
- Nested parens: [link](url(a(b)))
- Escaped paren: [link](url\(not-paren\))

## Design

## Goal

Harden ExtractionRenderer offset scanning to handle: mismatched fence 
delimiters (backtick vs tilde), fence length mismatches, setext headings 
inside fenced blocks, and parenthesized URLs.

## Implementation Plan

### F1+F2: Fence char/length tracking (ATX heading + link scans)

In findHeadingOffset ATX path (line 344) and findLinkOffset inline path (line 433):

Replace:
  var in_fence = false;
  ...
  if (flen >= 3) {
      in_fence = !in_fence;
  }

With:
  var in_fence = false;
  var fence_char: u8 = 0;
  var fence_len: u32 = 0;
  ...
  if (flen >= 3) {
      if (!in_fence) {
          in_fence = true;
          fence_char = fc;
          fence_len = flen;
      } else if (fc == fence_char and flen >= fence_len) {
          in_fence = false;
          fence_char = 0;
          fence_len = 0;
      }
      // else: different char or shorter fence — ignore, stay in fence
  }

### F3: Setext path fence awareness (line 300)

The setext path (heading_is_setext branch) scans for text + underline 
without any fence tracking. Add fence tracking at the line-scan level:

  var in_fence_s = false;
  var fence_char_s: u8 = 0;
  var fence_len_s: u32 = 0;
  while (pos < src.len) {
      // Check for fence at line start (same pattern as ATX)
      // ... detect fence, track char/len ...
      if (in_fence_s) { pos = next_line; continue; }
      // ... existing setext detection logic ...
  }

### F4: Paren depth tracking (line 467)

Replace:
  if (end < src.len and src[end] == '(') {
      end += 1;
      while (end < src.len and src[end] != ')') : (end += 1) {}
      if (end < src.len) end += 1;
  }

With:
  if (end < src.len and src[end] == '(') {
      end += 1;
      var paren_depth: u32 = 1;
      while (end < src.len and paren_depth > 0) {
          if (src[end] == '\\' and end + 1 < src.len) {
              end += 2;
              continue;
          }
          if (src[end] == '(') paren_depth += 1;
          if (src[end] == ')') paren_depth -= 1;
          if (paren_depth > 0) end += 1;  // don't advance past closing paren
      }
      if (paren_depth == 0) end += 1;  // skip final ')'
  }

## Test Matrix

| # | Input Pattern | Asserts |
|---|---------------|---------|
| 1 | ``` containing ~~~, then # H1 | heading offset at # H1, not inside fence |
| 2 | ~~~ containing ```, then # H1 | heading offset at # H1, not inside fence |
| 3 | ```` containing ```, then # H1 | heading NOT closed by shorter fence |
| 4 | ``` containing ---, then setext H1 with === | setext offset at text line, not code block |
| 5 | [link](url_(bar)) | link end_offset covers full URL including (bar) |
| 6 | [link](url(a(b))) | nested parens handled |
| 7 | [link](url\(esc\)) | escaped parens not counted |

## Success Criteria

- All 7 new tests pass
- All existing extraction_renderer tests pass (no regression)
- zig build test clean
- cargo nextest clean

## Risk Assessment

LOW — changes are isolated to two functions in one file. Only offset 
calculation is affected, never extraction correctness. All changes are 
additive (extra tracking variables, depth counter). No API changes.

## Anti-patterns
- Do NOT modify md4c extraction behavior — only offset recovery scanning
- Do NOT add fence tracking as struct fields — use local variables (per-call state)
- Do NOT handle tileset mismatches in autolink/wiki paths — those don't need fence skip
