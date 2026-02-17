use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use markymark_mcp::{
    MarkymarkMcp, OutlineRequest, OutlineResponse, RuntimeEngine, SearchSymbolsRequest,
    SearchSymbolsResponse,
};
#[cfg(feature = "semantic-search")]
use markymark_mcp::{SemanticSearchRequest, SemanticSearchResponse};
use rmcp::handler::server::wrapper::Parameters;

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "markymark-mcp-runtime-tools-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temporary workspace directory should be created");
        Self { root }
    }

    fn root(&self) -> PathBuf {
        self.root.clone()
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn mcp_tools_return_real_indexed_data() {
    let ws = TempWorkspace::new("real-data");
    let file = ws.root().join("notes.md");
    fs::write(&file, "# Intro\nSome text\n## Deep Dive\n#rust #tools\n")
        .expect("markdown fixture should be written");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");
    let mcp = MarkymarkMcp::new(Arc::new(engine));

    let outline_result = mcp
        .get_outline_tool(Parameters(OutlineRequest {
            uri: format!("file://{}", file.to_string_lossy()),
            realm: None,
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

#[cfg(feature = "semantic-search")]
#[tokio::test]
async fn semantic_search_tool_returns_real_engine_results() {
    let ws = TempWorkspace::new("semantic-real");
    let file = ws.root().join("notes.md");
    fs::write(&file, "# Intro\nContext about embeddings.\n")
        .expect("markdown fixture should be written");

    let engine =
        RuntimeEngine::from_workspace_roots(vec![ws.root()]).expect("workspace should index");
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
