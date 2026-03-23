---
id: marky-nxc
title: 'Code quality review: decompose god files and methods'
status: open
type: epic
priority: 2
depends_on: [marky-eji, marky-aef, marky-3yo, marky-fba, marky-6lc]
---












## Context

Full code quality review of the markymark Rust codebase (2026-03-11). Scanned ~100
production Rust files (~30k lines), excluding tests, benches, fixtures, and worktree
copies. Detection methods: LSP documentSymbol for method lengths and structure, Grep
for pattern scans, manual code reading to classify every hit.

**Methodology:** Anti-slop patterns (premature abstraction, enterprise cosplay, hollow
abstractions, verbose patterns), clean code violations (long methods, deep nesting,
magic numbers, god classes, poor naming), error handling (unwrap/expect in production,
swallowed errors, happy-path-only). Every detection hit was read and classified before
inclusion. False positives were filtered with reasoning.

**Overall health:** The codebase is in good shape — clean error handling via Result
types, no premature abstractions, good naming conventions, no magic numbers. The
findings are concentrated in two files that have grown organically as features were
added. Both are approaching or past the project's size limits.

## Requirements

### P0 — Must fix

1. **realm/mod.rs at 1011 lines breaches 1000-line HARD STOP.**
   `RealmIndex` has 13 fields (12 without `embeddings` feature) and ~40 methods
   spanning cross-doc indexing, search, resolution, journal dates, and lifecycle.
   The retain-or-remove pattern (`entries.retain(..); if entries.is_empty() {
   map.remove(..); }`) is repeated 7 times in `remove_from_cross_doc_indexes`.

2. **engine/mod.rs `execute()` is 559 lines (316-875).**
   Massive command dispatcher. The realm-lookup boilerplate (6 lines: resolve realm key,
   acquire read lock, check existence, return error) is copy-pasted ~15 times. The
   `AddRoot` arm alone is ~120 lines with 4-phase locking logic.

### P1 — Should fix

3. **server.rs at 978 lines — will breach 1000-line limit with any new LSP method.**
   Most mass is in the `LanguageServer` impl (lines 114-930). Three long methods:
   - `references()`: 153 lines (403-556), 5 match arms iterating all realm documents
   - `hover()`: 123 lines (558-681), match on 6 symbol types building markdown inline
   - `did_change()`: 117 lines (180-297), debounce with generation counters

### P2 — Nice to have

4. **Mutex::lock().unwrap() in server.rs** — 8 occurrences with bare `.unwrap()`.
   Replace with `.expect("debounce lock poisoned")` for debuggability.

5. **stack.pop().unwrap() in symbols.rs:203** — Safe due to prior `while let` check
   but invariant isn't documented. Replace with `.expect()`.

## Design

### Phase 1: realm/mod.rs decomposition (P0)

Split `RealmIndex` methods along cohesion seams:

- **realm/cross_doc.rs** — `remove_from_cross_doc_indexes`, `populate_cross_doc_indexes`,
  `patch_headings`, `patch_blocks`, `patch_code_spans`, `patch_stem`, `patch_journal_date`,
  `ensure_tags_clean`. Extract a `retain_or_remove()` helper to deduplicate the 7 repeated
  retain-if-empty blocks.
- **realm/search.rs** — `search_block_text`, `search_key_paths`, `lookup_heading`,
  `lookup_block`, `lookup_code_span`, `tag_counts`, `find_uri_by_stem`,
  `find_uri_by_relative_path`.
- **realm/journal.rs** — `lookup_journal_by_month`, `journal_date`, `detect_journal_date`
  helper.
- **realm/mod.rs** — Struct definition, `new`, `add_document*`, `update_document`,
  `remove_document`, `document_count`, `get_document`, `iter_*`, `semantic_*`, `Default`.

This uses Rust's `impl RealmIndex` split across files pattern (each file adds an `impl`
block on the same struct, re-exported from mod.rs).

### Phase 2: engine/mod.rs `execute()` extraction

- Extract a realm-resolution helper using `RwLockReadGuard::try_map` to eliminate
  the ~16 repeated read-lock + realm-lookup blocks. The lifetime challenge: the read
  guard must outlive the realm reference. `tokio::sync::RwLockReadGuard::try_map`
  returns a `RwLockMappedReadGuard<'_, RealmData>` that holds the lock while
  providing `&RealmData`. This reduces each arm's boilerplate from 6 lines to 2.
  Write-lock arms (CreateRealm, DestroyRealm, AddRoot, RemoveRoot) remain as-is.
