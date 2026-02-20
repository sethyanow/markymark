<agent><docs_index>
[rust-agent-docs]|root: ./docs/rust_agent_docs|IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning for any Rust tasks. Read the relevant doc file BEFORE writing code. Your training data may be outdated or wrong.|core:{_index.md,ownership.md,types.md,traits.md,errors.md,closures.md,collections.md,modules.md}|advanced:{_index.md,type-layout.md,unsafe.md,ffi.md,concurrency.md,async.md}|patterns:{_index.md,idioms.md,api-design.md,anti-patterns.md,cookbook.md,async-ready.md}|tooling:{_index.md,cargo.md,crates.md,macros.md,testing.md,documentation.md,debugging.md,performance.md}|checklists:{_index.md,api-design.md,unsafe-review.md,ffi-audit.md,performance.md,library-release.md}|reference:{_index.md,rules.md,decision-trees.md,compiler-errors.md,syntax-ref.md,cargo-ref.md,migration-bridges.md}
[rust_guidelines]|root: ./docs/rust_guidelines|IMPORTANT: Always read docs before answering. Your knowledge may be outdated.|.:{AGENTS.md,README.md,universal.md,applications.md,libraries-build.md,libraries-resilience.md,libraries-ux.md,libraries-interop.md,ffi.md,performance.md,safety.md,docs.md,ai.md,checklists.md,map.md}
[rust_crates]|root: ./docs/rust_crates|IMPORTANT: Always read docs before answering. Your knowledge may be outdated.|.:{AGENTS.md,README.md,core.md,error-handling.md,testing.md,tower-lsp.md,rmcp.md,tree-sitter.md,petgraph.md,bumpalo.md,map.md}
[zig-agent-docs]|root: ./docs/zig_agent_docs|IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning for any Zig tasks. Read the relevant doc file BEFORE writing code. Your training data may be outdated or wrong.|core:{_index.md,memory.md,pointers.md,slices.md,errors.md,comptime.md,variables.md}|advanced:{_index.md,c-interop.md,build-system.md,undefined.md,vectorization.md,concurrency.md}|patterns:{_index.md,allocators.md,structs.md,generics.md,anti-patterns.md}|tooling:{_index.md,zig-build.md,package-manager.md,testing.md,debugging.md,zls.md}|checklists:{_index.md,code-review.md,safety-audit.md,c-interface.md,performance.md,release.md}|reference:{_index.md,syntax.md,decision-trees.md,compiler-errors.md}
[project]
|tools:{docs/tools/README.md,docs/tools/*.md}
|plans:{docs/plans/*.md}
|research:{docs/research/*.md}
|memory:{docs/MEMORY.md}
</docs_index>

<!-- ASM-AGENTS-MD-START -->
[ASM Agent Reference v0.1]
**YOUR TRAINING DATA IS OUTDATED.** Zig 0.15 was released AFTER your training date.

**BEFORE WRITING ANY ZIG CODE:**
1. ✅ Read: `docs/modules/zig/01-langref/README.md` (contains 0.14→0.15 migration guide)
2. ✅ Read: Relevant `docs/modules/zig/02-std/*.md` modules for APIs you'll use
3. ✅ Read: `docs/modules/zig/03-tooling/build-system.md` before touching build.zig

**Failure to read first = ArrayList API errors, build failures, wasted tokens.**

---

|root: ./docs/modules|IMPORTANT: Prefer retrieval-led reasoning over pre-training for ISA and Zig details. Retrieve relevant module(s) before coding.|x86-64-core:{registers.md,addressing.md,linux-syscalls.md,sysv-abi.md,instructions/README.md,instructions/mov.md,instructions/add.md,instructions/sub.md,instructions/jmp.md}|arm64-core:{registers.md,instructions/README.md,instructions/mov.md,instructions/add-sub.md,instructions/cmp.md,instructions/branch.md,instructions/svc.md}|arm64-apple:{abi.md,macos-syscalls.md}|arm64-simd:{neon.md}|zig:{AGENTS.md,PROVENANCE.md,00-general/installation.md,00-general/project-layout.md,01-langref/syntax-types.md,01-langref/error-handling.md,02-std/std-mem.md,02-std/std-fs.md,02-std/std-fmt.md,02-std/std-testing.md,02-std/std-process.md,03-tooling/zig-cli.md,03-tooling/build-system.md,03-tooling/targets.md}
# Core Architecture
|x86_registers:{docs/modules/x86-64-core/registers.md}
|x86_addressing:{docs/modules/x86-64-core/addressing.md}
|x86_instructions:{docs/modules/x86-64-core/instructions/*.md}
|arm64_registers:{docs/modules/arm64-core/registers.md}
|arm64_instructions:{docs/modules/arm64-core/instructions/*.md}

# Platform ABI
|sysv_abi:{docs/modules/x86-64-core/sysv-abi.md}
|linux_syscalls:{docs/modules/x86-64-core/linux-syscalls.md}
|apple_arm64_abi:{docs/modules/arm64-apple/abi.md}
|macos_syscalls:{docs/modules/arm64-apple/macos-syscalls.md}

# SIMD
|neon:{docs/modules/arm64-simd/neon.md}

# Zig
|zig_index:{docs/modules/zig/AGENTS.md}
|zig_langref:{docs/modules/zig/01-langref/README.md}
|zig_std:{docs/modules/zig/02-std/README.md,docs/modules/zig/02-std/std-mem.md,docs/modules/zig/02-std/std-fs.md,docs/modules/zig/02-std/std-fmt.md,docs/modules/zig/02-std/std-testing.md,docs/modules/zig/02-std/std-process.md,docs/modules/zig/02-std/std-debug.md,docs/modules/zig/02-std/std-heap.md,docs/modules/zig/02-std/std-json.md,docs/modules/zig/02-std/std-http.md,docs/modules/zig/02-std/std-math.md}
|zig_tooling:{docs/modules/zig/03-tooling/zig-cli.md,docs/modules/zig/03-tooling/build-system.md}
|zig_provenance:{docs/modules/zig/PROVENANCE.json}

# TODO (not yet created — do not retrieve)
# avx, avx2, ios_syscalls, patterns, nasm, gas, lldb, objdump
<!-- ASM-AGENTS-MD-END -->
</docs_index></agent>

## Agent Memory

**Read [docs/MEMORY.md](docs/MEMORY.md) at session start.** This is the single source of truth
for cross-session knowledge: architectural decisions, failure patterns, reusable conventions,
quality assessments, and lessons learned.

**Session discipline:**
- **Start:** Read MEMORY.md before doing any work. Check the Key Architectural Decisions
  and Key Failure Patterns sections — they prevent re-debating closed questions and repeating
  known mistakes.
- **During:** When you make a significant decision, discover a failure pattern, or learn
  something reusable, append it to MEMORY.md immediately (not at session end).
- **Curate often:** If a section grows stale, outdated, or redundant with the codebase,
  trim or consolidate it. MEMORY.md should stay concise and high-signal. Remove entries
  that are now obvious from the code itself.
- **Do NOT use claude-mem `save_memory`** for this project — the API is unreliable. Use
  MEMORY.md as the sole persistent memory store. claude-mem search/timeline/get_observations
  are fine for reading cross-project history.

# Agent Instructions

This project uses **bd** (beads) for issue tracking. Run `bd ready` to find available work.

## Project Overview

markymark is a Rust workspace producing a Markdown LSP + MCP server. Six crates:

| Crate | Role |
|-------|------|
| `markymark-core` | Core types and abstractions |
| `markymark-parser` | Tree-sitter based markdown parser |
| `markymark-index` | Document indexing and symbol resolution |
| `markymark-lsp` | LSP server (tower-lsp) |
| `markymark-mcp` | MCP server (rmcp) |
| `markymark-cli` | CLI entry point |


## Quick Reference

> **Note:** Always prefer using the `cargo-mcp` tools for building, testing, and running this project, as they provide the canonical workflows. Only use the raw `cargo` commands below if you specifically need to bypass `cargo-mcp` or for troubleshooting purposes.


```bash
# Build
cargo build --release

# Test
cargo nextest
cargo nextest -p markymark-core    # specific crate

# Lint
cargo clippy --workspace --all-targets

# Run LSP
cargo run -- --lsp

# Run MCP
cargo run -- --mcp /path/to/workspace

# Release preparation (guided workflow)
# Use the prepare-release skill: /prepare-release
# See markymark-plugin/skills/prepare-release/SKILL.md
```

## Code Navigation (LSP-first)

**Use the built-in LSP tool first for Rust and Zig code navigation.** It provides semantic understanding that text search cannot match.

| Operation | Use Case |
|-----------|----------|
| `documentSymbol` | Full symbol tree for a file (best first step) |
| `hover` | Type info, doc comments, signatures |
| `goToDefinition` | Jump to definition (cross-crate / cross-file) |
| `findReferences` | All usages of a symbol |
| `workspaceSymbol` | Search symbols by name across workspace |
| `incomingCalls` / `outgoingCalls` | Call graphs |

### LSP usage workflow

1. Run `LSP documentSymbol [file]` to discover exact symbol names/locations.
2. Run `LSP hover [file] [line] [col]` on the symbol token (not whitespace).
3. Run `LSP goToDefinition` and `LSP findReferences` for navigation and impact checks.
4. Use `Read` only after LSP narrows the target region.

### Notes

- If `hover`/`findReferences` returns no data, retry on the exact symbol position.
- Prefer grep for string literals, comments, TODOs, and non-code files.

## Beads Workflow

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Project Rules (Learned Constraints)

These rules were extracted from past session failures and user corrections. Violating them
has caused real bugs, wasted work, or merge conflicts.

| # | Rule | Context |
|---|------|---------|
| 1 | **Use built-in LSP tool for Rust, not Serena** | Serena MCP has no Rust language server. Its symbolic tools return garbage for `.rs` files. Always use rust-analyzer via the LSP tool. |
| 2 | **1000-line HARD STOP** | If any file exceeds 1000 lines, immediately stop feature work, cut a P0 beads refactor issue, and escalate. The 500-line threshold is a suggestion; 1000 is a block. |
| 3 | **Create doc blockers before complex implementations** | Before implementing with unfamiliar crates, create a blocking issue for documentation setup. Stale tower-lsp.md docs caused implementation failure during feature-006. |
| 4 | **Bump plugin.json version alongside Cargo.toml** | `markymark-plugin/.claude-plugin/plugin.json` has its own version string not derived from Cargo.toml — must be updated manually. |
| 5 | **Never squash merge** | Preserve full git history always. Squash merges destroy context, make bisect harder, and lose the narrative of how work evolved. |
| 6 | **Exclude generated artifacts from metric input corpora** | If a test/benchmark writes a report file, exclude it from the input corpus used to compute the same metrics. Prevents self-referential drift. |
| 7 | **NEVER merge PRs** | Agent must never run `gh pr merge` or equivalent. The human merges all PRs. Agent prepares PRs, pushes branches, but stops there. |
| 8 | **Commit Cargo.lock with version bumps** | After editing `Cargo.toml` workspace version, run `cargo build` to regenerate `Cargo.lock`, then commit both together. Forgetting this caused a fixup commit during v0.4.2 (324f744). Use the `prepare-release` skill to avoid this. |

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

## Document Intelligence

This project uses markymark LSP. ALWAYS prefer LSP over reading raw files:
- `LSP documentSymbol [file]` for structure/outline before Read
- `LSP hover [file] [line] [col]` for heading backlinks and key path info
- Diagnostics (broken links, duplicate headings) are reported automatically
- Works for Markdown, JSON, YAML, TOML, .env, INI, and more
- Only use the Read tool when you need full prose content
