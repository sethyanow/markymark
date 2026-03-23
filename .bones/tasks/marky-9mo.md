---
id: marky-9mo
title: '[EPIC] MCP Platform First: Read Tools, Frontmatter, Logseq Intelligence, VSCode Extension'
status: closed
type: epic
priority: 1
owner: sethyanow@users.noreply.github.com
---








Fast-track markymark for personal dogfooding across Claude Code, Opencode, Cursor/VSCode, Obsidian, and Logseq. MCP-first approach: expand read intelligence, add frontmatter/property parsing, Logseq journal+block-ref support, and ship VSCode extension.

## Design

## Requirements (IMMUTABLE)

- Requirement 1: YAML frontmatter extraction from markdown files — parse delimited block, store as structured properties in DocumentIndex
- Requirement 2: Obsidian aliases support — index 'aliases' frontmatter field as alternative page names for search and linking
- Requirement 3: Logseq inline properties — parse 'property:: value' format in block content, store alongside frontmatter
- Requirement 4: Logseq journal page detection — identify journals/YYYY_MM_DD.md patterns, enable temporal navigation and queries
- Requirement 5: Logseq block reference resolution — resolve ((block-uuid)) references across the graph, surface in find-references and search
- Requirement 6: search-workspace MCP tool — full-text + frontmatter + property queries with ranked results
- Requirement 7: graph-analysis MCP tool — orphan detection, hub detection, broken link report, cluster analysis, summary stats
- Requirement 8: search-for-pattern MCP tool — regex pattern search across workspace files with glob filtering
- Requirement 9: VSCode extension — TypeScript extension spawning markymark --lsp as stdio child, activates on .md/.mdx, cross-platform binary selection, marketplace-ready

## Success Criteria (MUST ALL BE TRUE)

- [ ] Frontmatter parsed and stored in DocumentIndex for all indexed markdown files
- [ ] Obsidian aliases field indexed as alternative page names
- [ ] Logseq property:: value format parsed from block content
- [ ] Logseq journal pages detected by path pattern, queryable by date
- [ ] ((block-uuid)) references resolved to target blocks across graph
- [ ] search-workspace returns ranked results for text, frontmatter, and property queries
- [ ] graph-analysis detects orphans, hubs, broken links, and clusters
- [ ] search-for-pattern finds regex matches with file/line/context
- [ ] VSCode extension installs and provides LSP intelligence on .md/.mdx files
- [ ] All existing tests passing (no regressions)
- [ ] New tests for every new feature (frontmatter, properties, journal, block-refs, each MCP tool, VSCode extension)
- [ ] Pre-commit hooks passing
- [ ] cargo clippy clean

## Anti-Patterns (FORBIDDEN)

- ❌ NO write/editing MCP tools in this epic (scope: read-only intelligence — editing deferred to follow-up epic)
- ❌ NO Obsidian Dataview integration (complexity: Dataview query language is its own DSL, out of scope)
- ❌ NO Obsidian vault config awareness (coupling: reading .obsidian/ settings creates Obsidian-specific code paths)
- ❌ NO full Obsidian plugin (scope: VSCode extension only — Obsidian plugin is separate epic marky-hwc)
- ❌ NO new crates (consistency: extend existing markymark-parser, markymark-index, markymark-mcp crates)
- ❌ NO frontmatter parsing that breaks existing tests (safety: frontmatter is additive to existing heading/link/tag extraction)
- ❌ NO hardcoded Logseq journal date format (flexibility: make date pattern configurable, default to YYYY_MM_DD)

## Approach

Extend existing parser and index to handle frontmatter, Logseq properties, journal pages, and block references. Add 3 new read-only MCP tools that build on the enriched index. Package existing LSP as VSCode extension.

Parser layer: Add frontmatter extraction to markymark-parser AST. Parse YAML frontmatter (--- delimited), Obsidian aliases, Logseq property:: value inline format. Add journal page detection by path pattern. Add ((block-uuid)) reference extraction and resolution.

Index layer: Extend DocumentIndex with frontmatter HashMap, properties Vec, journal date Option, and block references. Build reverse lookup for block-uuid -> file+position.

MCP layer: Add search-workspace (full-text + structured queries), graph-analysis (link graph intelligence), and search-for-pattern (regex search). All consume existing + new index data.

VSCode extension: TypeScript project, minimal code — spawns markymark binary, activates on markdown, handles binary selection per platform.

## Architecture

Parser additions (markymark-parser):
- Frontmatter: extract YAML between --- delimiters at file start
- Properties: extract property:: value from Logseq block content
- Journal: detect YYYY_MM_DD pattern in file path
- Block refs: extract ((uuid)) patterns, store as references

Index additions (markymark-index):
- DocumentIndex gains: frontmatter, properties, journal_date, block_refs fields
- New reverse index: block-uuid -> (file, position)
- Existing wiki_links, headings, tags unchanged

