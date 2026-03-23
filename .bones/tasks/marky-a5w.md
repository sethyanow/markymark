---
id: marky-a5w
title: Refactor oversized zig/src/c_adapter.zig into modular exports
status: open
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

zig/src/c_adapter.zig is >500 lines and continues to grow as BRZA kernels are added. Split by concern (scan exports, utility exports, tests) following existing exports_*.zig pattern to keep file maintainable and reduce merge conflicts.
