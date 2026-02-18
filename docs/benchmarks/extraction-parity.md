# Extraction Parity: Zig SIMD vs tree-sitter

This report compares Tier 1 (Zig scan via `DocumentIndex::from_scan`) against Tier 2 (tree-sitter via `DocumentIndex::from_ast`) across the local `docs/` corpus.

## Corpus

- Parsed markdown files: 90
- Skipped files (read/parse failures): 0
- Files with setext headings: 2
- Files with frontmatter: 1
- Files with fenced code blocks: 61

## Aggregate Counts

| Metric | AST Total | Scan Total | False Positives | False Negatives |
|---|---:|---:|---:|---:|
| Headings | 1082 | 1156 | 74 | 0 |
| Wiki Links | 54 | 54 | 0 | 0 |
| Markdown Links | 394 | 392 | 0 | 2 |
| Tags | 106 | 61 | 0 | 45 |
| Block IDs | 0 | 0 | 0 | 0 |

## Rates

- Raw false positive rate: 4.52%
- Raw false negative rate: 2.87%
- Adjusted false positive rate (excluding known setext/frontmatter gaps): 4.30%
- Adjusted false negative rate (excluding known setext/frontmatter gaps): 2.59%
- Code block false-positive doc rate: 32.79% (20 / 61)
- Code block false-positive events: 74

## Known Differences

- Setext headings are a known gap for scan extraction (ATX-focused).
- Frontmatter can produce extra scan-side tags/links because scan is lexical and not AST-contextual.
- Fenced code blocks are the primary expected source of scan false positives.

## Top Mismatch Files (by false positives)

| File | False Positives | False Negatives | Setext | Frontmatter |
|---|---:|---:|:---:|:---:|
| `docs/rust_agent_docs/tooling/cargo.md` | 12 | 0 | no | no |
| `docs/plans/brza-markymark.md` | 8 | 1 | no | no |
| `docs/rust_crates/testing.md` | 6 | 2 | yes | no |
| `docs/tools/rename_symbol.md` | 6 | 2 | no | no |
| `docs/rust_agent_docs/reference/cargo-ref.md` | 6 | 0 | no | no |
| `docs/tools/get_document_outline.md` | 5 | 0 | no | no |
| `docs/rust_agent_docs/patterns/anti-patterns.md` | 4 | 0 | no | no |
| `docs/rust_agent_docs/advanced/ffi.md` | 3 | 0 | no | no |
| `docs/rust_agent_docs/advanced/unsafe.md` | 3 | 0 | no | no |
| `docs/rust_agent_docs/core/modules.md` | 3 | 0 | no | no |
| `docs/rust_agent_docs/reference/migration-bridges.md` | 3 | 0 | no | no |
| `docs/rust_agent_docs/tooling/documentation.md` | 3 | 0 | no | no |
| `docs/tools/get_hover_info.md` | 2 | 2 | no | no |
| `docs/tools/find_references.md` | 2 | 1 | no | no |
| `docs/rust_agent_docs/tooling/testing.md` | 2 | 0 | no | no |
| `docs/rust_crates/rmcp.md` | 2 | 0 | no | no |
| `docs/tools/goto_definition.md` | 1 | 5 | no | no |
| `docs/rust_guidelines/docs.md` | 1 | 3 | no | no |
| `docs/rust_agent_docs/tooling/macros.md` | 1 | 1 | no | no |
| `docs/rust_agent_docs/tooling/performance.md` | 1 | 0 | no | no |