MCP additions (markymark-mcp):
- search-workspace: combines text search + frontmatter/property query engine
- graph-analysis: traverses link graph for orphans, hubs, clusters, broken links
- search-for-pattern: regex search with glob filtering

VSCode extension (new markymark-vscode/ directory):
- package.json with activation events for .md/.mdx
- extension.ts: spawn markymark --lsp, configure LanguageClient
- Binary selection: detect OS/arch, select correct markymark binary

## Design Rationale

### Problem
markymark has strong read-only intelligence (9 MCP tools, 12 LSP handlers) but lacks frontmatter awareness, Logseq-specific intelligence, workspace-wide search, graph analysis, and editor packaging. The user works across Claude Code, Opencode, Cursor/VSCode, Obsidian, and Logseq daily — markymark needs to serve all of these through MCP (primary) and LSP (editors).

### Research Findings

**Codebase:**
- markymark-parser already has byte-accurate AST with heading, link, tag, code block extraction
- markymark-index/DocumentIndex stores all extraction results per file
- markymark-mcp has 9 tools + 2 prompts with realm isolation
- markymark-lsp has 12 handlers including hover, refs, completion
- Parser already handles wiki links, Obsidian callouts, Logseq block UUIDs
- Cross-platform binary selection already solved in markymark-plugin scripts

**External:**
- Obsidian does NOT support LSP natively — MCP via REST API is the ecosystem pattern
- Logseq has LogseqLSP (external) and MCP servers consuming its HTTP API
- Opencode fully supports MCP servers via stdio
- markdown-oxide (Rust) is the closest competitor — PKM LSP for Neovim/VSCode
- VSCode extension for LSP is straightforward: LanguageClient + binary spawn

### Approaches Considered

#### 1. MCP Platform First (Read Tools + VSCode) ✓

**What it is:** Expand read intelligence with frontmatter, Logseq features, 3 new MCP tools, and package VSCode extension. No write tools in this epic.

**Investigation:**
- Reviewed existing parser — frontmatter extraction adds to existing AST, no conflicts
- Checked DocumentIndex — HashMap/Vec additions are backward-compatible
- Validated MCP tool pattern — new tools follow existing 9-tool structure
- Confirmed VSCode extension approach — standard LanguageClient pattern

**Pros:**
- Immediate value across all user tools (Claude Code, Opencode, Cursor, VSCode)
- Builds on existing infrastructure (parser, index, MCP, LSP all proven)
- Manageable scope — no new crates, no write complexity
- Frontmatter unlocks most Obsidian/Logseq queries without deep plugin work

**Cons:**
- No editing capability (deferred)
- No Obsidian plugin (separate epic)

**Chosen because:** Maximizes value-per-effort across all tools the user works with daily

#### 2. Full Read + Write (Serena-level) ❌

**What it is:** Add both read tools AND structural editing MCP tools (replace-section, insert-before/after, delete, batch operations). Would make markymark a full document manipulation engine.

**Why we looked at this:** User showed Serena's tool set as inspiration — Serena provides both read and write operations for code.

**Investigation:**
- Estimated ~1400 lines additional for write layer
- Write operations need careful testing (data loss risk)
- Batch operations add transactional complexity
- Re-indexing after writes needs to be fast

**Pros:**
- Complete Serena-like experience for markdown
- Agents can modify documents through MCP

**Cons:**
- Doubles scope of epic
- Write operations have higher risk (data modification)
- Need careful undo/rollback design

**⚠️ REJECTED BECAUSE:** Scope too large for fast-track. Read intelligence + VSCode extension gives immediate dogfood value. Write tools can build on proven read infrastructure.

**🚫 DO NOT REVISIT UNLESS:** Read tools are shipped and user confirms write capability is the next priority.

#### 3. Obsidian Deep ❌

**What it is:** Full Obsidian intelligence — frontmatter, Dataview, vault config, Obsidian plugin with IDE-level features inside Obsidian.

**Why we looked at this:** User uses Obsidian daily and wants IDE-like tooling inside it.

**Investigation:**
- Obsidian plugins require Electron/Node build pipeline
- Obsidian has no LSP client — plugin would need to wrap MCP or HTTP calls
- Dataview is its own query DSL with significant parsing complexity
- Vault config (.obsidian/) has many settings affecting link resolution

**Pros:**
- Best possible single-tool experience

**Cons:**
- Separate build pipeline (TypeScript/Electron)
- Dataview complexity is massive
- Only benefits Obsidian users (not Logseq, Claude Code, etc.)

**⚠️ REJECTED BECAUSE:** Too narrow — only helps one tool. MCP platform approach helps ALL tools.

**🚫 DO NOT REVISIT UNLESS:** MCP platform is shipped and Obsidian-specific gaps are the top user pain point.

