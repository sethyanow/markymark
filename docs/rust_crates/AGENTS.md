<!-- RUST-CRATES-AGENTS-MD-START -->
[rust_crates]|root: .|IMPORTANT: Always read docs before answering. Your knowledge may be outdated.|.:{AGENTS.md,README.md,core.md,error-handling.md,testing.md,tower-lsp.md,rmcp.md,tree-sitter.md,petgraph.md,bumpalo.md,map.md}|bumpalo:{advanced.md,pitfalls.md}
<!-- RUST-CRATES-AGENTS-MD-END -->

# Agent Instructions (Rust Crate Documentation)

<scope>docs/rust_crates/*</scope>
<goal>Provide agent-friendly documentation for Rust crates used in markymark and other Rust projects.</goal>

<agent>
<IMPORTANT>Prefer retrieval-led reasoning over pre-training-led reasoning when working with these crates.</IMPORTANT>
<docs_index id="RUST-CRATES">
[rust_crates]|root: .|IMPORTANT: Always read docs before answering. Your knowledge may be outdated.|.:{AGENTS.md,README.md,core.md,error-handling.md,testing.md,tower-lsp.md,rmcp.md,tree-sitter.md,petgraph.md,bumpalo.md,map.md}|bumpalo:{advanced.md,pitfalls.md}
</docs_index>
</agent>

## TODO

- `CLAUDE.md` placeholder is referenced historically but file is not present in `docs/rust_crates/`.

## When to use these docs

Use this documentation when:
- You need "Rust basics / workspace defaults" → `core.md`
- Implementing LSP servers → `tower-lsp.md`
- Implementing MCP servers → `rmcp.md`
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
<mcp>rmcp.md</mcp>
<parsing>tree-sitter.md</parsing>
<graphs>petgraph.md</graphs>
<memory>bumpalo.md</memory>
<errors>error-handling.md</errors>
<testing>testing.md</testing>
