---
id: marky-vew
title: Normalize source in structured parsers (json/yaml/toml/jsonl)
status: open
type: task
priority: 3
depends_on: [marky-gnk]
parent: marky-p88
---




## Context

Finding from the 2026-04-20 debugging session (epic marky-p88).

`markymark-parser/src/structured/{json,yaml,toml,jsonl}.rs` each call `tree_sitter::Parser::parse(source, None)` with raw user-provided source and then walk the resulting tree with the same source, passing it to `node.utf8_text(source.as_bytes())`. None of these normalize trailing newlines the way the markdown parser does.

```rust
// json.rs:20
let tree = parser.parse(source, None).ok_or_else(...)?;
// then later walk_object / extract_pair do node.utf8_text(source.as_bytes())
```

The torture harness (`/tmp/mm_torture.py`, 72 cases) exercised these parsers with:
- No trailing newline
- CRLF line endings
- UTF-8 BOM prefix
- Incomplete multibyte UTF-8 at end
- Unclosed strings / braces
- BOM + no trailing newline
- Large and small sizes

None currently panic. BUT the code pattern is identical to marky-prs — any future grammar behavior that reports `end_byte > source.len()` would trigger the same panic. Since marky-gnk introduces a bounds-checked helper, this task becomes the proactive second line of defence: normalize if we know a specific grammar behaves the way tree-sitter-md does.

## Requirements

1. Verify whether `tree-sitter-json`, `tree-sitter-yaml`, `tree-sitter-toml-ng` ever report end_byte past EOF. Most likely they don't — confirm via small-grammar testing.
2. If ANY grammar does, apply the same normalization the markdown parser uses (via `markymark_parser::normalize_block_source` or an equivalent for the structured format).
3. If NONE do, close this task with the finding documented. Don't add unnecessary normalization.
4. Either way: add adversarial tests to each structured parser covering the torture-harness edge cases (empty file, no trailing newline, BOM, CRLF, truncated content, unclosed structures, multibyte at EOF).

## Investigation notes

- Tree-sitter grammars vary in EOF handling. tree-sitter-md specifically needs the trailing newline for block parsing; tree-sitter-json may be cleaner by design.
- Empirical test: parse `b""`, `b"{"`, `b"{\"k\":"`, etc., check `root_node().end_byte()` against `source.len()`.
- `walk_value`, `walk_document`, `extract_pair`, etc. all receive `source: &str` from the top-level `parse_*` fn. If normalization is needed, apply it ONCE at the entry and use the normalized string throughout.

## Success Criteria

- [ ] Empirical verification documented: which grammars (if any) overshoot EOF
- [ ] Fix applied ONLY where needed (don't add dead code)
- [ ] Adversarial test suite for each structured parser mirroring the torture harness
- [ ] All structured-format corner cases (empty, BOM, CRLF, no-trailing-newline, truncated) handled without panic
- [ ] `bazel test //...` green