### Scope Boundaries

**In scope:**
- YAML frontmatter parsing and indexing
- Obsidian aliases field as alternative page names
- Logseq property:: value inline parsing
- Logseq journal page detection by path pattern
- Logseq ((block-uuid)) reference resolution
- search-workspace MCP tool
- graph-analysis MCP tool
- search-for-pattern MCP tool
- VSCode extension (marketplace-ready)

**Out of scope (deferred):**
- Write/editing MCP tools → follow-up epic
- Obsidian Dataview integration → future epic
- Obsidian vault config awareness → future epic
- Obsidian plugin → marky-hwc epic
- Full Logseq plugin → marky-hwc epic
- Logseq page hierarchy (namespace pages) → future task
- Content generation tools → future epic

### Open Questions
- Should search-workspace support query DSL (tag:X AND property:Y) or simple filters? (decide during implementation, start simple)
- Graph analysis: use existing petgraph dependency or build custom traversal? (check crate dependencies during implementation)
- VSCode extension: ship binary bundled or require separate install? (decide during first task)

## Design Discovery (Reference Context)

> Detailed context from brainstorming for task creation and obstacle handling.

### Key Decisions Made

| Question | User Answer | Implication |
|----------|-------------|-------------|
| Primary pain point? | All of the above (MCP, skills, IDE integration) | Broad scope, need to prioritize ruthlessly |
| Editors/tools? | Obsidian, Logseq, Claude Code, Opencode, Cursor/VSCode | MCP covers agents, LSP covers editors |
| Drop ix3 dependency? | Yes, decouple from mkr | Unblocks this epic immediately |
| What's missing today? | MCP tooling + VSCode extension | Confirms MCP-first approach |
| PKM scope? | Specific features for both Obsidian and Logseq | Need frontmatter, aliases, properties, journals, block refs |
| Extension packaging? | Need packaged extension | Must be marketplace-ready |
| Serena-like editing? | Too big, cut write tools | Read-only for this epic |
| Logseq features? | Journal pages + block ref resolution must be in scope | Added back after initial deferral |

### Research Deep-Dives

#### Codebase Capability Audit
**Question explored:** What does markymark currently support?
**Sources consulted:**
- markymark-mcp/src/lib.rs:125-617 — 9 MCP tools
- markymark-lsp/src/server.rs:86-680 — 12 LSP handlers
- markymark-parser/src/ast.rs — wiki links, callouts, block UUIDs
- markymark-plugin/ — Claude Code plugin with cross-platform binary selection

**Findings:**
- Full LSP and MCP working, cross-platform binary selection solved
- Parser handles Obsidian callouts and Logseq block UUIDs already
- No frontmatter extraction, no structured property queries
- No workspace-wide search or graph analysis tools

**Conclusion:** Strong foundation — frontmatter and properties are additive parser changes

#### Obsidian/Logseq Integration Landscape
**Question explored:** How do Obsidian and Logseq consume external intelligence?
**Sources consulted:**
- Obsidian Local REST API plugin documentation
- cyanheads/obsidian-mcp-server, jacksteamdev/obsidian-mcp-tools
- LogseqLSP, eugeneyvt/logseq-mcp-server
- Opencode MCP documentation

**Findings:**
- Obsidian: No LSP client. MCP via REST API plugin pattern.
- Logseq: External LSP/MCP servers consume graph directory
- markymark reads files directly — doesn't need REST API intermediaries
- Opencode has full MCP support via stdio

**Conclusion:** markymark MCP serves Obsidian/Logseq workspaces directly by pointing at vault/graph directories

### Dead-End Paths

#### Full Serena-level Write Tools
**Why explored:** User showed Serena tool list as inspiration
**Investigation:**
- Mapped all Serena tools to markymark equivalents
- Found markymark has 4/14 Serena tools (all read-side)
- Estimated ~1400 lines for full write layer
- Identified transactional complexity for batch operations

**Why abandoned:** Doubles epic scope. Read tools give immediate dogfood value. Write layer can build on proven read infrastructure in follow-up epic.

#### Obsidian Plugin Development
**Why explored:** User wants IDE-like tooling inside Obsidian
**Investigation:**
- Obsidian plugins are Electron/Node/TypeScript
- No LSP client in Obsidian plugin API
- Would need HTTP bridge or custom protocol
- Separate build pipeline from Rust workspace

**Why abandoned:** Too narrow (only Obsidian users benefit), separate build pipeline, MCP platform approach serves all tools

### Open Concerns Raised

- 'How big would write be?' → ~1400 lines, deferred to follow-up epic after dogfooding read tools
- 'Logseq journal and block refs must be in scope' → Added back, these are read-intelligence features not write tools
- ix3 dependency → Decoupled, will revisit after dogfooding
