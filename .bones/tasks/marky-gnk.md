---
id: marky-gnk
title: Bounds-checked utf8_text helper + audit all node.utf8_text call sites
status: open
type: task
priority: 2
depends_on: [marky-4g3]
parent: marky-p88
---





## Context

Finding from the 2026-04-20 debugging session (epic marky-p88).

`tree_sitter::Node::utf8_text` at `binding_rust/lib.rs:2010` is implemented as:

```rust
pub fn utf8_text<'a>(&self, source: &'a [u8]) -> Result<&'a str, str::Utf8Error> {
    str::from_utf8(&source[self.start_byte()..self.end_byte()])
}
```

The `&source[start..end]` slice has no bounds check. If `end > source.len()` the call panics, which is how marky-prs manifested. The `Result` type only covers UTF-8 validity — not out-of-range.

The marky-prs fix removed the known source mismatch, but **ten** call sites across the workspace still use this API with user-provided source. Any future position mismatch (new parser added, grammar upgrade, etc.) will panic again:

- `markymark-parser/src/ast.rs:240` — try_logseq_heading
- `markymark-parser/src/types/elements.rs:72` — heading level detection
- `markymark-parser/src/types/elements.rs:108` — list item child text
- `markymark-parser/src/types/elements.rs:166` — paragraph text
- `markymark-parser/src/types/elements.rs:213` — blockquote text
- `markymark-index/src/document/from_engine.rs:113` — is_logseq_heading (fixed via normalization, not defensive)
- `markymark-parser/src/structured/json.rs:86` — JSON key text
- `markymark-parser/src/structured/yaml.rs:118` — YAML value text
- `markymark-parser/src/structured/toml.rs:55` — TOML key text
- `markymark-parser/src/structured/toml.rs:59` — TOML value text

Defence-in-depth: a bounds-checked helper eliminates this class of panic entirely, regardless of which caller has a position mismatch.

## Requirements

1. Add a helper in `markymark-core` or `markymark-parser`:
   ```rust
   pub fn node_text_checked<'a>(node: tree_sitter::Node, source: &'a str) -> Option<&'a str> {
       let start = node.start_byte();
       let end = node.end_byte();
       source.as_bytes().get(start..end)
           .and_then(|bytes| std::str::from_utf8(bytes).ok())
   }
   ```
2. Replace all 10 `node.utf8_text(source.as_bytes())` call sites with `node_text_checked(node, source).unwrap_or("")` or appropriate error propagation.
3. Where callers currently use `.ok()?`, preserve the early-return behaviour.
4. Where callers use `.unwrap_or("")` or `.unwrap_or("").trim()`, preserve the default.
5. Where callers use `.map_err(...)` (e.g. `elements.rs:166`), return a `CoreError::Message` on out-of-range with the same semantics.

## Related latent bug

`markymark-index/src/document/mod.rs:139 block_text`:

```rust
pub fn block_text(&self, block: &ContentBlock<'_>) -> &str {
    let source = &self.cell.borrow_owner().source_text;
    source.get(block.start_byte..block.end_byte).unwrap_or("")
}
```

Same pattern — `.get(...)` is safe but silently returns `""` on OOB. Marky-prs closed the known OOB source, but this still masks future position bugs. Consider either:
- Accepting the `unwrap_or("")` as the defensive contract (document it)
- Returning `Option<&str>` and forcing callers to handle the OOB case explicitly

## Success Criteria

- [ ] `node_text_checked` helper exists and is public
- [ ] All 10 call sites use the helper (no direct `node.utf8_text` in markymark code)
- [ ] `bazel test //...` still green
- [ ] Regression test: passing a synthetic out-of-range node to each call site does not panic
- [ ] `block_text()` behaviour documented or changed to `Option<&str>`
