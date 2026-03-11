---
id: marky-g9t.2
title: Migrate markymark-parser types to arena lifetimes
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9t.1, marky-luy]
parent: marky-g9t
---





Thread 'arena lifetime through all 19 types in markymark-parser/src/types.rs. String → &'arena str, Vec<T> → BumpVec<'arena, T>, HashMap<K,V> → hashbrown HashMap with bumpalo allocator. Update Element enum. This is the largest single task (~732 lines). Do NOT change extraction logic yet — just the type definitions and their constructors.

Success: types.rs compiles with 'arena lifetimes. Tests will fail (expected — extraction not updated yet).
