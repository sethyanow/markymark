---
id: marky-lkj.2
title: Implement structured parser module with JSON parser (tree-sitter-json)
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---



## Design

## Goal
Create the structured parser module infrastructure and implement the first format parser (JSON via tree-sitter-json). This establishes the pattern all subsequent format parsers will follow.

## Dependencies
- tree-sitter-json 0.19.x (compatible with pinned tree-sitter =0.19.5)
- Types from marky-dr3: DocumentKind, KeyEntry, ValueKind, StructuredAst (in markymark-core)

## Implementation

### 1. Add dependency
- Workspace Cargo.toml: add tree-sitter-json = \"0.19\" to workspace.dependencies
- markymark-parser/Cargo.toml: add tree-sitter-json = { workspace = true }

### 2. Create structured parser module
- markymark-parser/src/structured/mod.rs:
  - pub fn parse_structured(source: &str, kind: DocumentKind) -> Result<StructuredAst, ParserError>
  - Dispatches to format-specific parsers based on DocumentKind
  - Returns error for Markdown kind (handled by existing parser)
  - Returns NotImplemented for formats not yet implemented

- markymark-parser/src/structured/json.rs:
  - pub fn parse_json(source: &str) -> Result<StructuredAst, ParserError>
  - Uses tree-sitter-json to build CST
  - Walks CST recursively, extracting KeyEntry for every key at every depth
  - Produces dotted paths: \"database\", \"database.host\", \"database.port\"
  - Classifies ValueKind from node kind (string_content -> String, number -> Number, etc.)
  - Key ranges and value ranges extracted from tree-sitter node byte positions
  - Converts byte offsets to Position(line, character) using source text

### 3. Tests (TDD)
- test_parse_json_empty_object: {} -> empty keys
- test_parse_json_flat: {\"a\": 1, \"b\": \"x\"} -> 2 entries at depth 0
- test_parse_json_nested: {\"db\": {\"host\": \"localhost\"}} -> depth 0 (db/Object) + depth 1 (host/String)
- test_parse_json_array: {\"items\": [1, 2]} -> depth 0 (items/Array)
- test_parse_json_nested_array_of_objects: {\"servers\": [{\"host\": \"a\"}]} -> servers[0].host
- test_parse_json_value_kinds: verify String, Number, Boolean, Null, Array, Object classification
- test_parse_json_position_accuracy: verify byte-accurate key and value ranges
- test_parse_json_root_keys: StructuredAst.root_keys() returns only depth-0 entries
- test_parse_structured_dispatch_json: parse_structured with DocumentKind::Json delegates correctly
- test_parse_structured_dispatch_markdown_errors: parse_structured with Markdown returns error
- test_parse_structured_dispatch_unimplemented: parse_structured with Yaml returns NotImplemented

### 4. Wire up module
- markymark-parser/src/lib.rs: add pub mod structured;

## Success Criteria
- tree-sitter-json parses JSON and produces accurate KeyEntry items
- Byte-accurate Position ranges for all keys and values
- Nested objects produce dotted paths at correct depths
- Arrays produce indexed paths like items[0], items[1]
- All value kinds correctly classified
- parse_structured dispatches correctly by DocumentKind
- No file exceeds 500 lines
- cargo test / clippy clean
