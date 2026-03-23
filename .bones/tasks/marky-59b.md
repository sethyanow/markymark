---
id: marky-59b
title: 'Task 4: Implement search-for-pattern MCP tool'
status: closed
type: feature
priority: 1
owner: sethyanow@users.noreply.github.com
depends_on: [marky-ck1]
parent: marky-9mo
---



## Design

## Goal
Add a search-for-pattern MCP tool to markymark-mcp that performs regex pattern search across workspace files with glob filtering.

## Context
Tasks 1-3 complete. Existing infrastructure:
- markymark-mcp/src/dto.rs — request/response DTO structs (serde)
- markymark-mcp/src/lib.rs — tool methods (862 lines, approaching limit)
- markymark-mcp/src/search.rs — workspace search logic
- Tool pattern: Parameters<Req> -> Result<CallToolResult, McpError>
- Realm-scoped: tools take Option<String> realm, resolve via CoreEngine

## Implementation Steps

### Step 1: Add DTOs to dto.rs
In markymark-mcp/src/dto.rs, add:
- SearchForPatternRequest { pattern: String, glob: Option<String>, realm: Option<String>, context_lines: Option<u32>, limit: u32 }
- default_search_for_pattern_limit() -> u32 { 50 }
- PatternMatchDto { uri: String, line: u32, column: u32, line_text: String, context_before: Vec<String>, context_after: Vec<String> }
- SearchForPatternResponse { realm: String, pattern: String, glob: Option<String>, total_matches: usize, results: Vec<PatternMatchDto> }

### Step 2: Create markymark-mcp/src/pattern.rs
Implement the core search function:
- execute_search_for_pattern(realm_key: &str, realm: &RealmIndex, pattern: &str, glob_filter: Option<&str>, context_lines: u32, limit: u32) -> CoreOperationResult
- Use regex crate (already in Cargo.toml? — check first, add if missing)
- For each indexed file in realm, if glob_filter is Some, match the file path against the glob pattern (use glob crate or manual pattern match)
- Read file content from disk (use std::fs::read_to_string), line-by-line search
- For each matching line: record uri, 1-based line number, 1-based column, matched line text, context_before/after lines
- Stop after limit total matches
- Return as CoreOperationResult::SearchForPattern(response)

Glob matching: simple approach — convert glob to regex OR use an existing glob crate. Check Cargo.toml for what's available. If globset or glob is present, use it. Otherwise, use simple suffix matching (*.rs matches paths ending in .rs).

### Step 3: Add CoreOperationResult variant (if needed)
In markymark-core/src/engine.rs or equivalent, check if CoreOperationResult has a variant for new tool results, or if tools return JSON directly. Look at how search_workspace_tool returns — it may use serde_json::to_value directly. Follow the same pattern.

### Step 4: Add search_for_pattern_tool to lib.rs
- Add #[tool(...)] annotation with description, add to tool_router
- Parse params, validate regex (return tool_error if invalid regex), call pattern::execute_search_for_pattern
- Return results as JSON text (same pattern as search_workspace_tool)

### Step 5: Add mod pattern; to lib.rs or Cargo.toml

### Step 6: Write tests FIRST (RED before GREEN)
In markymark-mcp/src/pattern.rs (tests module):
- test_search_for_pattern_no_results: pattern that matches nothing returns empty results
- test_search_for_pattern_finds_literal: simple literal pattern finds exact match
- test_search_for_pattern_regex_match: regex pattern like 'fn \w+' finds function defs
- test_search_for_pattern_glob_filter: glob filter *.md only returns markdown files
- test_search_for_pattern_glob_filter_excludes: glob *.rs excludes markdown files  
- test_search_for_pattern_context_lines: context_lines=1 returns 1 line before/after
- test_search_for_pattern_limit: limit=2 returns at most 2 results
- test_search_for_pattern_invalid_regex: returns error, not panic
- test_search_for_pattern_multiline_document: document with multiple matches, all found
- test_search_for_pattern_line_and_column_numbers: 1-based line/col correct for a known fixture

## Success Criteria
- [ ] SearchForPatternRequest, PatternMatchDto, SearchForPatternResponse in dto.rs
- [ ] pattern.rs module with execute_search_for_pattern function
- [ ] Glob filter works: *.md, *.rs, **/*.toml patterns
- [ ] search_for_pattern_tool in lib.rs wired to tool_router
- [ ] Invalid regex returns tool_error, not panic
- [ ] Limit enforced (max results capped)
- [ ] Context lines returned correctly (before/after)
- [ ] All 10 tests written RED first, then GREEN
- [ ] All existing workspace tests pass (no regressions)
- [ ] cargo fmt --check clean
- [ ] cargo clippy --workspace --all-targets clean
- [ ] lib.rs stays under 1000 lines (extract to pattern.rs)

## Anti-Patterns
- NO reading files outside the indexed realm roots
- NO glob crate introduction if globset already available
- NO unwrap/expect in production code paths
- NO adding pattern.rs over 400 lines (keep it focused)
