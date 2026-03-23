---
id: marky-cqx
title: Implement YAML parser (tree-sitter-yaml)
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-dr3, marky-lkj.2]
parent: marky-lkj
---




## Design

## Goal
Implement YAML parser using tree-sitter-yaml 0.7.2 following the proven JSON parser pattern. Enables .yaml and .yml file indexing with byte-accurate key path extraction.

## Effort Estimate
6-8 hours (single session, similar to JSON parser marky-lkj.2)

## Context
- marky-dr3: DocumentKind enum and core types in place
- marky-lkj.2: JSON parser establishes CST walker pattern (REFERENCE IMPLEMENTATION)
- dec-035: Tree-sitter 0.26 migration enables tree-sitter-yaml (^0.25.4 compatible)
- Approach: Unified tree-sitter paradigm for JSON/YAML/TOML

## Dependencies
- tree-sitter-yaml 0.7.2 (compatible with tree-sitter 0.26 via tree-sitter-language ^0.1)
- Types from marky-dr3: DocumentKind, KeyEntry, ValueKind, StructuredAst

## Implementation

### 1. Add dependency
- Workspace Cargo.toml: add tree-sitter-yaml = "0.7" to workspace.dependencies
- markymark-parser/Cargo.toml: add tree-sitter-yaml = { workspace = true }

### 2. Create YAML parser module
- markymark-parser/src/structured/yaml.rs:
  - pub fn parse_yaml(source: &str) -> Result<StructuredAst, ParserError>
  - Use tree-sitter-yaml grammar (tree_sitter_yaml::LANGUAGE)
  - Walk CST recursively extracting KeyEntry for every key at every depth
  - Handle YAML-specific nodes: block_mapping, block_sequence, flow_mapping, flow_sequence
  - Produce dotted paths: "database", "database.host", "servers[0].name"
  - Classify ValueKind from node kind (string_scalar -> String, integer_scalar -> Number, etc.)
  - Extract byte-accurate ranges from tree-sitter nodes
  - Convert byte offsets to Position(line, character)
  - **REFERENCE:** Follow markymark-parser/src/structured/json.rs pattern

### 3. Update dispatch
- markymark-parser/src/structured/mod.rs:
  - Add DocumentKind::Yaml => yaml::parse_yaml(source) to parse_structured dispatch

### 4. Tests (TDD - write RED tests first)
- test_parse_yaml_empty_document: "" -> StructuredAst{keys: vec![]}
- test_parse_yaml_flat: "key: value" -> 1 KeyEntry at depth 0
- test_parse_yaml_nested: "database:\n  host: localhost" -> 2 entries (database at depth 0, database.host at depth 1)
- test_parse_yaml_block_sequence: "items:\n  - a\n  - b" -> items[0], items[1]
- test_parse_yaml_flow_sequence: "items: [a, b]" -> items[0], items[1]
- test_parse_yaml_nested_sequence: "servers:\n  - host: a" -> servers[0].host at depth 2
- test_parse_yaml_value_kinds_string: "key: value" -> ValueKind::String
- test_parse_yaml_value_kinds_number: "port: 8080" -> ValueKind::Number
- test_parse_yaml_value_kinds_boolean: "enabled: true" -> ValueKind::Boolean
- test_parse_yaml_value_kinds_null: "value: null" -> ValueKind::Null
- test_parse_yaml_value_kinds_tilde: "value: ~" -> ValueKind::Null (YAML null variant)
- test_parse_yaml_value_kinds_array: "items: [1, 2]" -> ValueKind::Array
- test_parse_yaml_value_kinds_object: "db: {host: x}" -> ValueKind::Object
- test_parse_yaml_position_accuracy: Verify key_range.start == Position(line: 0, character: 0) for first key
- test_parse_yaml_multiline_string_pipe: "text: |\n  line1\n  line2" -> ValueKind::String, preserves newlines
- test_parse_yaml_multiline_string_gt: "text: >\n  line1\n  line2" -> ValueKind::String, folds to single line
- test_parse_yaml_anchors_aliases: "a: &x value\nb: *x" -> both keys indexed with their values (no special anchor handling)
- test_parse_yaml_merge_keys: "base: &b {x: 1}\nderived: {<<: *b, y: 2}" -> derived.x and derived.y both indexed
- test_parse_yaml_malformed_syntax: "key:\n value" -> returns Err(ParserError) (indentation error)
- test_parse_yaml_tab_indentation: "key:\n\tvalue" -> returns Err(ParserError) (tabs forbidden)
- test_parse_yaml_deep_nesting: 200-level nested object -> all keys indexed without stack overflow
- test_parse_yaml_large_document: 10,000-key document -> completes within 1 second
- test_parse_yaml_unicode_keys: "日本語: value" -> key extracted correctly
- test_parse_yaml_unicode_values: "key: 🎉" -> value extracted correctly
- test_parse_structured_dispatch_yaml: parse_structured(source, DocumentKind::Yaml) -> delegates to parse_yaml

