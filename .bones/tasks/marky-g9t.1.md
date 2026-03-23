---
id: marky-g9t.1
title: Add hashbrown dep and arena infrastructure to markymark-core
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-luy]
parent: marky-g9t
---




Add hashbrown workspace dep. Create markymark-core/src/arena.rs module with type aliases (ArenaStr, ArenaVec, ArenaHashMap) and a DocumentArena wrapper around Bump. This establishes the shared vocabulary all crates will use. No functional changes to existing types yet.

Success: cargo build passes, new module exists with type aliases.
