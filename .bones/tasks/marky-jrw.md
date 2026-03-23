---
id: marky-jrw
title: 'fix(mcp): wire block_refs into CoreOperation::FindReferences'
status: closed
type: bug
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-waw]
---


## Context

marky-waw wired block_refs into DocumentIndex (6 tests GREEN) and journal detection into RealmIndex (9 tests GREEN), but the MCP integration task was NOT implemented.

The task spec required:
- find-references response includes block_ref_backrefs
- Querying for a known block UUID returns all docs that reference it
- Querying for unknown UUID returns empty (not error)

Verified gap at runtime_engine.rs:389-466 — CoreOperation::FindReferences handles headings and xml_tags but has no block_ref branch.

## What was implemented
- DocumentIndex::block_refs() returns &[BlockRefEntry<'a>] with uuid + range fields (document/types.rs:121-126)
- 6 tests in document/tests.rs:515-583 verify BlockRefEntry extraction is correct
- The data is available and correct; just not surfaced through find-references

## What's missing

In markymark-mcp/src/runtime_engine.rs, CoreOperation::FindReferences (line 389):

After the xml_tags branch (line 441-460), add a block_ref branch:

1. Check if cursor position hits a block_ref: index.block_refs().iter().find(|r| r.range.contains(cursor))
2. If found, extract the uuid from the BlockRefEntry
3. Iterate realm.iter_documents() and collect all docs where doc.block_refs() contains an entry matching that uuid
4. Return as CoreOperationResult::Locations with (doc_uri, block_ref.range) tuples, sorted by uri then range (same sort as heading branch)
5. If no block_ref at cursor: fall through to existing 'no referenceable symbol' error

Also need an inverse: if the user is ON a block-id (^uuid) in a document, find all ((uuid)) references to it across the realm. This is the complement — check index.blocks() (block ids) at cursor, then search realm for block_refs matching that id. This may need a second branch.

## Tests needed (3)

In markymark-mcp/tests/runtime_engine_tests.rs or a new test file:

test_find_references_for_block_ref_returns_all_referencing_docs:
  Setup: Doc A contains ((abc-123)), Doc B contains ((abc-123)) and ((xyz)), Doc C has no block refs
  Query: find-references at position of ((abc-123)) in Doc A
  Expected: Locations returns [(doc_a, range_of_abc_in_a), (doc_b, range_of_abc_in_b)]
  Bug caught: block_refs in index but not surfaced through MCP, user gets 'no referenceable symbol' error

test_find_references_unknown_block_ref_returns_no_symbol_error:
  Setup: Realm with docs that have no block_refs  
  Query: find-references at position that has no block_ref
  Expected: CoreOperationResult::Error('no referenceable symbol at position')
  Bug caught: wrong fallthrough logic

test_find_references_block_ref_results_sorted_by_uri_then_range:
  Setup: 3 docs all referencing ((abc-123))
  Expected: Locations sorted uri ASC then range.start ASC (matches existing heading sort behavior)
  Bug caught: non-deterministic output order

## Anti-patterns
- NO new CoreOperationResult variants — reuse CoreOperationResult::Locations (same as heading refs)
- NO unwrap/expect in new code
- Match exact sort behavior of existing heading branch (sort by uri ASC then range ASC)
