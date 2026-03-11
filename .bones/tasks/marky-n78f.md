---
id: marky-n78f
title: 'Task 4: LSP integration — replace incremental scan with engine pipeline'
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-2n4u]
parent: marky-io3h
---



## Design

## Goal

Wire DocumentEngine into ServerState so every markdown did_open/did_change calls
engine.update(text) + get_blob() + from_blob() instead of the current
build_markdown_index_with_old_tree() incremental pipeline.

Epic: marky-io3h (Task 4 of 4). This is the final integration step.

## Context

Tasks 1-3 complete:
- Task 1 (marky-6jzs): Zig DocumentEngine struct + blob serialization
- Task 2 (marky-atsp): FFI exports + Rust DocumentEngine wrapper (markymark-kernels/src/engine.rs)
- Task 3 (marky-2n4u): DocumentIndex::from_blob() constructor (markymark-index/src/document/from_blob.rs, feature=zig-kernels)

Current state of markymark-lsp:
- state/mod.rs (446 lines): ServerState has parser, md_trees, pending_edits for incremental reparse
- incremental/ module (824 lines source + 1322 lines tests): 5-extractor incremental merge logic
- Cargo.toml: depends on markymark-index but NOT markymark-kernels (no DocumentEngine access)

## Implementation Steps

### Step 1: Update Cargo.toml (causes compile failure — first RED signal)
Add to markymark-lsp/Cargo.toml dependencies:
  markymark-kernels = { version = "0.4.0", path = "../markymark-kernels" }
Change markymark-index dep to add zig-kernels feature:
  markymark-index = { version = "0.4.0", path = "../markymark-index", features = ["zig-kernels"] }

Run: cargo build -p markymark-lsp
Expect: compile error about missing DocumentEngine because state/mod.rs not yet updated.
This is the RED signal. If it compiles cleanly, something is wrong.

### Step 2: Write tests that verify BEHAVIORAL requirements (these are regression guards)
In markymark-lsp/tests/state_tests.rs, add these tests. They should PASS currently
(via from_scan) and must continue to pass after migration. They serve as parity guards.

- test_engine_parity_headings: Open doc with 3 headings via engine path. Assert heading
  slugs match exactly what from_scan produces for the same text. Use a fixed text with
  known headings ("# Foo\n## Bar\n### Baz") and hardcode expected slugs.

- test_engine_parity_wiki_links: Open doc with wiki links. Assert wiki_links() count and
  target strings match from_scan output for identical text.

