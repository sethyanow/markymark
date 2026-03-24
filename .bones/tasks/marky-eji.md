---
id: marky-eji
title: 'Phase 1: Decompose realm/mod.rs into cohesion-based submodules'
status: closed
type: task
priority: 2
parent: marky-nxc
---






## Context

`markymark-index/src/realm/mod.rs` is 1011 lines — breaching the 1000-line HARD STOP.
`RealmIndex` has 13 fields and ~40 methods spanning cross-doc indexing, search/lookup,
journal dates, and lifecycle. Split along cohesion seams into 3 new submodule files
using Rust's multi-file `impl` pattern. Pure decomposition — no behavioral changes.

**Blocked by:** Nothing (first task in epic)
**Unlocks:** Phase 2 (engine/mod.rs) and Phase 3 (server.rs) — all phases are
independent since they touch different crates, but this is the P0 priority.

**Existing module structure:** `realm/` already has `helpers.rs`, `types.rs`, and
`tests/` as submodules. The new files follow the established pattern. Each existing
submodule uses its own `use` statements (not `use super::*`).

## Requirements

1. Extract cross-doc index methods to `realm/cross_doc.rs` with a `retain_or_remove`
   helper deduplicating the 7 inline retain-if-empty instances.
2. Extract search/lookup methods to `realm/search.rs`.
3. Extract journal methods to `realm/journal.rs`.
4. `realm/mod.rs` retains only: struct definition, `DocContribution`, `intern_stem`,
   constructors, lifecycle methods (`add_document*`, `update_document`, `remove_document`),
   count/getter methods, iterators, `semantic_*` methods, and `Default`.
5. All `#[cfg(feature = "embeddings")]` gated code stays in mod.rs.
6. No behavioral changes — existing tests must pass under all feature flags.

## Design

### Method assignment (verified via LSP documentSymbol)

**Stay in mod.rs (~420 lines):**
- Lines 31-73: `RealmIndex` struct definition (13 fields)
- Line 75: `intern_stem()` standalone helper
- Lines 84-130: `DocContribution` struct + impl
- `new()`, `new_with_embeddings()` [cfg-gated]
- `add_document()`, `add_documents()`, `add_document_structural()`, `add_structured_document()`
- `update_document()`, `remove_document()`
- `document_count()`, `markdown_count()`, `structured_count()`, `interner_len()`, `key_path_count()`
- `get_document()`, `get_any_document()`, `get_structured_document()`
- `iter_documents()`, `iter_all_documents()`, `iter_structured_documents()`
- `semantic_index_arc()`, `semantic_search()`, `detect_semantic_duplicates()` [all cfg-gated]
- `impl Default`

**Move to cross_doc.rs (~365 lines):**
- `ensure_tags_clean()` (line 333)
- `remove_from_cross_doc_indexes()` (line 352, 100 lines — 7x retain-or-remove)
- `populate_cross_doc_indexes()` (line 454, 78 lines)
- `patch_headings()` (line 532)
- `patch_blocks()` (line 578)
- `patch_code_spans()` (line 609)
- `patch_stem()` (line 645)
- `patch_journal_date()` (line 668)

**Move to search.rs (~190 lines):**
- `lookup_heading()` (line 733)
- `lookup_block()` (line 742)
- `lookup_code_span()` (line 750)
- `tag_counts()` (line 759, handles `tags_dirty` by computing from contributions)
- `find_uri_by_stem()` (line 825)
- `find_uri_by_relative_path()` (line 835, calls `helpers::resolve_relative_path`)
- `search_key_paths()` (line 902)
- `search_block_text()` (line 923, 63 lines)

**Move to journal.rs (~25 lines):**
- `lookup_journal_by_month()` (line 986)
- `journal_date()` (line 997)

### retain_or_remove helper

There are 12 total instances of the `get_mut → retain → remove-if-empty` pattern across
all cross-doc methods: 7 in `remove_from_cross_doc_indexes` + 5 in patch methods
(patch_headings L542, patch_blocks L587, patch_code_spans L618, patch_stem L652,
patch_journal_date L681). All 12 should use the helper.

They differ in map type (HashMap×6, BTreeMap×1 in remove; HashMap×4, BTreeMap×1 in
patches) and value type (Vec<(DocumentUri, T)> vs Vec<DocumentUri>). Design a private
standalone function — not a trait. The executing agent decides the exact generic design.

