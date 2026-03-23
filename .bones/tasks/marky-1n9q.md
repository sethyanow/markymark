---
id: marky-1n9q
title: 'Bug: unsound Sync impl on DocumentEngine - get_blob mutates Zig state'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
---

DocumentEngine is marked Sync but get_blob(&self) calls marky_engine_get_blob which mutates Zig-side cached_blob on cache miss. Two threads calling get_blob concurrently through &DocumentEngine race on that mutable state — UB across the FFI boundary. The comment acknowledges this requires external locking, but Sync promises thread-safe &self access without preconditions. Fix: remove unsafe impl Sync for DocumentEngine. Only Send is needed (DocumentEngine lives inside RwLock<ServerState> which only requires Send on the inner type).