## Success Criteria
- [ ] test_parse_yaml_empty_document passes (verifies: tree-sitter-yaml handles empty)
- [ ] test_parse_yaml_flat passes (verifies: basic key extraction)
- [ ] test_parse_yaml_nested passes (verifies: dotted paths at correct depths)
- [ ] test_parse_yaml_block_sequence passes (verifies: array indexing [0], [1])
- [ ] test_parse_yaml_flow_sequence passes (verifies: flow syntax support)
- [ ] test_parse_yaml_value_kinds_* tests pass (verifies: all 6 ValueKind variants classified correctly)
- [ ] test_parse_yaml_position_accuracy passes (verifies: byte-accurate ranges)
- [ ] test_parse_yaml_multiline_string_* tests pass (verifies: block scalar handling)
- [ ] test_parse_yaml_merge_keys passes (verifies: YAML << operator support)
- [ ] test_parse_yaml_malformed_syntax passes (verifies: error handling)
- [ ] test_parse_yaml_tab_indentation passes (verifies: YAML spec compliance)
- [ ] test_parse_yaml_deep_nesting passes (verifies: no stack overflow)
- [ ] test_parse_yaml_large_document passes (verifies: performance <1s)
- [ ] test_parse_yaml_unicode_* tests pass (verifies: UTF-8 support)
- [ ] test_parse_structured_dispatch_yaml passes (verifies: integration)
- [ ] All 25+ tests pass: cargo test --package markymark-parser yaml
- [ ] No file exceeds 500 lines: wc -l yaml.rs (should be 200-350 lines following json.rs ~280 line pattern)
- [ ] cargo test --workspace passes (zero regression)
- [ ] cargo clippy --workspace --all-targets clean (no warnings)
- [ ] Pre-commit hooks passing

## Anti-Patterns (FORBIDDEN)
- NO yaml-rust2 MarkedEventReceiver approach (superseded by tree-sitter-yaml per dec-035)
- NO serde_yaml (drops position info)
- NO approximate position tracking (must use tree-sitter node byte ranges)
- NO YAML-specific code in transport layers (parser details stay in markymark-parser)
- NO modifying DocumentIndex (use StructuredAst output type)
- NO unwrap/expect in production code (use pattern matching or ?)
- NO TODO comments without issue numbers (complete implementation or file issue)
- NO unimplemented! or todo! stubs (all code paths must be real)
- NO regex for YAML parsing (use tree-sitter grammar)

## Key Considerations (ADDED BY SRE REVIEW)

**YAML Complexity vs JSON**:
- YAML is significantly more complex than JSON
- Whitespace-sensitive (indentation defines structure)
- Multiple syntaxes (block vs flow)
- Special features (anchors, aliases, merge keys, multi-document)
- REFERENCE: Study json.rs first, then adapt for YAML-specific nodes

**Edge Case: Malformed YAML**:
- tree-sitter-yaml will produce ERROR nodes for invalid syntax
- MUST check for ERROR nodes in CST and return ParserError
- Test: "key:\n value" (wrong indentation) should error
- Test: "key:\n\tvalue" (tabs forbidden) should error
- Do NOT silently ignore parse errors - return Err(ParserError)

**Edge Case: Tab Characters**:
- YAML 1.2 spec explicitly forbids tabs for indentation
- tree-sitter-yaml should produce ERROR nodes for tabs
- MUST test: "key:\n\tvalue" -> Err(ParserError)
- Production code that violates spec should not parse successfully

**Edge Case: Deep Nesting**:
- YAML can be arbitrarily nested
- CST walker is recursive - risk of stack overflow
- MUST test: 200+ level nesting to verify no stack overflow
- If stack overflow occurs, consider iterative traversal or depth limit

**Edge Case: Multi-Document YAML**:
- YAML supports --- separators for multiple documents in one file
- For v1: Only parse first document, ignore rest
- Add comment in code: "TODO(marky-XXX): Multi-document support"
- File issue if this becomes user-requested feature

**Edge Case: Merge Keys (<<)**:
- YAML << operator merges mappings: {<<: *base, override: x}
- tree-sitter-yaml parses this as merge_key node
- MUST index both merged keys AND override keys
- Test: verify derived.x and derived.y both appear after merge

**Edge Case: Anchors and Aliases**:
- YAML &anchor and *alias create references
- For v1: Index both anchor and alias as separate keys with same value
- No deduplication - if *alias used 5 times, index 5 times
- This matches user expectation (find all locations where value appears)

**Edge Case: Unicode**:
- YAML supports full UTF-8 in keys and values
- tree-sitter provides byte ranges (not char ranges)
- MUST convert to Position(line, character) correctly for multi-byte chars
- Test: "日本語: 🎉" to verify UTF-8 handling
- REFERENCE: json.rs already handles this - use same byte_offset_to_position logic

**Edge Case: Large Documents**:
- Some projects have huge YAML configs (10,000+ keys)
- CST traversal is O(nodes), should be fast
- Test: 10,000-key document should parse in <1 second
- If performance issue, profile and optimize hotspots

**Performance: Node Kind Matching**:
- tree-sitter node.kind() returns &str, not enum
- String matching is slower than enum matching
- Consider: Cache node kind strings or use match on &str slices
- REFERENCE: json.rs uses match node.kind() - follow same pattern

**Reference Implementation**:
- markymark-parser/src/structured/json.rs (~280 lines)
- Uses tree-sitter-json grammar
- Recursive CST walker extracting KeyEntry
- Byte range -> Position conversion
- FOLLOW THIS PATTERN for YAML implementation
- Key differences: YAML has block/flow variants, anchors, merge keys

**YAML Spec Compliance**:
- tree-sitter-yaml implements YAML 1.2
- YAML 1.2 differs from YAML 1.1 (true/false/null are only booleans/null)
- This is GOOD - fewer ambiguous cases
- Document which YAML version supported in module docstring
