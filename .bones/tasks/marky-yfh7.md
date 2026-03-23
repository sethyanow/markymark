---
id: marky-yfh7
title: 'ExtractionRenderer: entity references not decoded (e.g. &amp; stays as &amp;)'
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


## Design

## Root Cause

md4c parser emits entity references (e.g. `&amp;`) via `TextType.entity` callback with raw bytes.
ExtractionRenderer.text() ignores the TextType parameter — treats all text types identically,
appending raw bytes to buffers. No entity decoding happens.

## Fix: Zig-side entity decoding in ExtractionRenderer.text()

When `text_type == .entity`, call `helpers.decodeEntityToUtf8()` to decode the entity to UTF-8
before appending to heading_text_buf/link_text_buf. This uses the same decode infrastructure
that HtmlRenderer already uses.

## Files to Change

1. `zig/src/md4c/extraction_renderer.zig` — Modify text() to decode .entity text type
2. `zig/src/md4c/exports.zig` — Update test expectation (currently asserts raw entity)
3. `markymark-core/src/scanner.rs` — Update Rust test expectation
4. `markymark-kernels/src/md4c.rs` — Update Rust FFI test expectation

## Success Criteria

- [ ] Entity references in headings decoded (e.g. `&amp;` → `&`)
- [ ] Entity references in links decoded
- [ ] Numeric entities decoded (e.g. `&#38;` → `&`)
- [ ] Named entities decoded (e.g. `&lt;` → `<`)
- [ ] Unknown entities passed through as-is (fallback)
- [ ] All Zig tests pass with memory leak detection
- [ ] All Rust tests pass
