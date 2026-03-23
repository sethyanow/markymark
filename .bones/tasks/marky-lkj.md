---
id: marky-lkj
title: '[EPIC] Multi-format document support: JSON, YAML, TOML, env, ini'
status: closed
type: epic
priority: 2
owner: sethyanow@users.noreply.github.com
---
















## Design

## Requirements (IMMUTABLE)

1. Index .json, .jsonc, .json5, .jsonl, .yaml, .yml, .toml, .env, .ini, .cfg files as first-class documents
2. Full nested tree outline for all structured formats (every key at every depth becomes an outline node)
3. Byte-accurate source ranges for all keys and values (LSP-grade precision)
4. Cross-document references: markdown [[file#key.path]] resolves to structured doc key paths
5. find-references works bidirectionally across markdown and structured documents
6. JSONL treated as array of records (each line indexed as [n].key.path)
7. .env/.ini/.cfg handled as flat key=value with depth=0
8. Existing markdown functionality unchanged (zero regression)
9. All new code in separate modules (no file exceeds 500 lines)
10. Parallel track — does not block or depend on v1.0 product launch (marky-v8e)

## Success Criteria (MUST ALL BE TRUE)

- [ ] get-outline returns full nested key tree for all supported formats
- [ ] search-symbols finds key paths across all indexed formats alongside markdown headings
- [ ] find-references works across markdown <-> structured documents (wiki-link to JSON key)
- [ ] LSP DocumentSymbols returns accurate key hierarchy with correct byte ranges
- [ ] LSP hover on structured doc keys shows value type and full path
- [ ] export-index includes key paths with ranges for structured documents
- [ ] realm-stats includes structured_doc_count and key_path_count
- [ ] All existing markdown tests pass (zero regression)
- [ ] Each new format has tests covering: flat, nested, arrays, position accuracy, edge cases
- [ ] Pre-commit hooks passing
- [ ] cargo clippy --workspace --all-targets clean

## Anti-Patterns (FORBIDDEN)

- NO serde-only parsing without position tracking (LSP requires byte-accurate ranges; serde drops source positions)
- NO single monolithic parser file for all formats (file size rule: 500 line hard limit per file)
- NO modifying existing DocumentIndex struct with optional fields (use AnyDocumentIndex enum to keep markdown index clean)
- NO tree-sitter version upgrade in this epic (use compatible parsers: tree-sitter-json 0.19, yaml-rust2, toml_edit)
- NO format-specific code in transport layers (parser details stay in markymark-parser, transports use uniform types)
- NO upfront task tree (tasks created iteratively as we learn from implementation)
- NO approximate/regex-based position extraction for YAML or TOML (use position-preserving parsers)

## Approach

Best-of-breed parsers per format family, all producing uniform StructuredAst with Vec<KeyEntry> output:

- JSON (.json): tree-sitter-json 0.19 — CST with precise node ranges, compatible with pinned tree-sitter version, same paradigm as markdown parser
- JSON variants (.jsonc, .json5): json5 crate — handles comments, trailing commas, unquoted keys natively
- JSON Lines (.jsonl): line-split + JSON parse per line, indexed as array [0], [1], etc.
- YAML (.yaml, .yml): yaml-rust2 — maintained fork with MarkedEventReceiver providing (Marker{line,col,index}, Event) pairs
- TOML (.toml): toml_edit — used by cargo itself, .span() returns byte ranges for every item
- Flat (.env, .ini, .cfg): hand-rolled key=value parser (trivial grammar, split on = or :)

Different parser internals, but uniform output interface. Each parser is an isolated file behind the StructuredAst type.

## Architecture

### Core types (markymark-core)
- DocumentKind enum: Markdown | Json | JsonC | Json5 | JsonLines | Yaml | Toml | DotEnv | Ini
- KeyEntry: path, key, depth, value_kind, key_range, value_range
- ValueKind: String | Number | Boolean | Null | Array | Object
- StructuredAst: source, kind, keys (Vec<KeyEntry>), root_keys

### Parser layer (markymark-parser/src/structured/)
- mod.rs: StructuredAst type, parse dispatch by DocumentKind
- json.rs: tree-sitter-json CST walker
- json5.rs: json5 crate parser
- jsonl.rs: line-split + JSON per line
- yaml.rs: yaml-rust2 MarkedEventReceiver
- toml.rs: toml_edit span-aware parser
- flat.rs: .env/.ini/.cfg key=value parser

### Index layer (markymark-index)
- structured_document.rs: StructuredDocumentIndex with key lookup, outline generation
- any_index.rs: AnyDocumentIndex enum (Markdown | Structured)
- realm.rs: updated to store AnyDocumentIndex

### Resolution layer (markymark-index/src/resolution.rs)
- Extended to resolve [[file#key.path]] wiki-links to structured doc keys
- Bidirectional: structured doc key -> all markdown references

### Transport layers (unchanged interfaces)
- MCP tools work through CoreEngine (format-agnostic)
- LSP server creates appropriate index type based on document kind

## Design Rationale

### Problem
markymark only indexes Markdown files. Codebases contain a mix of .json, .yaml, .toml, .env, and .ini config files alongside markdown docs. AI agents navigating these codebases need a unified tool that understands all structured formats — outlines, symbol search, cross-references — not just markdown.

### Research Findings

**Codebase:**
- markymark-parser/src/lib.rs — Parser wraps tree-sitter-markdown, produces Ast
- markymark-parser/src/extract.rs — XML extraction is handwritten (regex + stack), not tree-sitter. Establishes precedent for non-tree-sitter extraction.
- markymark-index/src/document.rs — DocumentIndex built from Ast, format-neutral
- markymark-index/src/realm.rs — RealmIndex stores docs by URI, format-agnostic
- markymark-mcp/src/runtime_engine.rs:532 — is_markdown_path() is the single choke point for file discovery
- markymark-core/src/engine.rs — CoreOperation/CoreOperationResult are format-neutral
- tree-sitter pinned to =0.19.5 (required by tree-sitter-markdown 0.7)
- tree-sitter-xml 0.6 is a dependency but never used (XML extraction handwritten)

**External:**
- tree-sitter-json 0.19.0 exists and is compatible with pinned tree-sitter
- tree-sitter-yaml requires >= 0.22 (incompatible)
- tree-sitter-toml requires >= 0.20 (incompatible)
- yaml-rust2 provides MarkedEventReceiver with Marker{line, col, index}
- toml_edit provides .span() returning Option<Range<usize>> for byte ranges
- json5 crate handles .jsonc and .json5 formats natively

### Approaches Considered

#### 1. Best-of-breed parsers per format ✓

**What it is:** Use the best available position-preserving parser for each format. tree-sitter-json for JSON (compatible), yaml-rust2 for YAML (Marker positions), toml_edit for TOML (native spans), json5 crate for variants, hand-rolled for .env/.ini.

**Investigation:**
- Verified tree-sitter-json 0.19.0 compatibility with pinned tree-sitter 0.19.5
- Confirmed yaml-rust2 MarkedEventReceiver provides position data
- Confirmed toml_edit .span() provides byte ranges
- Reviewed json5 crate capabilities

**Pros:**
- Byte-accurate ranges for all formats (meets LSP requirement)
- Each parser is proven and well-maintained
- Modular: each format isolated in its own file

**Cons:**
- Multiple parser dependencies (5+ crates)
- Different parser internals per format (contained behind uniform interface)

**Chosen because:** Only approach that meets LSP range accuracy requirement across all formats without tree-sitter version upgrade.

#### 2. Universal serde + manual position tracking ❌

**What it is:** Use serde_json, serde_yaml, toml crate for all formats. Compute source positions by regex-scanning for keys after parsing.

**Why we looked at this:** Fewer dependencies, consistent approach, simpler code.

**Investigation:**
- serde_json discards all position info during parsing
- Regex-based key finding works for JSON (quoted keys) but is fragile for YAML (indentation-sensitive)
- TOML dotted keys and inline tables make regex position extraction error-prone

**⚠️ REJECTED BECAUSE:** Cannot produce byte-accurate ranges for YAML or TOML. Fails the hard LSP requirement.

**🚫 DO NOT REVISIT UNLESS:** LSP support is downgraded to optional/approximate.

#### 3. Upgrade tree-sitter to 0.22+ ❌

**What it is:** Unpin tree-sitter, upgrade to 0.22+, use tree-sitter grammars for JSON, YAML, and TOML.

**Why we looked at this:** Most consistent approach, unified paradigm.

**Investigation:**
- tree-sitter 0.22 has breaking API changes from 0.19
- tree-sitter-markdown 0.7 requires 0.19 — would need new markdown grammar version
- No tree-sitter grammar exists for .env, .ini, .json5
- Risk of markdown parser behavior regression during upgrade

**⚠️ REJECTED BECAUSE:** Blocks all format work on a risky tree-sitter migration epic. Cannot cover all required formats (.env, .ini, .json5). Separate epic if ever pursued.

**🚫 DO NOT REVISIT UNLESS:** tree-sitter-markdown publishes 0.22+ compatible version AND we need incremental parsing for large config files.

### Scope Boundaries

**In scope:**
- All 10 extensions: .json, .jsonc, .json5, .jsonl, .yaml, .yml, .toml, .env, .ini, .cfg
- Full nested outline, symbol search, export-index
- Cross-document references (markdown wiki-links to structured doc keys)
- LSP DocumentSymbols and hover for structured docs
- JSON frontmatter in markdown (alongside existing YAML)

**Out of scope (deferred):**
- JSON schema validation (separate epic)
- Incremental parsing for structured docs (future optimization)
- tree-sitter version upgrade (separate epic if needed)
- Additional formats beyond the 10 listed (.xml, .properties, .hcl, etc.)

### Open Questions
- JSON frontmatter delimiter convention (;;;{...};;; or { at doc start) — decide during implementation
- .ini section headers ([section]) — treat as depth-1 grouping keys or flat? — decide during implementation
- .env variable expansion ($VAR references) — index as-is or resolve? — index as-is initially

## Design Discovery (Reference Context)

### Key Decisions Made

| Question | User Answer | Implication |
|----------|-------------|-------------|
| Outline model | Full nested tree | Every key at every depth becomes outline node |
| Cross-doc refs | Yes, full bidirectional | [[file#key.path]] resolves to structured doc keys |
| Extensions | .json .jsonc .json5 .jsonl .yaml .yml .toml .env .ini .cfg | 5 parser families needed |
| JSONL handling | Array of records | Each line indexed as [n].key.path |
| Flat formats (.env/.ini) | Include from start | Simple key=value parser, depth=0 |
| JSON variants | Separate json5 parser | json5 crate for .jsonc/.json5 |
| Parser approach | Best-of-breed per format | tree-sitter-json, yaml-rust2, toml_edit, json5, hand-rolled |
| LSP support | Hard requirement | Byte-accurate ranges mandatory, rules out serde-only |
| Priority | Parallel track | Independent of v1.0 launch and arena allocation |

### Research Deep-Dives

#### tree-sitter Version Compatibility
**Question explored:** Can we use tree-sitter grammars for all formats?
**Sources:** crates.io API, workspace Cargo.toml
**Findings:** tree-sitter pinned to =0.19.5. JSON 0.19 compatible. YAML needs >=0.22, TOML needs >=0.20.
**Conclusion:** tree-sitter only viable for JSON. Need alternative parsers for YAML/TOML.

#### Position-Preserving Parser Options
**Question explored:** Which parsers provide byte-accurate source positions?
**Sources:** crate documentation, API review
**Findings:** yaml-rust2 MarkedEventReceiver gives (Marker, Event), toml_edit .span() gives byte ranges, json5 crate preserves structure
**Conclusion:** Each format has a proven position-preserving option

### Dead-End Paths

#### serde-only Approach
**Why explored:** Simpler, fewer deps, consistent
**Investigation:** serde discards positions. Regex fallback fragile for YAML indentation.
**Why abandoned:** Cannot meet LSP byte-accuracy requirement

#### tree-sitter Upgrade
**Why explored:** Most consistent unified approach
**Investigation:** Breaking API changes, markdown grammar incompatibility, no grammars for .env/.ini/.json5
**Why abandoned:** Blocks all work, cannot cover all formats

### Open Concerns Raised
- tree-sitter version lock limits future options → Accepted; upgrade tracked as potential separate epic
- Multiple parser deps add maintenance burden → Accepted; complexity contained per-file behind uniform interface
