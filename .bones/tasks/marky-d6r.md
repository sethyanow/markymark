---
id: marky-d6r
title: Audit EmbeddingIndex unsafe impl Sync soundness
status: active
type: task
priority: 3
owner: Seth
---


## Context

`EmbeddingIndex` (markymark-kernels/src/embed.rs:101) has `unsafe impl Sync` with a documented
invariant: only safe under external RwLock. The safety comment at embed.rs:91-99 is explicit.

**Invariant verification:** The call chain enforces the invariant:
- `EmbeddingIndex` → `SemanticIndex` (wraps it, markymark-index/src/semantic/mod.rs:23)
- `SemanticIndex` → `RealmIndex` stores as `Arc<TokioMutex<SemanticIndex>>` (realm/mod.rs:76)
- tokio Mutex serializes all access — invariant upheld

**Finding:** The `unsafe impl Sync` is sound given current usage. The risk is that a future
caller bypasses the Mutex wrapper. The safety comment documents this but there's no compile-time
enforcement.

## Decision needed

Options:
1. **Accept current state** — safety comment is clear, usage is correct, close as verified
2. **Add a newtype wrapper** that is `!Sync` and only exposes access through a lock guard,
   making the invariant compile-time enforced instead of comment-enforced

## Success Criteria

- [ ] Invariant verified or compile-time enforcement added
