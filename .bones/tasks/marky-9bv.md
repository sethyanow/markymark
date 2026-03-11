---
id: marky-9bv
title: 'fix(semantic): skip empty headings instead of aborting document indexing'
status: closed
type: bug
priority: 2
owner: sethyanow@users.noreply.github.com
---

add_document propagates EmbedError with ? for every heading. HashEmbeddingProvider rejects empty/whitespace text. A single empty heading (e.g. # with no title) causes entire document semantic indexing to fail. Fix: skip empty/whitespace headings with continue.
