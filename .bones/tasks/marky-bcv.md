---
id: marky-bcv
title: Add SemanticIndex to RealmIndex with embedding support
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
depends_on: [marky-qv6, marky-0oz]
---





Create markymark-index/src/semantic.rs with SemanticIndex struct wrapping ZigEmbeddingIndex. SemanticEntry: doc_uri, heading, level, section_range. When document is added to realm (with embeddings feature), embed each heading+section and add to SemanticIndex. Semantic search method: embed query, search index, return ranked results. Add near-duplicate detection: entity_hashes per doc, jaccard pairwise, flag pairs > 0.8. Feature-gated behind embeddings flag.

## Design

## Goal
Create markymark-index/src/semantic.rs with SemanticIndex struct wrapping ZigEmbeddingIndex. When a document is added to a realm with the embeddings feature, embed each heading+section and add to SemanticIndex. Provide semantic search (embed query, search index, return ranked results) and near-duplicate detection (entity hashes + Jaccard similarity).

## Effort Estimate
10-14 hours

## Success Criteria
- [ ] SemanticIndex struct with new(), add_document(), search(), detect_duplicates() methods
- [ ] SemanticEntry contains: doc_uri, heading, heading_level, section_start, section_end
- [ ] add_document embeds each heading + first N tokens of section content
- [ ] search returns ranked Vec<SearchResult> with doc_uri, heading, score
- [ ] detect_duplicates returns Vec<(doc_uri_a, doc_uri_b, similarity)> for pairs > threshold
- [ ] Feature-gated behind embeddings flag in markymark-index/Cargo.toml
- [ ] Without embeddings: cargo test -p markymark-index passes (no embedding dependency)
- [ ] With embeddings: lifecycle test passes (create, add 10 docs, search, detect dupes, destroy)
- [ ] cargo clippy -p markymark-index -- -D warnings is clean

## Implementation Checklist
- [ ] Create markymark-index/src/semantic.rs
- [ ] Define SemanticIndex struct with ZigEmbeddingIndex + entries Vec
- [ ] Define SemanticEntry struct
- [ ] Define SearchResult struct (doc_uri, heading, score, section_range)
- [ ] Implement new(provider: Arc<dyn EmbeddingProvider>) constructor
- [ ] Implement add_document: extract headings, embed heading+section text, add to index
- [ ] Implement search: embed query, search index, map results to SearchResult
- [ ] Implement detect_duplicates: extract entity hashes per doc, pairwise Jaccard, filter by threshold
- [ ] Add embeddings feature to markymark-index/Cargo.toml: embeddings = ["zig-kernels"]
- [ ] Wire SemanticIndex into RealmIndex (optional field, initialized when embeddings enabled)
- [ ] Write lifecycle test: create, add documents, search, detect dupes
- [ ] Write search quality test: known document set, verify relevant results rank higher

## Edge Cases
- Empty realm: search returns empty results, detect_duplicates returns empty
- Document with no headings: use document title or filename as single entry
- Very short sections (<10 tokens): embed anyway, but quality may be low
- Many documents (1000+): embedding all sections may be slow — document expected latency
- Duplicate document add: update existing embeddings, don't create duplicates
- EmbeddingProvider failure: propagate error, don't partially update index
- Near-duplicate threshold: default 0.8, configurable via detect_duplicates parameter
- Jaccard on large document sets: O(n^2) pairwise comparison — document scaling limits
- Thread safety: SemanticIndex operations should be serialized (Mutex or &mut self)

## Anti-patterns
- NO embedding at indexing time without feature flag (must be opt-in)
- NO O(n^2) Jaccard without warning about scaling (forge failed at 1000+ episodes)
- NO storing raw embedding vectors in SemanticEntry (only store in ZigEmbeddingIndex)
- NO making SemanticIndex Clone (would double-free the Zig index handle)
- NO ignoring EmbeddingProvider errors during add_document (must propagate or skip with warning)
- NO blocking on embedding API calls without timeout

## Error Handling
- EmbeddingProvider::embed fails: return Err, document not added to semantic index
- ZigEmbeddingIndex::add fails: return Err with context
- Search on empty index: return Ok(empty vec), not error
- Detect duplicates on <2 documents: return Ok(empty vec)
- Invalid min_score (negative or >1.0): clamp to [0.0, 1.0]
- Invalid top_k (0): return Ok(empty vec)

## Test Specifications (what bug does each test catch?)
- test_semantic_index_empty: catches null deref on search/detect with no documents
- test_add_document_and_search: catches embedding not being added to index
- test_search_relevance_ordering: catches incorrect score sorting (highest first)
- test_search_min_score_filtering: catches results below min_score being returned
- test_search_top_k_limit: catches returning more than top_k results
- test_detect_duplicates_identical_docs: catches Jaccard not returning 1.0 for identical docs
- test_detect_duplicates_different_docs: catches false positive duplicates
- test_detect_duplicates_threshold: catches threshold not being applied
- test_add_document_no_headings: catches panic when document has no headings
- test_embedding_provider_failure: catches error not propagated from EmbeddingProvider
- test_feature_flag_gate: catches SemanticIndex being available without embeddings feature
- test_duplicate_document_add: catches stale entries when same doc re-added
