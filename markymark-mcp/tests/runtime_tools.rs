mod common;

use std::fs;
use std::sync::Arc;

use common::TempWorkspace;
#[cfg(feature = "semantic-search")]
use markymark_core::prelude::EmbeddingProvider;
#[cfg(feature = "semantic-search")]
use markymark_mcp::{HashEmbeddingProvider, SemanticSearchRequest, SemanticSearchResponse};
use markymark_mcp::{
    ExportDocsIndexRequest, ExportDocsIndexResponse, MarkymarkMcp, OutlineRequest, OutlineResponse,
    RecommendDocsRequest, RecommendDocsResponse, RuntimeEngine, SearchSymbolsRequest,
    SearchSymbolsResponse,
};
use rmcp::handler::server::wrapper::Parameters;

#[tokio::test]
async fn mcp_tools_return_real_indexed_data() {
    let ws = TempWorkspace::new("real-data");
    let file = ws.root().join("notes.md");
    fs::write(&file, "# Intro\nSome text\n## Deep Dive\n#rust #tools\n")
        .expect("markdown fixture should be written");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");
    let mcp = MarkymarkMcp::new(Arc::new(engine));

    let outline_result = mcp
        .get_outline_tool(Parameters(OutlineRequest {
            uri: format!("file://{}", file.to_string_lossy()),
            realm: None,
            format: None,
            include_text: false,
        }))
        .await
        .expect("outline tool should return a result");
    assert_eq!(outline_result.is_error, Some(false));
    let outline: OutlineResponse = outline_result.into_typed().expect("typed outline");
    assert_eq!(
        outline.headings,
        vec!["Intro".to_string(), "Deep Dive".to_string()]
    );

    let symbols_result = mcp
        .search_symbols_tool(Parameters(SearchSymbolsRequest {
            query: "deep".to_string(),
            realm: None,
        }))
        .await
        .expect("search-symbols tool should return a result");
    assert_eq!(symbols_result.is_error, Some(false));
    let symbols: SearchSymbolsResponse = symbols_result.into_typed().expect("typed symbols");
    assert_eq!(symbols.symbols.len(), 1);
    assert_eq!(symbols.symbols[0].name, "Deep Dive");
}

#[tokio::test]
async fn export_docs_index_tool_returns_real_indexed_data() {
    let ws = TempWorkspace::new("export-docs-index");
    fs::create_dir_all(ws.root().join("core")).expect("create core dir");
    fs::write(ws.root().join("README.md"), "# My Docs\n").expect("write README");
    fs::write(ws.root().join("core/_index.md"), "# Core Index\n").expect("write core index");
    fs::write(ws.root().join("core/types.md"), "# Types\n").expect("write types");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");
    let mcp = MarkymarkMcp::new(Arc::new(engine));

    let result = mcp
        .export_docs_index_tool(Parameters(ExportDocsIndexRequest {
            realm: None,
            name_override: Some("my-docs".to_string()),
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: ExportDocsIndexResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.realm, "default");
    assert_eq!(payload.entries.len(), 1);
    assert_eq!(payload.doc_count, 3);
    assert_eq!(payload.skipped_count, 0);

    let entry = &payload.entries[0];
    assert!(entry.starts_with("[my-docs]|root: "), "expected [my-docs] prefix, got: {entry}");
    assert!(entry.contains("|.:{README.md}"), "expected root-level README.md");
    assert!(entry.contains("|core:{_index.md,types.md}"), "expected core category with sorted files");
}

#[cfg(feature = "semantic-search")]
#[tokio::test]
async fn semantic_search_tool_returns_real_engine_results() {
    let ws = TempWorkspace::new("semantic-real");
    let file = ws.root().join("notes.md");
    fs::write(&file, "# Intro\nContext about embeddings.\n")
        .expect("markdown fixture should be written");

    let provider: Arc<dyn EmbeddingProvider> = Arc::new(HashEmbeddingProvider::new(128));
    let engine = RuntimeEngine::from_workspace_roots_with_provider(vec![ws.root()], Some(provider))
        .await
        .expect("workspace should index");
    let mcp = MarkymarkMcp::new(Arc::new(engine));

    let result = mcp
        .semantic_search_tool(Parameters(SemanticSearchRequest {
            query: "intro embeddings".to_string(),
            realm: None,
            top_k: Some(5),
            min_score: Some(0.0),
        }))
        .await
        .expect("semantic-search tool should return a result");

    assert_eq!(result.is_error, Some(false));
    let payload: SemanticSearchResponse = result.into_typed().expect("typed semantic response");
    assert_eq!(payload.realm, "default");
    assert!(!payload.results.is_empty());
    assert_eq!(payload.results[0].heading, "Intro");
    assert!(payload.results[0].section_preview.len() <= 200);
}

#[tokio::test]
async fn recommend_docs_tool_returns_real_ranked_results() {
    let ws = TempWorkspace::new("recommend-docs");
    fs::write(
        ws.root().join("rust_guide.md"),
        "# Rust Guide\n\nLearn Rust programming with examples.\n",
    )
    .expect("write rust guide");
    fs::write(
        ws.root().join("python_guide.md"),
        "# Python Guide\n\nLearn Python programming with examples.\n",
    )
    .expect("write python guide");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");
    let mcp = MarkymarkMcp::new(Arc::new(engine));

    let result = mcp
        .recommend_docs_tool(Parameters(RecommendDocsRequest {
            query: "Rust".to_string(),
            realm: None,
            top_k: 5,
            include_sections: false,
        }))
        .await
        .expect("tool call should not return protocol error");

    assert_eq!(result.is_error, Some(false));
    let payload: RecommendDocsResponse = result.into_typed().expect("typed response");
    assert_eq!(payload.realm, "default");
    assert_eq!(payload.query, "Rust");
    assert!(!payload.recommendations.is_empty());

    // Rust guide should be the top recommendation (title match)
    let top = &payload.recommendations[0];
    assert_eq!(top.title, "Rust Guide");
    assert!(top.relevance_score > 0.0);
    assert!(top.search_score > 0.0);
}
