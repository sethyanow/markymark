---
id: marky-lpb
title: 'Phase 1: Content Hash Short-Circuit'
status: open
type: epic
priority: 2
depends_on: [marky-a02, marky-1ic, marky-840]
parent: marky-zsys
---









## Context
Parent epic marky-zsys, Phase 1. No phase dependency (first phase).
Exposes the content hash that Zig already computes on every parse and uses it to
short-circuit the expensive blob serialization + deserialization when the extracted
structure hasn't changed. The parse still runs, but ~2ms of blob + arena work is
skipped on edits that don't change document structure.

## Requirements
- R1: Zig DocumentEngine exposes content hash via C FFI (`marky_engine_get_content_hash`)
- R2: LSP update path short-circuits blob serialization + deserialization when content hash is unchanged after `engine.update()`

## Success Criteria
- [x] `marky_engine_get_content_hash` C function exported from Zig, declared in Rust extern block
- [x] `DocumentEngine::content_hash()` method returns `u64` on Rust side
- [x] `ServerState.engines` stores last-known hash alongside each `DocumentEngine` (EngineState wrapper)
- [x] `build_markdown_index_via_engine` returns `Option<DocumentIndex>` — `None` when hash unchanged
- [x] `change_document` and `apply_document_changes` skip `realm.update_document()` when `None`
- [x] Test: engine FFI returns consistent hash for same content
- [x] Test: hash changes when heading/link structure changes
- [x] Test: `build_markdown_index_via_engine` returns `None` for no-op structural edit
- [x] Benchmark: existing bench operates below short-circuit; test proves None returned. Savings ~2ms blob/arena per epic analysis.
- [x] All existing tests pass (208 tests, 0 failures)

## Anti-Patterns
- NO pre-parse Rust-side text hashing (misses frontmatter masking, competes with engine's hash)
- NO caching previous DocumentIndex (owned by RealmIndex after handoff; return None instead)
- NO modifying engine.update() atomicity contract (old state preserved on failure)

## Key Considerations
- The hash is computed on raw text input to md4c, not on extracted structure. Two different texts
  that produce identical headings/links will have different hashes. This is conservative (might
  rebuild blob unnecessarily) but never wrong (never skips a real change).
- `build_markdown_index_via_engine` currently takes `&mut self` — the Option return is a signature
  change that all callers must handle.
- `ServerState.engines` is `HashMap<String, Mutex<DocumentEngine>>` — needs to become
  `HashMap<String, Mutex<EngineState>>` or similar wrapper to store hash alongside engine.

## Acceptance Requirements
**Agent Documentation:**
- [x] CLAUDE.md: no updates needed (internal optimization, no API change)
- [x] docs/MEMORY.md: update with content hash short-circuit decision

**User Walkthrough Must Cover:**
- FFI round-trip: create engine, get hash, update with same text, hash unchanged
- Short-circuit path: edit that doesn't change structure triggers None return
- Changed path: edit that adds a heading triggers Some(index) return
