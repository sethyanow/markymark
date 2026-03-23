---
id: marky-0xh
title: 'P2 refactor: split markymark-lsp/src/incremental/tests.rs below 1000 LOC (currently 1084)'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

incremental/tests.rs is 1084 lines — just over the 1000-line hard stop. Split by extractor type (headings, wiki_links, blocks, xml_tags, markdown_links) into separate test files under incremental/tests/. Each resulting file should be under 500 lines. Pattern: create submodule directory, move test groups, verify cargo nextest -p markymark-lsp passes after each move.
