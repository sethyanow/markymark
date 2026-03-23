---
id: marky-dr3
title: Add DocumentKind enum, KeyEntry types, and expand file discovery
status: closed
type: feature
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---



## Design

## Goal
Add the foundational types and file discovery infrastructure that all subsequent format parsers build on. Zero behavior change — existing markdown indexing works exactly as before.

## Effort Estimate
4-6 hours (single session)

## Implementation

1. Study existing code:
   - markymark-core/src/lib.rs:14-68 (Position, Range types — pattern for new types)
   - markymark-core/src/lib.rs:74-106 (DocumentUri — pattern for new wrapper type)
   - markymark-mcp/src/runtime_engine.rs:500-537 (collect_markdown_files, is_markdown_path — functions to rename)
   - markymark-mcp/src/runtime_engine.rs:81 (sole call site of collect_markdown_files)
   - markymark-core/src/engine.rs:87-149 (CoreOperationResult — add fields to RealmStats/DocumentExport)

2. Write tests first (TDD):
   - test_document_kind_json: from_path("config.json") == Some(Json)
   - test_document_kind_jsonc: from_path("tsconfig.jsonc") == Some(JsonC)
   - test_document_kind_json5: from_path("config.json5") == Some(Json5)
   - test_document_kind_jsonl: from_path("logs.jsonl") == Some(JsonLines)
   - test_document_kind_yaml: from_path("config.yaml") == Some(Yaml)
   - test_document_kind_yml: from_path("config.yml") == Some(Yaml)
   - test_document_kind_toml: from_path("Cargo.toml") == Some(Toml)
   - test_document_kind_env: from_path(".env") == Some(DotEnv) — CRITICAL: dotfile edge case
   - test_document_kind_env_named: from_path("prod.env") == Some(DotEnv)
   - test_document_kind_env_local: from_path(".env.local") == None (extension is "local", not "env")
   - test_document_kind_ini: from_path("config.ini") == Some(Ini)
   - test_document_kind_cfg: from_path("setup.cfg") == Some(Ini)
   - test_document_kind_markdown: from_path("README.md") == Some(Markdown)
   - test_document_kind_markdown_long: from_path("doc.markdown") == Some(Markdown)
   - test_document_kind_unsupported: from_path("main.rs") == None
   - test_document_kind_no_extension: from_path("Makefile") == None
   - test_document_kind_case_insensitive: from_path("config.JSON") == Some(Json)
   - test_collect_documents_includes_json: collect_documents finds .json files alongside .md
   - test_collect_documents_markdown_unchanged: .md files still discovered (regression check)
   - test_collect_documents_only_markdown_indexed: non-.md files collected but not parsed/indexed

3. Implementation checklist:
   - [ ] markymark-core/src/lib.rs — add DocumentKind enum:
         ```rust
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
         pub enum DocumentKind {
             Markdown,   // .md, .markdown
             Json,       // .json
             JsonC,      // .jsonc
             Json5,      // .json5
             JsonLines,  // .jsonl
             Yaml,       // .yaml, .yml
             Toml,       // .toml
             DotEnv,     // .env (bare dotfile or *.env)
             Ini,        // .ini, .cfg
         }
         ```
   - [ ] DocumentKind::from_path(path: &Path) -> Option<Self>:
         - Check path.extension() first for standard extensions
         - ALSO check path.file_name() for bare dotfiles (.env)
         - Case-insensitive matching: compare ext.to_ascii_lowercase()
   - [ ] DocumentKind::extensions(&self) -> &[&str]: return all extensions for this kind
   - [ ] impl Display for DocumentKind: human-readable format name
   - [ ] markymark-core/src/structured.rs — new file with types:
         ```rust
         pub struct KeyEntry {
             pub path: String,        // "database.host", "servers[0].port"
             pub key: String,         // "host", "port"
             pub depth: usize,        // nesting level (0 = top-level)
             pub value_kind: ValueKind,
             pub key_range: Range,    // source range of key
             pub value_range: Range,  // source range of value
         }

         pub enum ValueKind {
             String, Number, Boolean, Null, Array, Object,
         }

         pub struct StructuredAst {
             pub source: String,
             pub kind: DocumentKind,
             pub keys: Vec<KeyEntry>,
         }
         ```
   - [ ] markymark-core/src/lib.rs — add `pub mod structured;` and re-export key types in prelude
   - [ ] markymark-mcp/src/runtime_engine.rs — rename is_markdown_path -> document_kind_for_path:
         ```rust
         fn document_kind_for_path(path: &Path) -> Option<DocumentKind> {
             DocumentKind::from_path(path)
         }
         ```
   - [ ] markymark-mcp/src/runtime_engine.rs — rename collect_markdown_files -> collect_documents:
         Return Vec<(PathBuf, DocumentKind)> instead of Vec<PathBuf>.
         Call site at line 81: update variable binding, filter for Markdown when indexing.
   - [ ] markymark-mcp/src/runtime_engine.rs line 81: update caller to destructure (path, kind), only index when kind == Markdown
   - [ ] markymark-core/src/engine.rs — add to RealmStats: `structured_doc_count: usize`, `key_path_count: usize`
   - [ ] markymark-core/src/engine.rs — add to DocumentExport: `document_kind: Option<DocumentKind>`
   - [ ] Update RealmStats construction sites in runtime_engine.rs to pass structured_doc_count: 0, key_path_count: 0
   - [ ] cargo test --workspace passes
   - [ ] cargo clippy --workspace --all-targets clean

