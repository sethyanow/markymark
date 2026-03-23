---
id: marky-9ui
title: Add semantic-search MCP tool
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-bcv]
---


Add semantic-search tool to markymark-mcp. Inputs: query (string), realm (optional), top_k (default 10), min_score (default 0.5). Calls RuntimeEngine semantic search -> EmbeddingProvider -> ZigEmbeddingIndex. Returns ranked list of doc_uri + heading + score. Feature-gated behind semantic-search flag. Also enhance realm-stats tool with optional --check-duplicates and token estimation per document.

## Design

## Goal
Add semantic-search tool to markymark-mcp. Input: query (string), realm (optional), top_k (default 10), min_score (default 0.5). Routes through RuntimeEngine to SemanticIndex. Returns ranked list of doc_uri + heading + score. Also enhance realm-stats tool with optional --check-duplicates and token estimation per document. Feature-gated behind semantic-search flag.

## Effort Estimate
8-10 hours

## Success Criteria
- [ ] semantic-search MCP tool registered and callable via MCP protocol
- [ ] Input schema matches spec Section 6.3: query (required), realm (optional), top_k (default 10), min_score (default 0.5)
- [ ] Returns JSON array of {doc_uri, heading, heading_level, score, section_preview}
- [ ] realm-stats enhanced with optional duplicate_check and token_counts fields
- [ ] Feature-gated behind semantic-search flag in markymark-mcp/Cargo.toml
- [ ] Without semantic-search: cargo test -p markymark-mcp passes (no new dependencies)
- [ ] With semantic-search: MCP tool integration test passes
- [ ] cargo clippy -p markymark-mcp -- -D warnings is clean

## Implementation Checklist
- [ ] Add semantic-search feature to markymark-mcp/Cargo.toml
- [ ] Create markymark-mcp/src/tools/semantic_search.rs
- [ ] Register semantic-search tool in MCP tool list (behind feature flag)
- [ ] Implement tool handler: parse input, call runtime_engine.semantic_search(), format output
- [ ] Add section_preview: first 200 chars of matched section for context
- [ ] Enhance realm-stats: add duplicate_pairs (if --check-duplicates), total_tokens
- [ ] Wire token estimation into realm-stats (call estimate_tokens per document)
- [ ] Write MCP tool integration test: create realm, add docs, search
- [ ] Write realm-stats enhancement test: verify new fields present
- [ ] Update MCP tool documentation

## Edge Cases
- Empty query: return empty results (or all documents ranked by some default)
- Query with no matches (min_score filters all): return empty results array
- Realm not found: return MCP error with clear message
- Realm with no semantic index (embeddings not enabled): return MCP error explaining feature requirement
- Very long query: truncate to embedding model's max input length
- top_k=0: return empty results
- min_score=0.0: return all results (no filtering)
- min_score=1.0: likely empty results (exact match only)
- Concurrent search requests: SemanticIndex must handle concurrent reads
- No EmbeddingProvider configured: return clear error message

## Anti-patterns
- NO blocking the MCP event loop with synchronous embedding calls (use async or spawn_blocking)
- NO returning raw float scores without rounding (round to 4 decimal places)
- NO hardcoding realm name (must support multi-realm)
- NO returning entire document content in results (preview only: first 200 chars of section)
- NO making semantic-search available without the feature flag

## Error Handling
- Missing required field (query): MCP input validation error
- Invalid top_k (negative): clamp to 0 or return MCP error
- Invalid min_score (not 0-1): clamp to [0.0, 1.0] or return MCP error
- Realm not found: MCP error with realm name in message
- Embedding provider not configured: MCP error "Semantic search requires an embedding provider"
- EmbeddingProvider::embed fails: MCP error with provider error message
- Internal search error: MCP error with details

## Test Specifications (what bug does each test catch?)
- test_semantic_search_basic: catches tool registration or handler wiring failure
- test_semantic_search_input_validation: catches accepting invalid input (missing query)
- test_semantic_search_results_format: catches malformed output JSON (missing fields)
- test_semantic_search_min_score_filter: catches results below min_score being included
- test_semantic_search_top_k_limit: catches returning more than requested results
- test_semantic_search_realm_not_found: catches panic instead of error on missing realm
- test_semantic_search_no_provider: catches unhelpful error when embeddings not configured
- test_realm_stats_with_duplicates: catches missing duplicate_pairs field in enhanced stats
- test_realm_stats_with_tokens: catches missing total_tokens field in enhanced stats
- test_feature_flag_gate: catches tool being available without semantic-search feature
- test_section_preview_truncation: catches preview exceeding 200 chars
