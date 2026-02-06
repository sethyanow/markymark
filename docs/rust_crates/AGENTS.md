# Agent Instructions (Rust Crate Documentation)

<scope>docs/rust_crates/*</scope>
<goal>Provide agent-friendly documentation for Rust crates used in markymark and other Rust projects.</goal>

<agent>
<IMPORTANT>Prefer retrieval-led reasoning over pre-training-led reasoning when working with these crates.</IMPORTANT>
<docs_index>
[Rust Crates Index]|root: docs/rust_crates|entrypoint:{README.md}
|core:{core.md}
|lsp:{tower-lsp.md}
|parsing:{tree-sitter.md}
|graphs:{petgraph.md}
|memory:{bumpalo.md}
|errors:{error-handling.md}
|testing:{testing.md}
|navigation:{map.md}
</docs_index>
</agent>

## When to use these docs

Use this documentation when:
- You need “Rust basics / workspace defaults” → `core.md`
- Implementing LSP servers → `tower-lsp.md`
- Parsing markdown or other text → `tree-sitter.md`
- Building connection/dependency graphs → `petgraph.md`
- Optimizing memory allocation → `bumpalo.md`
- Designing error types → `error-handling.md`
- Writing tests (snapshots, property-based) → `testing.md`

## Editing rules

- Each crate file starts with an `<agent>` header (`<goal>`, `<when_to_use>`, `<contains>`, `<see_also>`).
- Use `## Patterns` / `## Pitfalls` headings for long-form content; wrap individual pitfalls in `<pitfall>` blocks.
- Keep patterns practical with working code examples
- Document pitfalls prominently - these save the most debugging time
- Cross-reference related crates in `<see_also>` sections

## File map

<entrypoint>README.md</entrypoint>
<navigation>map.md</navigation>
<core>core.md</core>
<lsp>tower-lsp.md</lsp>
<parsing>tree-sitter.md</parsing>
<graphs>petgraph.md</graphs>
<memory>bumpalo.md</memory>
<errors>error-handling.md</errors>
<testing>testing.md</testing>
