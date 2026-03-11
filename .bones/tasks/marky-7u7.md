---
id: marky-7u7
title: Add crates.io metadata to all workspace crates
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-peu
---



Add required crates.io publishing metadata to all 6 crates.

## Deliverables
1. description, keywords, categories in Cargo.toml for: markymark-core, parser, index, lsp, mcp
2. cargo publish --dry-run passes for all crates in dependency order
3. Per-crate README.md files with brief usage examples

## Design

## Goal
Add required crates.io publishing metadata to all 6 crates so cargo publish --dry-run succeeds.

## Codebase Verification
- Workspace metadata in root Cargo.toml: version 0.1.0, MIT OR Apache-2.0, repo URL
- markymark-cli already has full metadata (description, keywords, categories)
- 5 crates missing: description, keywords, categories
- All internal deps use path = "../..." — must add version alongside for crates.io
- No per-crate README.md files (optional, skip for alpha)

## Implementation Steps

### Step 1: Add metadata to markymark-core/Cargo.toml
Add after authors.workspace = true:
\`\`\`toml
description = "Core types and traits for the markymark markdown LSP and MCP server"
keywords = ["markdown", "lsp", "parser", "types"]
categories = ["development-tools", "text-processing"]
\`\`\`

### Step 2: Add metadata to markymark-parser/Cargo.toml
\`\`\`toml
description = "Tree-sitter based parser for CommonMark, Obsidian, and Logseq markdown"
keywords = ["markdown", "parser", "tree-sitter", "obsidian"]
categories = ["parsing", "text-processing"]
\`\`\`

### Step 3: Add metadata to markymark-index/Cargo.toml
\`\`\`toml
description = "Document indexing and cross-reference resolution for markdown workspaces"
keywords = ["markdown", "index", "cross-reference", "workspace"]
categories = ["development-tools", "text-processing"]
\`\`\`

### Step 4: Add metadata to markymark-lsp/Cargo.toml
\`\`\`toml
description = "LSP server for markdown with go-to-definition, references, hover, and diagnostics"
keywords = ["markdown", "lsp", "language-server", "editor"]
categories = ["development-tools", "text-editors"]
\`\`\`

### Step 5: Add metadata to markymark-mcp/Cargo.toml
\`\`\`toml
description = "MCP server for markdown intelligence in AI assistants"
keywords = ["markdown", "mcp", "ai", "language-server"]
categories = ["development-tools", "text-processing"]
\`\`\`

### Step 6: Add version to path dependencies
For each internal dependency, add version = "0.1.0" alongside path:
\`\`\`toml
markymark-core = { version = "0.1.0", path = "../markymark-core" }
\`\`\`
Apply to all internal deps across parser, index, lsp, mcp, cli (total ~10 lines).

### Step 7: Dry-run publish in dependency order
\`\`\`bash
cargo publish --dry-run -p markymark-core
cargo publish --dry-run -p markymark-parser
cargo publish --dry-run -p markymark-index
cargo publish --dry-run -p markymark-lsp
cargo publish --dry-run -p markymark-mcp
cargo publish --dry-run -p markymark-cli
\`\`\`

### Step 8: Verify tests still pass
\`\`\`bash
cargo test --workspace
cargo clippy --workspace --all-targets
\`\`\`

### Step 9: Commit
\`\`\`bash
git add */Cargo.toml Cargo.toml
git commit -m "chore: add crates.io metadata to all workspace crates"
\`\`\`

## Success Criteria
- [ ] All 6 crates have description, keywords, categories
- [ ] All internal deps have version alongside path
- [ ] cargo publish --dry-run succeeds for all 6 crates in order
- [ ] cargo test --workspace passes