- test_engine_parity_tags: Open doc with tags (#tag1, #tag2). Assert tags() output matches.

- test_engine_parity_block_ids: Open doc with block IDs (^block1). Assert block_by_id works.

- test_engine_fallback_index_populated: This test verifies behavior when engine succeeds.
  For the fallback path, we rely on functional tests of from_scan.

Note: These tests currently pass via from_scan. After migration, they must still pass via
from_blob. If any fail after migration, it means from_blob parity is broken.

### Step 3: Modify ServerState struct (in markymark-lsp/src/state/mod.rs)

Remove these fields from ServerState:
  parser: Parser
  md_trees: HashMap<String, MarkdownTree>
  pending_edits: Vec<InputEdit>

Add this field:
  engines: HashMap<String, DocumentEngine>
  (type is markymark_kernels::engine::DocumentEngine)

Update ServerState::new():
  Remove: parser: Parser::new().expect(...), md_trees: HashMap::new(), pending_edits: Vec::new()
  Add: engines: HashMap::new()

Remove these imports from top of state/mod.rs (they become unused):
  use markymark_parser::{byte_to_point, InputEdit, MarkdownTree, Parser};
  use crate::incremental::{self, incremental_byte_bounds};

Add this import:
  use markymark_kernels::engine::DocumentEngine;
  use markymark_index::BlobError;  (for fallback error logging)

### Step 4: Implement engine-based markdown index builder

Add private method to ServerState impl block:

```rust
fn build_markdown_index_via_engine(&mut self, uri_str: &str, text: &str) -> DocumentIndex {
    // LIFETIME NOTE: ScanBlob<'_> borrows &engine. from_blob() copies all text
    // into its own bumpalo arena, so the DocumentIndex does NOT hold references
    // to the blob after from_blob() returns. The blob can safely drop.
    
    // Case 1: Engine already exists for this URI — update it
    if let Some(engine) = self.engines.get_mut(uri_str) {
        match engine.update(text) {
            Ok(()) => {
                match engine.get_blob() {
                    Ok(blob) => match DocumentIndex::from_blob(blob.data()) {
                        Ok(index) => return index,
                        Err(e) => {
                            eprintln!("markymark-lsp: from_blob failed for {uri_str}: {e:?}, falling back to from_scan");
                        }
                    },
                    Err(e) => {
                        eprintln!("markymark-lsp: get_blob failed for {uri_str}: {e:?}, falling back to from_scan");
                    }
                }
            }
            Err(e) => {
                eprintln!("markymark-lsp: engine update failed for {uri_str}: {e:?}, falling back to from_scan");
            }
        }
    } else {
        // Case 2: No engine for this URI — create one
        match DocumentEngine::new(text) {
            Ok(mut engine) => {
                match engine.get_blob() {
                    Ok(blob) => match DocumentIndex::from_blob(blob.data()) {
                        Ok(index) => {
                            self.engines.insert(uri_str.to_string(), engine);
                            return index;
                        }
                        Err(e) => {
                            eprintln!("markymark-lsp: from_blob failed (new engine) for {uri_str}: {e:?}, falling back to from_scan");
                        }
                    },
                    Err(e) => {
                        eprintln!("markymark-lsp: get_blob failed (new engine) for {uri_str}: {e:?}, falling back to from_scan");
                    }
                }
            }
            Err(e) => {
                eprintln!("markymark-lsp: engine create failed for {uri_str}: {e:?}, falling back to from_scan");
            }
        }
    }
    
    // Fallback: from_scan (from_scan does NOT require zig-kernels feature gate)
    // This preserves backward compat per epic anti-patterns.
    let (index, _) = crate::incremental::build_markdown_index_with_old_tree(
        // WAIT: incremental module is being deleted. Use fallback via markymark_index directly.
        // See implementation note below — keep build_markdown_index() method until Step 6.
        // In Step 6 we update this fallback.
    );
    index
}
```

IMPLEMENTATION NOTE for fallback: Until Step 6 deletes the incremental module, keep the
existing build_markdown_index() private helper and call it from the fallback path. In Step 6,
replace it with a direct call to DocumentIndex::from_scan() or markymark_parser+scan path.
Consult markymark-kernels/src/scan.rs for the from_scan() API.

### Step 5: Replace build_markdown_index calls

open_document (around line 125):
  Old: let (index, md_tree) = self.build_markdown_index(&text);
       if let Some(tree) = md_tree { self.md_trees.insert(uri_str, tree); }
  New: let index = self.build_markdown_index_via_engine(uri.as_str(), &text);

change_document (around line 148):
  Old: let (index, md_tree) = self.build_markdown_index(&text);
       ...md_trees insert/remove...
       self.pending_edits.clear()
  New: let index = self.build_markdown_index_via_engine(uri.as_str(), &text);

apply_document_changes — Phase 1 (text editing, ~line 185):
  KEEP: The text editing loop (Full/Incremental change application)
  KEEP: The bounds validation (bounds.end_before_start check and error log)
  REMOVE: All InputEdit construction and pending_edits.push()
  REMOVE: All old_tree manipulation (old_tree.edit(), md_trees.remove/insert)
  REMOVE: All old-data capture block (old_wiki_links, old_blocks, old_markdown_links, old_xml_tags)
          — lines 192–248 — delete the entire if let Some(index) = self.realm.get_document(uri) block

apply_document_changes — Phase 2 (re-index):
  Old: build_markdown_index_with_old_tree(...) with all the incremental args
  New: let index = self.build_markdown_index_via_engine(uri_str, &final_text);

close_document (around line 380):
  Add: self.engines.remove(uri.as_str());
  Remove: self.md_trees.remove(uri.as_str());
  Remove: self.pending_edits.clear();

### Step 6: Remove dead methods and module

Remove these methods from ServerState impl:
  build_markdown_index() — replaced by build_markdown_index_via_engine
  build_markdown_index_with_old_tree() — deleted
  get_md_tree() — no longer needed
  pending_edit_count() — check first: grep -rn "pending_edit_count" markymark-lsp/
    If unused outside state/mod.rs: delete. If used in tests: update tests to remove usage.

Update fallback in build_markdown_index_via_engine to call DocumentIndex::from_scan() directly:
  Use: markymark_parser::parse() + DocumentIndex::from_scan()
  Look at existing build_markdown_index() implementation to copy the scan chain.

Delete entire markymark-lsp/src/incremental/ directory:
  rm -rf markymark-lsp/src/incremental/
  Remove line "pub mod incremental;" or "mod incremental;" from markymark-lsp/src/lib.rs

### Step 7: Update tests

Grep for all usages of deleted API in tests:
  grep -rn "get_md_tree\|pending_edit_count\|incremental" markymark-lsp/tests/

Remove test functions (they test deleted implementation, not behavior):
  - test_markdown_tree_stored_after_open
  - test_md_tree_retained_after_change (and similar md_tree tests, ~6 tests)
  - All test_incremental_* tests in state_tests.rs (~16 tests)
  - assert_incremental_matches_full helper function

Keep: all other tests in state_tests.rs (they test heading/link/block behavior, not internals)
Verify: The Step 2 parity tests now pass GREEN.
Add: test_engine_created_and_destroyed_lifecycle — verify engine lifecycle
  via document_count() and behavior: open → headings correct, close → index gone.

### Step 8: Run full test suite (verification)

# Full workspace
cargo nextest --workspace

# LSP crate only (faster iteration)  
cargo nextest -p markymark-lsp

# Lint
cargo clippy --workspace --all-targets

# Build check
cargo build --workspace

## Success Criteria

- [ ] cargo nextest --workspace passes (all tests green) — run in release if needed
- [ ] cargo clippy --workspace --all-targets — zero warnings
- [ ] git diff shows: incremental/ directory gone, state/mod.rs simplified
- [ ] ServerState struct has no parser, md_trees, or pending_edits fields (verify via grep)
- [ ] Parity tests pass: from_blob headings/links/tags/block_ids match from_scan for same text
- [ ] Engine lifecycle: engine exists after open_document, is gone after close_document (via behavior: open→correct index, open+close→no index)
- [ ] Error fallback verified: from_scan fallback produces a non-empty index (verifiable via eprintln output in test)
- [ ] apply_document_changes: no InputEdit construction, no old-data capture block

## Key Considerations (SRE REVIEW ADDITIONS)

**URI Key Consistency**:
- `documents`, `realm`, and new `engines` maps all key by `uri.as_str().to_string()`
- MUST verify: open_document inserts into engines with same key that close_document removes
- Mismatch = engine leak (grows unboundedly as vault grows)
- Verify: grep for all engines.insert() and engines.remove() and confirm key format matches

**Engine Absent During apply_document_changes**:
- apply_document_changes can be called without a prior open_document (race in some clients)
- In the engine path: if `engines.get_mut(uri_str)` is None, fall through to the "create new" branch
- The "create new" branch in build_markdown_index_via_engine handles this correctly
- BUT: also ensure realm.remove_document() is safe when no document existed (verify it's a no-op)

**ScanBlob Lifetime — Key Pattern**:
- engine.get_blob() returns ScanBlob<'_> borrowing &engine
- from_blob(blob.data()) copies ALL text into its own bumpalo arena
- Therefore: blob can safely drop after from_blob() returns, even if engine is later mutated
- This is why the approach of: get_blob → from_blob → drop blob → store engine works correctly
- DO NOT try to store ScanBlob or pass it across awaits

**Phase 1 Cleanup in apply_document_changes**:
- The bounds check (bounds.end_before_start) MUST be kept
- The clamping warning (start_clamped || end_clamped) MUST be kept
- These validate user input from the LSP client — do not remove them
- Only remove the InputEdit construction and old-tree manipulation below the check

**Fallback to from_scan**:
- The fallback must use a DIRECT call to the scan pipeline, not the old incremental module
- After Step 6 deletes incremental/, the fallback can use:
    markymark_parser::parse(text, None) + DocumentIndex::from_scan(&scan_result)
  OR use markymark_kernels::scan functions directly
- Look at the current build_markdown_index() implementation to extract the scan call chain
- The fallback MUST NOT panic or return an empty DocumentIndex — log and recover

**No unwrap()/expect() in new code**:
- build_markdown_index_via_engine must not use unwrap/expect anywhere
- All engine/blob errors go through the fallback path with eprintln! logging
- Panic = LSP server crash = bad user experience

## Anti-Patterns (from epic + SRE review)

- ❌ Do NOT remove from_scan() or from_ast() — they must stay for MCP/batch use
- ❌ Do NOT add incremental merge logic to the engine path — Zig does full rebuild
- ❌ Do NOT panic if engine fails — fallback to from_scan instead
- ❌ Do NOT put tree-sitter inside the engine path — tree-sitter stays separate
- ❌ No unwrap()/expect() in build_markdown_index_via_engine or fallback path
- ❌ Do NOT use different key formats for engines vs documents vs md_trees (URI key leak)
- ❌ Do NOT silently drop from the index if both engine AND from_scan fail — log clearly
- ❌ Do NOT delete bounds checking from apply_document_changes Phase 1 (client input validation)
