---
id: marky-9s9
title: Fix build.rs rerun-if-changed to watch individual Zig files
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

CodeRabbit found that cargo:rerun-if-changed on a directory only triggers when directory metadata changes (add/remove files), not when files are modified. This contradicts dec-brza-een-002. Need to add walkdir as build dependency and enumerate individual .zig files in markymark-kernels/build.rs lines 35-40. Update decision record after fix.
