---
id: marky-b1q
title: 'PR #16 review: triage and fix all review comments'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
---

Triage and address all review comments from PR #16 (multi-format structured document support).

## Critical (must fix)

1. **json5.rs escape sequence handling in read_quoted_string** (Copilot + CodeRabbit)
   - read_quoted_string does not decode escape sequences
   - Key "foo\nbar" scanned as literal foo\nbar but serde map has decoded key with actual newline
   - map.get(&key_text) returns None, key silently treated as null via unwrap_or(Null)
   - Need to handle: \\", \\\, \\n, \\r, \\t, \\b, \\f, \\uXXXX
   - No tests cover escaped key names -- add regression tests
   - File: markymark-parser/src/structured/json5.rs around line 147

## Major (should fix)

2. **jsonl.rs range offsets broken for indented lines** (CodeRabbit)
   - parse_jsonl parses trimmed but offset_range only adds line_byte_offset
   - For lines with leading whitespace, key/value ranges shifted left
   - Fix: parse original line instead of trimmed, or add leading-whitespace offset
   - File: markymark-parser/src/structured/jsonl.rs around line 85

3. **successes.json duplicate ID suc-017** (CodeRabbit)
   - Two entries share ID suc-017 (perf pattern vs batch parser impl)
   - Renumber second entry to suc-017b or next available
   - File: .claude-harness/memory/procedural/successes.json

## Minor (should fix)

4. **json5.rs dead let _ = i; statement** (Copilot)
   - No-op, remove it. Line 360.

5. **yaml.rs byte_to_position duplicated across 6 parsers** (Copilot)
   - Identical helper in json.rs, json5.rs, jsonl.rs, yaml.rs, toml.rs, flat.rs
   - Extract to shared utility (structured/util.rs or markymark-core)

6. **resolution.rs doc comment missing key-path mention** (CodeRabbit)
   - Wiki-link resolution now falls back to structured key paths
   - Doc comment still implies only headings
   - File: markymark-index/src/resolution.rs around line 103

7. **flat.rs value ranges end early for quoted values** (CodeRabbit)
   - After stripping quotes, val_end_byte uses unquoted length
   - val_start_byte still points to opening quote
   - Use raw_value.len() for range calculation
   - File: markymark-parser/src/structured/flat.rs around line 112

8. **mod.rs JSONC dispatch test uses plain JSON** (CodeRabbit)
   - test_parse_structured_dispatch_jsonc should use JSONC-specific syntax (comments)
   - tree-sitter-json 0.24.8 does NOT support trailing commas
   - File: markymark-parser/src/structured/mod.rs around line 90

## Approach

- Write failing tests first for each fix (TDD)
- Address critical items first, then major, then minor
- Items 4-8 are quick wins, can batch together
- Item 5 (DRY extraction) is the largest refactor