## Success Criteria
- [ ] DocumentKind::from_path() correctly maps all 12 extensions (.json, .jsonc, .json5, .jsonl, .yaml, .yml, .toml, .env, .ini, .cfg, .md, .markdown)
- [ ] from_path() returns None for unsupported extensions (.rs, .txt, .py, Makefile)
- [ ] from_path() handles case insensitivity (.JSON, .Yaml, .TOML all match)
- [ ] from_path() detects bare .env dotfile via filename check (Path::extension returns None for .env)
- [ ] collect_documents() discovers JSON/YAML/TOML/env/ini files alongside markdown
- [ ] Only Markdown kind triggers indexing (zero behavior change)
- [ ] All existing tests pass unchanged (zero regression — run cargo test --workspace)
- [ ] KeyEntry, ValueKind, StructuredAst types compile with unit tests covering construction and field access
- [ ] RealmStats has structured_doc_count and key_path_count fields (initially 0)
- [ ] DocumentExport has document_kind field
- [ ] No file exceeds 500 lines (lib.rs ~240 lines, structured.rs ~80 lines, runtime_engine.rs ~610 lines)
- [ ] cargo clippy --workspace --all-targets clean
- [ ] Pre-commit hooks passing

## Anti-Patterns (FORBIDDEN)
- NO unwrap/expect in production code (use pattern matching or Result)
- NO TODO stubs or unimplemented!() — all types must have real implementations
- NO modifying existing DocumentIndex type (this task only adds new types)
- NO changing markdown parsing behavior (this task is infrastructure only)
- NO hardcoded extension list without tests (every extension mapping must have a test)
- NO extension-only matching for .env (Path::extension() returns None for dotfiles — MUST also check file_name())

## Key Considerations (SRE Review)

**CRITICAL: Dotfile Extension Edge Case**
Path::new(".env").extension() returns None in Rust. The file_name() is ".env" with no extension.
DocumentKind::from_path() MUST check file_name() as fallback for dotfiles.
Pattern: check extension first, then fall through to filename check for known dotfile names.
Test: test_document_kind_env with bare ".env" path.

**Case Sensitivity**
File extensions vary by platform (Windows: CONFIG.JSON, macOS/Linux: config.json).
from_path() must lowercase the extension before matching.
Test: test_document_kind_case_insensitive with ".JSON", ".Yaml", ".TOML".

**.env.local and Compound Extensions**
Path::new(".env.local").extension() returns Some("local"), not "env".
This should NOT match DotEnv kind — only bare .env and *.env should match.
Test: test_document_kind_env_local returns None.

**Call Site Update Safety**
Only 2 call sites for the renamed functions (both in runtime_engine.rs lines 81 and 522).
Both are private functions in the same file. No cross-crate API breakage.

**File Size Budget**
- markymark-core/src/lib.rs: 189 → ~240 lines (adding DocumentKind enum + impls)
- markymark-core/src/structured.rs: new file ~80 lines
- markymark-mcp/src/runtime_engine.rs: 605 → ~615 lines (function renames, same logic)
All well under 500-line limit.

**Reference Implementation**
- Study is_markdown_path() at runtime_engine.rs:532 for extension matching pattern
- Study DocumentUri at lib.rs:74 for type wrapper pattern with Display impl