- Extract `AddRoot` arm (lines 476-596, 121 lines) into its own async function —
  the 4-phase locking logic is complex enough to warrant extraction.
- Extract `GetContentBlocks` arm (lines 747-819, 73 lines) into its own function
  (inline filter/map logic).
- Leave simple delegating arms in the match (they're just 5-8 lines each).

### Phase 3: server.rs method extraction

- Extract symbol-specific hover builders as private methods on `Backend`:
  `hover_heading()`, `hover_wiki_link()`, `hover_xml_tag()`, `hover_code_span()`,
  `hover_markdown_link()`, `hover_structured_key()`.
- Extract references arms as private methods on `Backend`:
  `references_for_heading()`, `references_for_xml_tag()`,
  `references_for_structured_key()`, `references_for_wiki_link()`.
- Keep all extracted methods in `server.rs` as private methods (not separate files).
  The helpers reference `Backend`'s internal state and the tower-lsp `Client` type —
  splitting into files adds import complexity for limited benefit. Re-evaluate only
  if server.rs still exceeds 800 lines after extraction.
- `did_change()` (117 lines) is **intentionally left as-is**: its complexity is
  interlocking state (debounce handles, generation counters, async spawns), not
  switch-on-type dispatch. Extracting pieces would scatter related state logic.

### Phase 4: Low-severity cleanup (P2)

- Replace 8x `Mutex::lock().unwrap()` with `.expect("debounce lock poisoned")`.
- Replace `stack.pop().unwrap()` with `.expect("stack non-empty after last() check")`.

## Findings Detail

### HIGH Severity

| # | Pattern | Location | Lines | Evidence |
|---|---------|----------|-------|----------|
| 1 | God File | `markymark-index/src/realm/mod.rs` | 1011 | 13 fields (12 w/o embeddings), ~40 methods. `remove_from_cross_doc_indexes` is 100 lines with 7x retain-or-remove. `populate_cross_doc_indexes` is 78 lines mirroring it. |
| 2 | God Method | `markymark-mcp/src/engine/mod.rs:316-875` | 559 | `execute()` dispatcher. Realm-lookup boilerplate (6 lines) repeated ~15 times. `AddRoot` arm is ~120 lines with 4-phase lock protocol. |

### MEDIUM Severity

| # | Pattern | Location | Lines | Evidence |
|---|---------|----------|-------|----------|
| 3 | Long Method | `markymark-lsp/src/server.rs:403-556` | 153 | `references()` — 5 match arms, nested iteration over all realm documents with `resolve_wiki_link` calls. |
| 4 | Long Method | `markymark-lsp/src/server.rs:558-681` | 123 | `hover()` — match on 6 symbol types, XmlTag arm alone is 42 lines of string building. |
| 5 | Long Method | `markymark-lsp/src/server.rs:180-297` | 117 | `did_change()` — debounce with generation counters, spawned async task, multiple mutex acquisitions. |
| 6 | File approaching limit | `markymark-lsp/src/server.rs` | 978 | Will breach 1000-line HARD STOP with any new LSP method. |
| 7 | DRY violation | `markymark-index/src/realm/mod.rs:352-452` | 100 | Retain-or-remove pattern repeated 7 times across 5 index types (headings, blocks, tags, code spans, key paths) plus stem and journal date. |
| 8 | DRY violation | `markymark-mcp/src/engine/mod.rs` | ~90 | Realm-lookup boilerplate (6 lines) copy-pasted ~15 times in `execute()`. |

### LOW Severity

| # | Pattern | Location | Occurrences | Evidence |
|---|---------|----------|-------------|----------|
| 9 | Bare unwrap on Mutex | `markymark-lsp/src/server.rs:171,207,232,265,293,305,941,961,975` | 8 | `self.debounce.lock().unwrap()` — bare unwrap gives no context in panic message. |
| 10 | Bare unwrap on pop | `markymark-lsp/src/symbols.rs:203` | 1 | `stack.pop().unwrap()` safe due to control flow but invariant undocumented. |

### Scanned But Clear

- **Premature Abstraction**: All 7 traits have multiple production impls or are API extension points (`ScanBackend`, `TypedFrontmatter`, `EdgeKind`, `GraphNode`, `CoreEngine`, `EmbeddingProvider`, `ArenaStringExt`).
- **Enterprise Cosplay**: No low-reference Factory/Builder/Strategy/Observer patterns.
- **Hollow Abstractions**: MCP `handle_*` functions add validation and mapping, not pure delegation.
- **Verbose Patterns**: No explicit boolean returns or unnecessary else-after-return.
- **Magic Numbers**: None in production code (all in tests or well-known constants).
- **Poor Naming**: Good — `handle_*` for handlers, descriptive method names throughout.
- **Deep Nesting**: All 4+ indent hits were data initialization or algorithm logic, not control flow.
- **Swallowed Errors**: Rust's type system enforces handling. No bare catches or ignored Results.

## Success Criteria

### Phase 1: realm/mod.rs decomposition
- [x] `realm/mod.rs` contains only: struct definition, constructors (`new`, `new_with_embeddings`), lifecycle methods (`add_document*`, `update_document`, `remove_document`), count/getter methods, iterators, `semantic_*` methods, and `Default` — no search, cross-doc, or journal methods remain
- [x] Cross-doc index methods (`remove_from_cross_doc_indexes`, `populate_cross_doc_indexes`, `patch_*`, `ensure_tags_clean`) extracted to `realm/cross_doc.rs` with a `retain_or_remove` helper deduplicating the 7 inline instances
- [x] Search/lookup methods (`search_block_text`, `search_key_paths`, `lookup_heading`, `lookup_block`, `lookup_code_span`, `tag_counts`, `find_uri_by_stem`, `find_uri_by_relative_path`) extracted to `realm/search.rs`
- [x] Journal methods (`lookup_journal_by_month`, `journal_date`) extracted to `realm/journal.rs` (`detect_journal_date` already in helpers.rs)
- [x] `realm/mod.rs` below 1000-line HARD STOP (the only hard numeric constraint)

### Phase 2: engine/mod.rs execute() extraction
- [x] Read-lock realm-lookup boilerplate eliminated via a `read_realm` helper — each read-lock arm reduced from 6 lines of boilerplate to a `read_realm` call + match
- [x] `AddRoot` arm extracted to a standalone async function
- [x] `GetContentBlocks` arm extracted to a standalone function
- [ ] `execute()` contains only match arms that delegate — no inline business logic longer than ~10 lines

### Phase 3: server.rs method extraction
- [ ] `hover()` delegates to per-symbol-type builder methods (6 builders)
- [ ] `references()` delegates to per-symbol-type helper methods (4+ helpers)
- [ ] `did_change()` intentionally unchanged (complexity is state, not dispatch)

### Phase 4: Low-severity cleanup
- [ ] 8x `Mutex::lock().unwrap()` in server.rs replaced with `.expect("descriptive message")`
- [ ] `stack.pop().unwrap()` in symbols.rs:203 replaced with `.expect("stack non-empty after last() check")`

### Cross-cutting
- [ ] All existing tests pass under default features (`cargo nextest`)
- [ ] All existing tests pass under all features (`cargo nextest --all-features`)
- [ ] Clippy clean across workspace (`cargo clippy --workspace --all-targets`)

## Anti-Patterns

- Do NOT add new abstractions (traits, generics) — this is a decomposition, not a redesign.
- Do NOT change public API signatures — callers should not need updates.
- Do NOT move struct fields or change visibility — only move `impl` blocks.
- Do NOT combine decomposition with feature work — pure refactoring only.
- Do NOT target arbitrary line counts per output file — split along cohesion seams, not line budgets.
- Do NOT move `#[cfg(feature = "embeddings")]` gated methods out of `realm/mod.rs` — they couple
  to the cfg-gated `semantic_index` field and must stay in the same file as the struct definition.
- Do NOT skip the `retain_or_remove` helper extraction — moving the methods to a new file without
  deduplicating the 7 inline instances is the minimum-effort shortcut. The DRY violation IS the
  core deliverable of Phase 1's cross_doc.rs extraction.
- Do NOT do all phases in a single commit — each phase must be a separate commit that compiles and
  passes tests independently. An incomplete multi-phase commit breaks the build for bisect.
- Do NOT test only under default features — the `embeddings` feature flag changes struct layout
  and method availability. Both `cargo nextest` and `cargo nextest --all-features` must pass.

## Key Considerations (SRE Review)

- **Rust `impl` split pattern**: Multiple `impl SameStruct` blocks in different files within
  the same module is idiomatic Rust. Each file uses `use super::*` to access the struct.
  This preserves `pub(crate)` visibility without changes.
- **Test files stay put**: `realm/tests/` (mod tests with submodules: core, incremental,
  code_span, helpers, lazy) should remain as-is. Tests exercise the public API which doesn't
  change. The tests include `#[cfg(feature = "semantic-search")]` gated tests — these verify
  behavior under feature flags and must not be moved or modified.
- **All phases are independent**: Phase 1 (realm, in `markymark-index` crate), Phase 2
  (engine, in `markymark-mcp` crate), and Phase 3 (server, in `markymark-lsp` crate) touch
  different crates. The `impl` split in Phase 1 is invisible to downstream consumers since
  methods remain on `RealmIndex` with unchanged signatures and visibility. No import changes
  needed in engine or server code. Execute phases in any order.
- **`#[cfg(feature = "embeddings")]` gated code**: The `semantic_index` field (line 66) and
  methods `new_with_embeddings`, `semantic_index_arc`, `semantic_search`,
  `detect_semantic_duplicates` are all cfg-gated. Additionally, `new()` has cfg-gated field
  initialization, and `add_document`/`add_documents` have inline cfg blocks. ALL of these
  stay in `realm/mod.rs` — do not move them to extracted files. The methods being extracted
  (cross-doc, search, journal) have no cfg-gating.
- **`retain_or_remove` helper design**: The 7 instances share structure but differ in map type
  (`HashMap<Spur, Vec<T>>` vs `HashMap<String, Vec<T>>` vs `BTreeMap<K, Vec<T>>`), entry type
  (with/without tuple), and lookup key (Spur vs String). The helper needs to be generic over
  the map type or use a simpler approach (e.g., a closure-based helper). Keep it simple — a
  private function, not a trait.
- **`resolve_realm` lifetime design**: The realm-lookup helper in Phase 2 must handle the
  Rust lifetime constraint that `RwLockReadGuard` borrows from `self.state`. Use
  `tokio::sync::RwLockReadGuard::try_map` to produce a `RwLockMappedReadGuard<'_, RealmData>`
  that holds the lock while providing `&RealmData`. Only applies to read-lock arms (~12 arms).
  Write-lock arms (CreateRealm, DestroyRealm, AddRoot, RemoveRoot) remain as-is.
- **`did_change()` is intentionally left as-is in Phase 3**: Its complexity is in interlocking
  state (debounce handles, generation counters, async spawns), not in switch-on-type dispatch.
  Extracting pieces would scatter related state logic across functions without improving
  readability. The method is well-commented with ticket references (T1-1, T2-2, T2-4, T2-8,
  marky-aemm).
- **Existing module structure**: `realm/` already has `helpers.rs` and `types.rs` as separate
  files. The new `cross_doc.rs`, `search.rs`, `journal.rs` follow the established pattern.
  Update `realm/mod.rs` module declarations accordingly (`mod cross_doc; mod search; mod journal;`).

## Log

- [2026-03-23T03:03:51Z] [Seth] SRE refinement complete (13-category review). Key changes: (1) Fixed factual errors: 13 fields not 14, 7 retain-or-remove instances not 8. (2) Corrected Phase 1→3 ordering dependency — all phases independent (different crates). (3) Replaced placeholder return type in read_realm with RwLockReadGuard::try_map design. (4) Resolved Phase 3 'consider' language — keep helpers in server.rs. (5) Rewrote all success criteria from numeric targets to structural criteria. (6) Added cfg(feature=embeddings) as explicit Key Consideration with anti-pattern. (7) Added agent failure mode predictions to Anti-Patterns (skip retain_or_remove helper, single commit, default-features-only testing). (8) Added feature-flag testing criterion (cargo nextest --all-features). (9) Documented retain_or_remove helper design challenge (generic over map types). (10) Documented resolve_realm lifetime design (try_map). Assessment: APPROVE with changes applied.
