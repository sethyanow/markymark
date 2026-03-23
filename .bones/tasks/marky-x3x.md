---
id: marky-x3x
title: 'CodeRabbit triage: 2 critical bugs + 2 improvements from feature-mark-brza review'
status: closed
type: task
priority: 1
owner: sethyanow@users.noreply.github.com
---

CodeRabbit review of committed changes (feature-mark-brza vs main) identified 4 issues:

## 🔴 Priority 1 - Critical Bugs (Fix Now)

### 1. Wiki link offset calculation bug
**File:** markymark-index/src/document.rs:391-393
**Issue:** End offset calculation assumes Markdown link format [text](target) for all links. Wiki links use [[target]] or [[target|alias]] format. For aliased wiki links, the math is incorrect.
**Impact:** Incorrect range calculations for wiki links, causing positioning errors in LSP features.
**Fix:** Match on l.link_type and compute length accordingly:
- Markdown: l.offset + l.text.len() + l.target.len() + 4
- Wiki: l.offset + l.target.len() + l.text.len() + 4 + (l.text != l.target ? 1 : 0)

### 2. Zero-width block range bug
**File:** markymark-index/src/document.rs:453-459
**Issue:** Block entry uses Range::new(pos, pos) creating zero-width range. Differs from from_ast which provides actual ranges.
**Impact:** Block ranges don't reflect actual source spans, breaking features relying on accurate ranges.
**Fix:** Compute proper end position: byte_offset_to_position(&line_starts, b.offset + b.id.len() as u32 + 1)

## 💡 Priority 2 - Improvements (Fix Soon)

### 3. Duplicate pattern entries
**File:** .claude-harness/memory/procedural/patterns.json:40-46
**Issue:** 7 patterns appear twice in the array (cross-agent handoff, ArenaHashMap cloning, bumpalo Vec, regex types, benchmarks, block IDs)
**Fix:** Remove duplicate entries programmatically or manually deduplicate.

### 4. Nested code fence rendering
**File:** docs/plans/brza-markymark.md:253-259
**Issue:** Example has nested triple-backticks that may render incorrectly.
**Fix:** Use different fence markers (e.g., ~~~ inside ```) to avoid conflicts.

## Recommended Approach
- Fix bugs #1 and #2 using TDD (write failing tests first)
- Fix improvement #3 (patterns.json) as part of cleanup
- Fix improvement #4 (docs) when convenient
