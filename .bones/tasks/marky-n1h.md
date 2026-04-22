---
id: marky-n1h
title: 'Refactor markymark-mcp/src/engine/tests/mod.rs: split into submodules (1413 lines > 1000 rule)'
status: open
type: task
priority: 0
---

## Context

CLAUDE.md rule #2 (1000-line HARD STOP): `markymark-mcp/src/engine/tests/mod.rs`
is 1413 lines as of commit `14735dce` (marky-v6c). It was already ~1150 lines
pre-v6c; the v6c adversarial + regression suite grew it to 1413.

This file already uses the submodule pattern for some tests — at lines 27–36
it declares `concurrency`, `curation`, `enrich`, `export_docs_index`,
`recommend`, `preview_profiling`. Other tests are inlined directly in `mod.rs`.
Follow the existing pattern: extract inlined tests into topic-named files
under `markymark-mcp/src/engine/tests/`.

## Requirements

1. No test removed, no assertion weakened — this is pure code motion.
2. Every `#[tokio::test]`-style or `#[test]` function currently inlined in
   `mod.rs` finds a topic-appropriate submodule. New submodules go in
   `markymark-mcp/src/engine/tests/<topic>.rs` and get `mod <topic>;`
   declared in `mod.rs`.
3. After the split, `mod.rs` must be under 1000 lines (the rule's threshold).
   No invented sub-target — just satisfy the rule.
4. `cargo test -p markymark-mcp --lib` and `bazel test //markymark-mcp:...`
   must remain green.

## Investigation notes

- Cohesion seams to extract (by topic, based on 2026-04-22 inspection):
  - `collect_documents_*` tests (the v6c additions) → `workspace_scan.rs`
  - `outline_*` / `get_outline_*` → `outline.rs`
  - `rename_*` → `rename.rs`
  - `find_references_*` → `find_references.rs`
  - `search_symbols_*` → `search_symbols.rs`
  - `engine_*` / `batch_indexed_*` → `engine_indexing.rs`
  - `export_index_*` → already under `export_docs_index/` submodule — check
    whether the remaining `export_index_*` inlined tests should move there.
  - `fnv1a32_*` + `hash_embedding_provider_*` (semantic-search feature) →
    `hash_embedding.rs` (keep `#[cfg(feature = "semantic-search")]`)
  - `from_text_equivalence_*` → `from_text_equivalence.rs`
  - `lto_eliminates_fault_injection` → probably its own tiny file, or grouped
    with any sibling LTO assertions.

- Shared fixtures (`make_temp_realm_dir`, `make_engine_with_custom_realm`)
  stay in `mod.rs` so all submodules can `use super::*;`. Or extract them to
  `common.rs` — either pattern works; follow what existing submodules do.

## Anti-Patterns

- **Do NOT** change any assertion. Code motion only.
- **Do NOT** rename test functions. Search/triage history references them.
- **Do NOT** shrink `mod.rs` by deleting tests.
- **Do NOT** inline the existing `concurrency`, `curation`, `enrich`, etc.
  submodules back into `mod.rs` — keep the pattern consistent.

## Success Criteria

- [ ] `wc -l markymark-mcp/src/engine/tests/mod.rs` < 1000
- [ ] No test function deleted (diff shows pure moves only)
- [ ] `cargo test -p markymark-mcp --lib` green
- [ ] `bazel test //markymark-mcp:markymark-mcp_test` green
- [ ] `bazel test //...` green
