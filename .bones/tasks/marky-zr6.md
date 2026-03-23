---
id: marky-zr6
title: Extract references() per-symbol-type helpers; relocate standalone functions to helpers.rs
status: open
type: task
priority: 2
parent: marky-nxc
---



## Context

`markymark-lsp/src/server.rs` `references()` method (L404-557, 154 lines) is a dispatch-on-type
method that matches on 4 `SymbolAtPosition` variants and iterates documents to find references.
The file is at 993 lines after hover extraction (marky-3gi).

Extracting references() arms in-place would add ~22 lines net → 1015 lines, breaching the
1000-line HARD STOP. The 6 standalone hover functions added in marky-3gi also live at the bottom
of server.rs (L981-993) and don't depend on Backend — they should relocate too.

**Solution:** Extract 4 references arms into standalone functions AND relocate all 10 standalone
functions (6 hover + 4 references) to `helpers.rs`. This file already exists for exactly this
purpose (its doc comment: "extracted from server.rs to keep it under 1000 lines"), already
imports `Location`, `ServerState`, `StructuredKeyInfo`, `DocumentUri`, and already defines
`iter_realm_documents` and `xml_hover_stats` which the functions call.

**Blocked by:** marky-3gi (closed — hover extraction)
**Unlocks:** Phase 3 criterion "references() delegates to per-symbol-type helper methods (4+ helpers)"

## Requirements

1. Extract Heading arm (L424-443, 20 lines) into `fn references_for_heading(state: &ServerState, heading: &HeadingEntry<'_>) -> Vec<Location>`.
2. Extract XmlTag arm (L444-458, 15 lines) into `fn references_for_xml_tag(state: &ServerState, doc_uri: &DocumentUri, xt: &XmlTagEntry<'_>, include_declaration: bool) -> Vec<Location>`.
3. Extract StructuredKey arm (L460-495, 36 lines) into `fn references_for_structured_key(state: &ServerState, doc_uri: &DocumentUri, info: &StructuredKeyInfo, include_declaration: bool) -> Vec<Location>`.
4. Extract WikiLink arm (L496-548, 53 lines) into `fn references_for_wiki_link(state: &ServerState, doc_uri: &DocumentUri, wl: &WikiLinkEntry<'_>, include_declaration: bool) -> Option<Vec<Location>>`. Returns `Option` because non-KeyPath resolutions return `None` (early exit).
5. All 4 functions are standalone (not Backend methods) — none use `self`.
6. references() match body becomes 4 delegation calls + catch-all early return.
7. Relocate 6 existing hover functions (hover_heading, hover_wiki_link, hover_markdown_link, hover_xml_tag, hover_code_span, hover_structured_key) from server.rs L981+ to helpers.rs.
8. All 10 functions live in helpers.rs as `pub(crate)`.
9. server.rs remains under 1000 lines. helpers.rs remains under 500 lines.
10. No behavioral changes — existing tests must pass.

## Design

### Current references() structure (L404-557)

- **Preamble** (L404-419, 16 lines): Extract URI, position, state read lock, symbol_at_position, include_declaration. Stays in references().
- **Mutable vec** (L421): `let mut locations = Vec::new()` — removed; each function creates its own.
- **Match dispatch** (L423-550): 4 arms + catch-all — this gets extracted.
- **Postamble** (L552-556, 5 lines): Return `Some(locations)` or `None`. Stays in references().

### Key difference from hover() extraction

hover() arms returned `String` (functional). references() arms push to a mutable `Vec<Location>`.
After extraction, each function creates its own `Vec<Location>` and returns it. The WikiLink arm
additionally returns `Option<Vec<Location>>` because non-KeyPath resolutions trigger an early
`return Ok(None)` from the outer function.

### After extraction, references() match becomes

```rust
let locations = match symbol {
    SymbolAtPosition::Heading(ref h) => references_for_heading(&state, h),
    SymbolAtPosition::XmlTag(ref xt) => {
        references_for_xml_tag(&state, &doc_uri, xt, include_declaration)
    }
    SymbolAtPosition::StructuredKey(ref info) => {
        references_for_structured_key(&state, &doc_uri, info, include_declaration)
    }
    SymbolAtPosition::WikiLink(ref wl) => {
        match references_for_wiki_link(&state, &doc_uri, wl, include_declaration) {
            Some(locs) => locs,
            None => return Ok(None),
        }
    }
    _ => return Ok(None),
};
```

### Dependencies used by the arms

- **Heading** → `iter_realm_documents(state)`, `crate::convert::to_lsp_location`
- **XmlTag** → `iter_realm_documents(state)`, `crate::convert::to_lsp_location`
- **StructuredKey** → `iter_realm_documents(state)`, `resolve_wiki_link`, `ResolvedTarget::KeyPath`, `state.get_structured_document_index`, `crate::convert::to_lsp_location`
- **WikiLink** → `resolve_wiki_link`, `ResolvedTarget::KeyPath`, `iter_realm_documents(state)`, `crate::convert::to_lsp_location`

All use `crate::convert::to_lsp_location` and `iter_realm_documents` — both accessible from helpers.rs.

### helpers.rs import additions needed

```rust
use markymark_index::resolution::resolve_wiki_link;
use markymark_index::{CodeSpanEntry, HeadingEntry, MarkdownLinkEntry, WikiLinkEntry, XmlTagEntry};
```

