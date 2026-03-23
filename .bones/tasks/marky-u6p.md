---
id: marky-u6p
title: 'fix(extraction_parity): gate report write behind env var'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

test_extraction_parity_docs_corpus_and_report unconditionally writes docs/benchmarks/extraction-parity.md, breaking test isolation. File: markymark-index/tests/extraction_parity.rs lines 306-496. Fix: only write when MARKYMARK_WRITE_PARITY_REPORT env var is set; default to tempdir.
