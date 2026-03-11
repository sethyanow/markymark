---
id: marky-36a
title: 'Fix: EmbeddingIndex !Send blocks semantic-search feature in markymark-mcp'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-8s3]
---



The EmbeddingIndex type in markymark-kernels/src/embed.rs uses PhantomData<*mut ()> to explicitly mark itself as !Send + !Sync. This causes the markymark-mcp crate to fail compilation when --features semantic-search is enabled, because RuntimeEngine (backed by RwLock<HashMap<String, RealmData>>) requires Send + Sync.

Pre-existing bug confirmed: even before the marky-8s3.11 changes, cargo test -p markymark-mcp --features semantic-search failed (with ZigEmbeddingIndex type errors).

Fix: The Zig embedding index heap allocation is thread-transferable (no per-thread state). Implementing unsafe impl Send for EmbeddingIndex is sound — the RwLock in RuntimeEngine guarantees only one writer at a time, and the Zig search function (zig_embedding_index_search) operates on an immutable snapshot. Add safety comment explaining the invariants.

Files to change:
- markymark-kernels/src/embed.rs: add unsafe impl Send for EmbeddingIndex with safety comment
- Verify markymark-mcp compiles with --features semantic-search after fix
- Run all tests including markymark-mcp --features semantic-search