`crate::convert::to_lsp_location` is used as a fully-qualified path, no import needed.

### Line count projection

- server.rs: 993 - 105 (hover functions removed) - 113 (references match shrunk) = ~775 lines
- helpers.rs: 110 + 105 (hover functions) + 136 (references functions) + 5 (imports) = ~356 lines

## Implementation

### Step 1: Baseline — run markymark-lsp tests via cargo MCP, confirm GREEN
### Step 2: Move 6 hover functions from server.rs to helpers.rs
- Cut hover_heading, hover_wiki_link, hover_markdown_link, hover_xml_tag, hover_code_span, hover_structured_key from server.rs (after L979)
- Add to helpers.rs as `pub(crate)` functions (they were private in server.rs)
- Add entry type imports to helpers.rs: `HeadingEntry`, `WikiLinkEntry`, `MarkdownLinkEntry`, `XmlTagEntry`, `CodeSpanEntry` from `markymark_index`
- Update server.rs: add `use crate::helpers::{hover_heading, hover_wiki_link, hover_markdown_link, hover_xml_tag, hover_code_span, hover_structured_key};` to imports
- Remove unused entry type imports from server.rs (they're now used in helpers.rs, unless references functions in Step 3 still need them — check after Step 3)
- Cargo check
### Step 3: Write 4 references functions in helpers.rs
- `pub(crate) fn references_for_heading(state: &ServerState, heading: &HeadingEntry<'_>) -> Vec<Location>`
- `pub(crate) fn references_for_xml_tag(state: &ServerState, doc_uri: &DocumentUri, xt: &XmlTagEntry<'_>, include_declaration: bool) -> Vec<Location>`
- `pub(crate) fn references_for_structured_key(state: &ServerState, doc_uri: &DocumentUri, info: &StructuredKeyInfo, include_declaration: bool) -> Vec<Location>`
- `pub(crate) fn references_for_wiki_link(state: &ServerState, doc_uri: &DocumentUri, wl: &WikiLinkEntry<'_>, include_declaration: bool) -> Option<Vec<Location>>`
- Add `resolve_wiki_link` import to helpers.rs
- Move each arm's body intact into the corresponding function, adjusting: `&state` → `state`, `&doc_uri` → `doc_uri` (already references)
- Cargo check
### Step 4: Replace references() match body with delegation calls
- Remove `let mut locations = Vec::new();` line
- Replace match arms with delegation calls (see Design section above)
- Update server.rs imports: add references function imports from helpers
- Cargo check
### Step 5: Verify line counts
- `wc -l` on server.rs — must be under 1000
- `wc -l` on helpers.rs — must be under 500
### Step 6: Full verification — cargo test (markymark-lsp), cargo clippy

## Success Criteria

- [ ] 4 standalone references helper functions exist in helpers.rs: `references_for_heading`, `references_for_xml_tag`, `references_for_structured_key`, `references_for_wiki_link`
- [ ] references() match body is delegation calls (no inline business logic longer than ~5 lines)
- [ ] 6 hover functions relocated from server.rs to helpers.rs
- [ ] server.rs remains under 1000 lines
- [ ] helpers.rs remains under 500 lines
- [ ] All markymark-lsp tests pass
- [ ] Clippy clean

## Anti-Patterns

- Do NOT leave hover functions in server.rs — the line count requires relocation.
- Do NOT make the functions methods on `Backend` — none use `self`.
- Do NOT change the references response structure or the early-return behavior for WikiLink/catch-all arms.
- Do NOT change `resolve_wiki_link`, `iter_realm_documents`, or `to_lsp_location`.
- Do NOT place functions inside the `#[cfg(test)]` impl Backend block.
- Do NOT add new abstractions, traits, or generics.
- Do NOT merge the relocation and extraction into a single step — relocate first (Step 2), then extract (Steps 3-4). This keeps each step independently verifiable.

## Key Considerations

- **Lifetime on extracted functions**: Same pattern as hover extraction — entry types need `<'_>` lifetime annotation. Return types are owned (`Vec<Location>`, `Option<Vec<Location>>`) so no lifetime flows out.
- **WikiLink arm returns Option**: The non-KeyPath branch does `return Ok(None)` in the current code. The extracted function returns `Option<Vec<Location>>` — `None` means "no results, exit early," `Some(vec)` means "found these locations." The caller matches on the Option.
- **`iter_realm_documents` already in helpers.rs**: No need to import — the references functions can call it directly as a sibling function.
- **`crate::convert::to_lsp_location` used as fully-qualified path**: All arms use `crate::convert::to_lsp_location(uri, range)`. Keep the fully-qualified path in helpers.rs (consistent with existing code that already uses `crate::` paths).
- **`StructuredKeyInfo` already imported in helpers.rs**: Line 8 — `use crate::state::{ServerState, StructuredKeyInfo};`.
- **Relocation makes hover functions `pub(crate)`**: They were private (`fn`) in server.rs. In helpers.rs they need `pub(crate)` visibility for server.rs to call them.
- **Entry type imports may move**: After Step 2, server.rs might still need entry types if they're used in pattern destructuring (e.g., `SymbolAtPosition::Heading(h)` doesn't require HeadingEntry import). Verify after Step 3 — if server.rs no longer uses them, remove the import line.