### Key codebase facts

- `detect_journal_date` is in `helpers.rs:26` (NOT in mod.rs) — no move needed.
  cross_doc.rs imports it via `super::helpers::detect_journal_date`.
- `resolve_relative_path` is in `helpers.rs:6` — used only by `find_uri_by_relative_path`
  (moving to search.rs). search.rs imports via `super::helpers::resolve_relative_path`.
- `tag_counts()` has special logic: when `tags_dirty` is true (on `&self` methods), it
  computes directly from `contributions` rather than reading `tag_to_docs`. This method
  accesses both `self.tag_to_docs` and `self.contributions` — verify the moved version
  still compiles with correct field access.
- `update_document()` stays in mod.rs and calls `remove_from_cross_doc_indexes()` and
  `populate_cross_doc_indexes()` — both in cross_doc.rs. This works because they're
  all `impl RealmIndex` methods, visible across files within the same module.

## Implementation

### Step 1: Baseline verification
- Run `cargo nextest -p markymark-index` — confirm GREEN, record test count
- `wc -l markymark-index/src/realm/mod.rs` — confirm 1011

### Step 2: Extract journal.rs (smallest — establish pattern)
- Create `markymark-index/src/realm/journal.rs`
- Add own imports: `super::RealmIndex`, `markymark_core::DocumentUri`
- Add `impl RealmIndex` block with `lookup_journal_by_month` and `journal_date`
- Add `mod journal;` to mod.rs (after `mod types;` line)
- Remove the 2 methods from mod.rs impl block
- Run: `cargo check -p markymark-index` — expect clean

### Step 3: Extract search.rs (8 methods)
- Create `markymark-index/src/realm/search.rs`
- Add own imports: `lasso::Spur`, `std::collections::HashMap`,
  `super::helpers::resolve_relative_path`, `super::types::*`,
  `super::{RealmIndex, DocContribution}`, `markymark_core::prelude::*`,
  `markymark_core::structured::ValueKind`, `markymark_core::DocumentUri`,
  `crate::document::BlockKind`
- Add `impl RealmIndex` block with all 8 search/lookup methods
- Add `mod search;` to mod.rs
- Remove the 8 methods from mod.rs impl block
- Run: `cargo check -p markymark-index` — expect clean

### Step 4: Extract cross_doc.rs (8 methods + retain_or_remove helper)
- Create `markymark-index/src/realm/cross_doc.rs`
- Add own imports: `lasso::{Rodeo, Spur}`, `std::collections::{HashMap, HashSet, BTreeMap}`,
  `super::{RealmIndex, DocContribution, intern_stem}`,
  `super::helpers::detect_journal_date`, `super::types::*`,
  `markymark_core::prelude::*`, `markymark_core::DocumentUri`,
  `crate::document::DocumentIndex`, `crate::structured_document::StructuredDocumentIndex`
- Add `impl RealmIndex` block with all 8 cross-doc methods
- Create `retain_or_remove` helper — deduplicate the 7 inline instances
- Add `mod cross_doc;` to mod.rs
- Remove the 8 methods from mod.rs impl block
- Run: `cargo check -p markymark-index` — expect clean

### Step 5: Clean up mod.rs imports
- `use helpers::{detect_journal_date, resolve_relative_path}` → remove `resolve_relative_path`
  (only caller was `find_uri_by_relative_path`, now in search.rs). Keep `detect_journal_date`
  (used in `DocContribution::build` L119).
- Remove `use markymark_core::structured::ValueKind;` (only used in `search_key_paths`, now
  in search.rs).
- `HashSet` STAYS — used by `DocContribution` struct fields (L88-91).
- `BTreeMap` STAYS — used by `date_to_docs` field (L62).
- Run: `cargo check -p markymark-index` — clean, no warnings

### Step 6: Full verification
- `wc -l markymark-index/src/realm/mod.rs` — below 1000 (expected ~420)
- `wc -l markymark-index/src/realm/{cross_doc,search,journal}.rs` — sanity check
- `cargo nextest -p markymark-index` — all tests pass, same count as Step 1
- `cargo nextest -p markymark-index --all-features` — all tests pass with embeddings
- `cargo clippy -p markymark-index --all-targets` — clean
- Commit and push

