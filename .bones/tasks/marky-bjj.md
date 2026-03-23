---
id: marky-bjj
title: 'P0 refactor: split markymark-parser/src/types.rs below 1000 LOC'
status: closed
type: task
priority: 0
owner: sethyanow@users.noreply.github.com
depends_on: [marky-luy]
---


types.rs is 1056 lines (hard-stop threshold exceeded). Split into submodules by domain (element primitives, metadata/frontmatter, links, list/task structures), preserve arena lifetime APIs and existing constructor behavior, and keep tests green. This follow-up is required by learned rule-004 hard stop at 1000 lines.
