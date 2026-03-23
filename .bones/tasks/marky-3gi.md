---
id: marky-3gi
title: Extract hover() per-symbol-type builder methods in server.rs
status: open
type: task
priority: 2
parent: marky-nxc
---



## Context

`markymark-lsp/src/server.rs` `hover()` method (L558-681, 124 lines) is a dispatch-on-type
method that matches on 6 `SymbolAtPosition` variants and builds markdown hover text inline.
The file is at 978 lines — approaching the 1000-line HARD STOP.

Extract each non-trivial match arm body into a private method on `Backend` in the second
`impl Backend` block (L931+). The hover() method retains the preamble (parameter extraction,
state read, symbol_at_position lookup) and postamble (wrapping markdown in Hover response),
and each match arm becomes a delegation call.

**Blocked by:** marky-iqg (closed — SemanticSearch extraction, Phase 2 complete)
**Unlocks:** Phase 3 criterion "hover() delegates to per-symbol-type builder methods (6 builders)"

## Requirements

1. Extract WikiLink arm (L579-608, 30 lines) into `fn hover_wiki_link(state: &ServerState, doc_uri: &DocumentUri, wl: &WikiLinkEntry) -> String`.
2. Extract XmlTag arm (L612-654, 43 lines) into `fn hover_xml_tag(state: &ServerState, xt: &XmlTagEntry) -> String`.
3. Extract CodeSpan arm (L656-669, 14 lines) into `fn hover_code_span(state: &ServerState, cs: &CodeSpanEntry) -> String`.
4. Extract Heading arm (L575-578, 4 lines) into `fn hover_heading(h: &HeadingEntry) -> String`.
5. Extract MarkdownLink arm (L609-611, 3 lines) into `fn hover_markdown_link(ml: &MarkdownLinkEntry) -> String`.
6. StructuredKey arm (L671, 1 line) already delegates to `structured_key_hover_markdown(info)` — wrap in `fn hover_structured_key(info: &StructuredKeyInfo) -> String` for consistency with the other 5 builders.
7. All 6 extracted functions are standalone functions (not methods on `Backend`) since none use `self` — they only need `&ServerState` (for realm access), `&DocumentUri` (WikiLink only), and the variant-specific entry type.
8. hover() match body becomes 6 one-line delegation calls.
9. No behavioral changes — existing tests must pass.

## Design

### Current hover() structure (L558-681)

- **Preamble** (L558-572, 15 lines): Extract URI, position, acquire state read lock, lookup `symbol_at_position`. Stays in hover().
- **Match dispatch** (L574-672): 6 arms producing `String` — this is what gets extracted.
- **Postamble** (L674-681, 8 lines): Wrap markdown in `Hover { contents: HoverContents::Markup(...) }`. Stays in hover().

### Function placement

All 6 functions are standalone private functions placed AFTER the second `impl Backend` block
(after L973). They are NOT methods on `Backend` because none of them use `self` — they only
need `&ServerState`, `&DocumentUri`, and variant data. Making them standalone avoids false
coupling to Backend.

### Dependencies used by the arms

- **WikiLink** → `resolve_wiki_link(state.realm(), &doc_uri, wl.target, wl.heading)` from `markymark_index::resolution`; `ResolvedTarget` variants
- **XmlTag** → `xml_hover_stats(&state, xt.tag_name)` from `crate::helpers`
- **CodeSpan** → `state.realm().lookup_code_span(cs.text)` via `RealmIndex`
- **Heading, MarkdownLink** → no external dependencies, pure formatting
- **StructuredKey** → `structured_key_hover_markdown(info)` from `crate::helpers` (already delegating)

### After extraction, hover() becomes

```rust
async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    let uri_str = &params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    let core_pos = crate::convert::from_lsp_position(pos);

    let state = self.state.read().await;
    let doc_uri = match crate::convert::from_lsp_uri(uri_str) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };

    let symbol = match state.symbol_at_position(&doc_uri, core_pos) {
        Some(s) => s,
        None => return Ok(None),
    };

    let markdown = match &symbol {
        SymbolAtPosition::Heading(h) => hover_heading(h),
        SymbolAtPosition::WikiLink(wl) => hover_wiki_link(&state, &doc_uri, wl),
        SymbolAtPosition::MarkdownLink(ml) => hover_markdown_link(ml),
        SymbolAtPosition::XmlTag(xt) => hover_xml_tag(&state, xt),
        SymbolAtPosition::CodeSpan(cs) => hover_code_span(&state, cs),
        SymbolAtPosition::StructuredKey(ref info) => hover_structured_key(info),
    };

    Ok(Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown,
        }),
        range: None,
    }))
}
```

## Implementation

### Step 1: Baseline — run markymark-lsp tests via cargo MCP, confirm GREEN
### Step 2: Write the 6 standalone functions after the second `impl Backend` block
- Place after L973 (after `document_generations_count` method)
- Each function takes only what it needs: `&ServerState`, `&DocumentUri`, variant entry type
- Move each arm's body intact into the corresponding function
- Functions return `String`
- Cargo check after each function (or batch-write all 6, then cargo check)
### Step 3: Replace hover() match arm bodies with delegation calls
- Each arm becomes a one-line call: `hover_heading(h)`, `hover_wiki_link(&state, &doc_uri, wl)`, etc.
- Cargo check
### Step 4: Verify server.rs line count
- `wc -l` on server.rs — should remain under 1000 lines (net change is near-zero: methods moved, not removed)
- If over 1000 lines, escalate — do NOT proceed
### Step 5: Full verification — cargo test (markymark-lsp), cargo clippy

## Success Criteria

- [ ] 6 standalone hover builder functions exist in server.rs: `hover_heading`, `hover_wiki_link`, `hover_markdown_link`, `hover_xml_tag`, `hover_code_span`, `hover_structured_key`
- [ ] hover() match body is 6 one-line delegation calls (no inline business logic)
- [ ] server.rs remains under 1000 lines
- [ ] All markymark-lsp tests pass
- [ ] Clippy clean

## Anti-Patterns

- Do NOT move the functions to a separate file — the epic Design explicitly says "Keep all extracted methods in server.rs" since they reference Backend internals and tower-lsp types.
- Do NOT make them methods on `Backend` — none of them use `self`. Standalone functions with explicit parameters are clearer.
- Do NOT change the hover response structure (HoverContents, MarkupKind, range).
- Do NOT change the `resolve_wiki_link`, `xml_hover_stats`, or `structured_key_hover_markdown` functions.
- Do NOT add new abstractions, traits, or generics.
- Do NOT refactor the preamble or postamble — only the match arm bodies move.

## Key Considerations

- **Lifetime on extracted functions**: `WikiLinkEntry<'a>`, `HeadingEntry<'a>`, etc. borrow from the `DocumentIndex` inside the read guard. The extracted functions receive references to these entries — the lifetime is bound to the state read guard scope in hover(). This is fine since the functions are called within that scope and return owned `String`.
- **`resolve_wiki_link` import**: Currently used inline at L580. After extraction, `hover_wiki_link` uses it — the import already exists at file scope.
- **Line count risk**: server.rs is at 978 lines. Extracting methods adds ~6 function signatures + doc comments but the match arm bodies are already counted. Net change should be approximately +30 lines (signatures, blank lines). If this pushes past 1000, the executing agent must STOP and escalate.
- **Test coverage**: hover behavior is tested via LSP integration tests in `markymark-cli/tests/lsp_methods.rs` and `markymark-lsp/tests/`. No unit tests exist for hover directly — the refactoring is validated by the integration test suite.
