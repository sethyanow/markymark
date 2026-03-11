---
id: marky-z9z
title: Improve markdown link resolution beyond stem-only
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

PR #35 review (T2-4): Stem-only resolution for markdown links causes false positives when docs in different directories share the same filename. For links with directory segments, attempt path-relative resolution first, fall back to stem-only.
