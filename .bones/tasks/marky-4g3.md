---
id: marky-4g3
title: Surface warnings when extract_content_blocks catch_unwind fires
status: open
type: task
priority: 3
depends_on: [marky-v6c]
parent: marky-p88
---





## Context

Finding from the 2026-04-20 debugging session (epic marky-p88).

`markymark_index::document::from_engine::extract_content_blocks` at `markymark-index/src/document/from_engine.rs:34` wraps its inner call in `std::panic::catch_unwind(...).unwrap_or_default()`. Without logging:

```rust
fn extract_content_blocks(source: &str) -> Vec<RawBlock> {
    std::panic::catch_unwind(|| extract_content_blocks_inner(source)).unwrap_or_default()
}
```

When a panic fires, the caller receives an empty `Vec<RawBlock>`. The file is silently dropped from the content-block index. No warning, no error, no metric.

This is how bug marky-prs went undiagnosed: the panic hook wrote to stderr (which Claude Code may or may not surface to the user), and the observable symptom was "this file has no content blocks" — indistinguishable from a legitimately empty file.

With marky-prs fixed, the known panic source is gone, but the defence-in-depth guard remains. If a new regression introduces another panic source, we will silently drop files again.

## Requirements

1. When `catch_unwind` catches a panic in `extract_content_blocks`, log it via `log::warn!` (or `log::error!`) with:
   - Source length
   - The panic payload if downcast-able to `String` / `&str`
   - Enough context to identify the file (caller may need to pass a hint, e.g. URI)
2. Do NOT change the return value behaviour — still return `Vec::new()` so callers don't see a new error type.
3. Counter/metric (optional, but ideal): increment a panic-caught counter so we can alert if it fires in production.

## Investigation notes

- `log` crate is already a workspace dep — usable directly.
- Caller `from_engine_result_with_source` has `source` but no URI. Consider adding a hint parameter, or logging only the source-length hash.
- Check whether any test intentionally triggers this path — if yes, suppress the log for that test or capture it.

## Success Criteria

- [ ] `catch_unwind` no longer silently swallows panics — at minimum a `warn!` fires
- [ ] Log message contains enough info to identify which input caused it
- [ ] Existing tests still pass (no spurious panic-catch logs)
- [ ] New regression test asserts the log fires when a synthetic panic is triggered