## Success Criteria

- [x] `realm/mod.rs` contains only struct definition, `DocContribution`, `intern_stem`,
      constructors, lifecycle methods, counts/getters, iterators, `semantic_*`, and `Default`
- [x] `realm/cross_doc.rs` contains `ensure_tags_clean`, `remove_from_cross_doc_indexes`,
      `populate_cross_doc_indexes`, and all 5 `patch_*` methods
- [x] `retain_or_remove` helper in cross_doc.rs deduplicates all 12 inline instances (7 in remove_from_cross_doc_indexes + 5 in patch methods)
- [x] `realm/search.rs` contains all 8 search/lookup methods
- [x] `realm/journal.rs` contains `lookup_journal_by_month` and `journal_date`
- [x] `realm/mod.rs` below 1000-line HARD STOP
- [x] All `#[cfg(feature = "embeddings")]` code remains in mod.rs
- [x] `cargo nextest -p markymark-index` passes (same test count as baseline)
- [x] `cargo nextest -p markymark-index --all-features` passes
- [x] `cargo clippy -p markymark-index --all-targets` clean

## Anti-Patterns

- Do NOT use `use super::*` in new files — follow types.rs pattern with explicit imports.
- Do NOT move `#[cfg(feature = "embeddings")]` gated methods out of mod.rs.
- Do NOT change any method signatures, visibility, or return types.
- Do NOT move struct fields or the struct definition.
- Do NOT add traits or new public API — `retain_or_remove` is a private helper.
- Do NOT skip the `retain_or_remove` helper — deduplicating the 7 instances is a core
  deliverable, not optional cleanup.
- Do NOT combine this with any feature work — pure refactoring.
- Do NOT move `detect_journal_date` — it's already in helpers.rs where it belongs.
- Do NOT do all extractions in one giant uncommitted change — verify `cargo check`
  after each file extraction to catch import/visibility issues incrementally.

## Key Considerations

- **TDD escape hatch applies:** This is a pure structural refactoring (no logic changes).
  Existing tests ARE the regression tests. No new tests needed — but verify the same
  test count passes before and after.
- **`intern_stem` is a standalone function** (line 75), not a method on RealmIndex. It stays
  in mod.rs. cross_doc.rs imports it via `super::intern_stem`.
- **`DocContribution` stays in mod.rs** — it's tightly coupled to the struct (used in
  constructors, update_document, and all patch methods). Moving it would fragment the
  struct's core type definitions.
- **Import precision matters:** Each new file needs only the imports its methods actually
  use. Don't cargo-cult all of mod.rs's imports into every new file. Let `cargo check`
  tell you what's missing.

## Log

- [2026-03-23T03:12:58Z] [Seth] Task scoped from marky-nxc Phase 1 during executing-plans flow. Full codebase verification via LSP: method assignments confirmed, detect_journal_date already in helpers.rs (not mod.rs as epic assumed), resolve_relative_path used only by find_uri_by_relative_path. Import patterns verified via types.rs (explicit use, not super::*). Expected ~420 lines remaining in mod.rs after all extractions.
- [2026-03-23T03:21:50Z] [Seth] SRE refinement (fresh session). 13 categories reviewed. APPROVE with 3 corrections: (1) retain_or_remove count is 12 not 7 — patch methods have 5 more identical instances. Updated design + success criterion. (2) Step 5 import cleanup was wrong: HashSet STAYS (DocContribution fields), resolve_relative_path goes, ValueKind goes. detect_journal_date stays (DocContribution::build L119). (3) Category 6 adversarial-planning deferred — pure structural refactoring with no new logic. All claims verified via LSP: line count 1011, 13 fields, all method locations confirmed, helpers.rs contents confirmed, import pattern confirmed.
- [2026-03-23T03:52:22Z] [Seth] Debrief: prelude::* needed #[allow(unused_imports)] for test compilation (tests use super::super::*). retain_or_remove as two functions (hash+btree), cross-doc methods changed to pub(super) for cross-file visibility. Reflections: prelude::* removal breaking tests was the one surprise — not caught by SRE (test infra dependency). Skeleton accuracy good — SRE caught the HashSet/import errors pre-execution. No user corrections. Memory: no new memories needed — all findings are in-code or one-time refactoring patterns.
