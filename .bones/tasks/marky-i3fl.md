---
id: marky-i3fl
title: Fix pre-existing normalizeLabel memory leak in vendored md4c
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-0mr
---


ref_defs.zig normalizeLabel allocates ArrayListUnmanaged buffers for label normalization but never frees them. This is a pre-existing issue from Bun's md4c port (they rely on arena/GC cleanup). Discovered during marky-s02r smoke testing. The wiki link test uses page_allocator as workaround. Fix should add proper cleanup in the Parser.deinit path or use a dedicated arena for temporary label normalization.
