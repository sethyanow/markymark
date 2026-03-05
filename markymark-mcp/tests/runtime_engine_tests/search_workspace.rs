use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperationResult};
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

async fn engine_with_workspace_files(
    name: &str,
    files: &[(&str, &str)],
) -> (TempWorkspace, RuntimeEngine) {
    let ws = TempWorkspace::new(name);
    for (filename, content) in files {
        fs::write(ws.root().join(filename), content).expect("test file should be created");
    }
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");
    (ws, engine)
}

async fn search_workspace(
    engine: &RuntimeEngine,
    query: Option<&str>,
    fm_filter: Option<(&str, &str)>,
    prop_filter: Option<(&str, &str)>,
    tag_filter: Option<&str>,
    limit: u32,
) -> Vec<markymark_core::engine::WorkspaceSearchResult> {
    let result = engine
        .execute(markymark_core::engine::CoreOperation::SearchWorkspace {
            query: query.map(str::to_string),
            frontmatter_filter: fm_filter.map(|(k, v)| (k.to_string(), v.to_string())),
            property_filter: prop_filter.map(|(k, v)| (k.to_string(), v.to_string())),
            tag_filter: tag_filter.map(str::to_string),
            realm: None,
            limit,
        })
        .await;
    match result {
        CoreOperationResult::WorkspaceSearchResults { results, .. } => results,
        other => panic!("expected WorkspaceSearchResults, got: {other:?}"),
    }
}

#[tokio::test]
async fn search_workspace_returns_empty_for_no_matches() {
    let (_ws, engine) = engine_with_workspace_files(
        "sw-no-match",
        &[
            ("alpha.md", "# Alpha Document\n\nSome content.\n"),
            ("beta.md", "# Beta Document\n\nOther content.\n"),
        ],
    )
    .await;
    let results =
        search_workspace(&engine, Some("nonexistent_xyz_abc"), None, None, None, 20).await;
    assert!(
        results.is_empty(),
        "expected no results for unmatched query"
    );
}

#[tokio::test]
async fn search_workspace_case_insensitive_query() {
    // Bug caught: case-sensitive match silently drops results.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-case",
        &[("notes.md", "# Project Alpha\n\nSome content.\n")],
    )
    .await;
    let results = search_workspace(&engine, Some("project alpha"), None, None, None, 20).await;
    assert_eq!(
        results.len(),
        1,
        "lowercase query should match title with mixed case"
    );
    assert!(
        (results[0].score - 1.0).abs() < f32::EPSILON,
        "title match should score 1.0"
    );
    assert!(results[0].matched_fields.contains(&"title".to_string()));
}

#[tokio::test]
async fn search_workspace_title_match_scores_higher_than_heading_match() {
    // Bug caught: title and heading scoring swapped.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-title-score",
        &[
            ("title-doc.md", "# Query Term\n\nContent.\n"),
            (
                "heading-doc.md",
                "# Other Doc\n\n## Query Term\n\nContent.\n",
            ),
        ],
    )
    .await;
    let results = search_workspace(&engine, Some("query term"), None, None, None, 20).await;
    assert_eq!(results.len(), 2, "both docs should match");
    // Title match must rank first (score 1.0 > 0.8).
    assert_eq!(results[0].score, 1.0, "title match should score 1.0");
    assert!(results[0].matched_fields.contains(&"title".to_string()));
    assert!(
        results[1].score <= 0.8 + f32::EPSILON,
        "heading match should score at most 0.8"
    );
    assert!(results[1].matched_fields.contains(&"heading".to_string()));
}

#[tokio::test]
async fn search_workspace_frontmatter_filter_exact_key_match() {
    // Bug caught: partial key match returning wrong docs ("statue" matching "status" filter).
    let (_ws, engine) = engine_with_workspace_files(
        "sw-fm-key",
        &[
            ("active.md", "---\nstatus: active\n---\n# Active Doc\n"),
            ("draft.md", "---\nstatus: draft\n---\n# Draft Doc\n"),
        ],
    )
    .await;
    let results = search_workspace(&engine, None, Some(("status", "active")), None, None, 20).await;
    assert_eq!(results.len(), 1, "only doc with status=active should match");
    assert!(results[0].title.contains("Active"), "wrong doc returned");
}

#[tokio::test]
async fn search_workspace_frontmatter_filter_case_insensitive_value() {
    // Bug caught: case-sensitive value comparison drops valid results.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-fm-ci",
        &[("doc.md", "---\nstatus: Active\n---\n# Doc\n")],
    )
    .await;
    let results = search_workspace(&engine, None, Some(("status", "active")), None, None, 20).await;
    assert_eq!(
        results.len(),
        1,
        "lowercase filter value should match 'Active' frontmatter"
    );
}

#[tokio::test]
async fn search_workspace_frontmatter_list_value_any_element_matches() {
    // Bug caught: list values collapsed to string fails partial match.
    // Parser handles inline YAML list format: [a, b, c]
    let (_ws, engine) = engine_with_workspace_files(
        "sw-fm-list",
        &[(
            "doc.md",
            "---\naliases: [Project X, Proj X, PX]\n---\n# Document\n",
        )],
    )
    .await;
    let results =
        search_workspace(&engine, None, Some(("aliases", "proj x")), None, None, 20).await;
    assert_eq!(
        results.len(),
        1,
        "filter should match any element in frontmatter list"
    );
}

#[tokio::test]
async fn search_workspace_property_filter() {
    // Bug caught: property filter not applied.
    // Logseq properties (key:: value) must appear BEFORE headings in source.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-prop",
        &[
            ("daily.md", "type:: daily\n\n# Daily\n\nSome notes.\n"),
            ("note.md", "type:: note\n\n# Note\n\nSome notes.\n"),
        ],
    )
    .await;
    let results = search_workspace(&engine, None, None, Some(("type", "daily")), None, 20).await;
    assert_eq!(results.len(), 1, "only doc with type::daily should match");
    assert!(results[0].title.contains("Daily"), "wrong doc returned");
}

#[tokio::test]
async fn search_workspace_tag_filter_case_insensitive() {
    // Bug caught: case-sensitive tag matching drops valid results.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-tag-ci",
        &[
            ("tagged.md", "# Doc\n\n#Project content here.\n"),
            ("other.md", "# Other\n\n#daily content.\n"),
        ],
    )
    .await;
    let results = search_workspace(&engine, None, None, None, Some("project"), 20).await;
    assert_eq!(
        results.len(),
        1,
        "lowercase filter should match #Project tag"
    );
}

#[tokio::test]
async fn search_workspace_multiple_filters_and_logic() {
    // Bug caught: OR instead of AND logic for multiple filters.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-and-logic",
        &[
            ("a.md", "---\nstatus: active\n---\n# Doc A\n\n#project\n"),
            ("b.md", "---\nstatus: active\n---\n# Doc B\n\n#daily\n"),
        ],
    )
    .await;
    let results = search_workspace(
        &engine,
        None,
        Some(("status", "active")),
        None,
        Some("project"),
        20,
    )
    .await;
    assert_eq!(
        results.len(),
        1,
        "only doc matching BOTH status=active AND tag=project should return"
    );
    assert!(results[0].title.contains("Doc A"), "wrong doc returned");
}

#[tokio::test]
async fn search_workspace_respects_limit() {
    // Bug caught: limit not applied or results sorted wrong direction.
    // Search only covers title and headings, not body prose.
    // Use a heading so all 10 docs match the query.
    let files: Vec<(String, String)> = (0..10)
        .map(|i| {
            (
                format!("doc{i:02}.md"),
                format!("# Document {i}\n\n## Common Query Term\n\nsome content\n"),
            )
        })
        .collect();
    let file_refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, content)| (name.as_str(), content.as_str()))
        .collect();

    let (_ws, engine) = engine_with_workspace_files("sw-limit", &file_refs).await;
    let results = search_workspace(&engine, Some("common query term"), None, None, None, 3).await;
    assert_eq!(results.len(), 3, "limit=3 should return exactly 3 results");
    // Verify descending score order.
    for i in 1..results.len() {
        assert!(
            results[i - 1].score >= results[i].score,
            "results should be sorted score DESC"
        );
    }
}

#[tokio::test]
async fn search_workspace_limit_zero_returns_empty() {
    // Bug caught: limit=0 causes panic or returns all docs.
    let (_ws, engine) =
        engine_with_workspace_files("sw-limit-zero", &[("doc.md", "# Doc\n\nsome content\n")])
            .await;
    let results = search_workspace(&engine, None, None, None, None, 0).await;
    assert!(
        results.is_empty(),
        "limit=0 should return empty results, not error"
    );
}

#[tokio::test]
async fn search_workspace_empty_realm_returns_empty() {
    // Bug caught: iter_documents on empty realm panics.
    let ws = TempWorkspace::new("sw-empty-realm");
    // No files — empty directory.
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("empty workspace should index");
    let results = search_workspace(&engine, Some("anything"), None, None, None, 20).await;
    assert!(
        results.is_empty(),
        "empty realm should return empty results, not error"
    );
}

#[tokio::test]
async fn search_workspace_no_query_no_filter_returns_all_up_to_limit() {
    // Bug caught: no-filter path broken or no-query path errors.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-no-filter",
        &[
            ("a.md", "# Alpha\n"),
            ("b.md", "# Beta\n"),
            ("c.md", "# Gamma\n"),
        ],
    )
    .await;
    let results = search_workspace(&engine, None, None, None, None, 10).await;
    assert_eq!(results.len(), 3, "no filters should return all docs");
    for r in &results {
        assert!(
            (r.score - 1.0).abs() < f32::EPSILON,
            "all docs should score 1.0 with no query"
        );
    }
}

#[tokio::test]
async fn search_workspace_sort_descending_score_ties_by_uri_ascending() {
    // Bug caught: unstable sort, non-deterministic output across runs.
    let (_ws, engine) = engine_with_workspace_files(
        "sw-sort",
        &[
            // All docs share the same query match (heading), so score=0.8.
            // Tie-break should be URI ascending.
            ("zzz-last.md", "# Other\n\n## Query Term\n"),
            ("aaa-first.md", "# Other\n\n## Query Term\n"),
            ("mmm-mid.md", "# Other\n\n## Query Term\n"),
        ],
    )
    .await;
    let results = search_workspace(&engine, Some("query term"), None, None, None, 20).await;
    assert_eq!(results.len(), 3, "all three docs should match");
    // All should have the same score (0.8 for heading match) since no title match.
    for r in &results {
        assert!(
            (r.score - 0.8).abs() < f32::EPSILON,
            "all should score 0.8 for heading match"
        );
    }
    // URIs must be in ascending order (deterministic tie-break).
    let uris: Vec<&str> = results.iter().map(|r| r.uri.as_str()).collect();
    let mut sorted = uris.clone();
    sorted.sort();
    assert_eq!(
        uris, sorted,
        "results with equal score should be sorted by URI ascending"
    );
}

// --- Structured document search tests ---

/// Helper to create a workspace with both markdown and structured files.
async fn engine_with_mixed_files(
    name: &str,
    files: &[(&str, &str)],
) -> (TempWorkspace, RuntimeEngine) {
    let ws = TempWorkspace::new(name);
    for (filename, content) in files {
        // Ensure subdirectories exist.
        let path = ws.root().join(filename);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("subdirectory should be created");
        }
        fs::write(&path, content).expect("test file should be created");
    }
    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");
    (ws, engine)
}

#[tokio::test]
async fn search_workspace_finds_jsonl_by_value() {
    let jsonl_content = r#"{"id": "issue-1", "title": "Fix authentication bug", "status": "open"}
{"id": "issue-2", "title": "Add Layer 4 memory integration", "status": "closed"}"#;

    let (_ws, engine) = engine_with_mixed_files(
        "sw-jsonl",
        &[
            ("docs/guide.md", "# User Guide\n\nSome documentation.\n"),
            ("issues.jsonl", jsonl_content),
        ],
    )
    .await;

    let results = search_workspace(&engine, Some("authentication"), None, None, None, 20).await;
    assert!(
        results.iter().any(|r| r.uri.as_str().contains("issues.jsonl")),
        "search should find JSONL file when query matches content value"
    );
}

#[tokio::test]
async fn search_workspace_finds_yaml_by_key_path() {
    let yaml_content = "database:\n  host: localhost\n  port: 5432\n";

    let (_ws, engine) = engine_with_mixed_files(
        "sw-yaml",
        &[
            ("readme.md", "# Readme\n\nProject info.\n"),
            ("config.yaml", yaml_content),
        ],
    )
    .await;

    let results = search_workspace(&engine, Some("database"), None, None, None, 20).await;
    assert!(
        results.iter().any(|r| r.uri.as_str().contains("config.yaml")),
        "search should find YAML file when query matches key path"
    );
}

#[tokio::test]
async fn search_workspace_mixed_results_sorted_by_score() {
    let yaml_content = "host: localhost\nport: 5432\n";

    let (_ws, engine) = engine_with_mixed_files(
        "sw-mixed-sort",
        &[
            // Markdown: title match "config" → score 1.0
            ("config.md", "# Config\n\nSome configuration docs.\n"),
            // Structured: URI stem match "config" → score 1.0
            ("config.yaml", yaml_content),
            // Structured: key-path match only → score 0.8
            ("server.yaml", "config:\n  enabled: true\n"),
        ],
    )
    .await;

    let results = search_workspace(&engine, Some("config"), None, None, None, 20).await;
    assert!(
        results.len() >= 2,
        "should match at least the two config files, got: {}",
        results.len()
    );
    // Verify sort order: score DESC.
    for i in 1..results.len() {
        assert!(
            results[i - 1].score >= results[i].score,
            "results must be sorted score DESC"
        );
    }
}

#[tokio::test]
async fn search_workspace_filters_exclude_structured_docs() {
    let yaml_content = "status: active\ntags: [project]\n";

    let (_ws, engine) = engine_with_mixed_files(
        "sw-filter-exclude",
        &[
            (
                "doc.md",
                "---\nstatus: active\n---\n# Doc\n\nContent.\n",
            ),
            ("config.yaml", yaml_content),
        ],
    )
    .await;

    // Frontmatter filter should only return markdown docs.
    let results =
        search_workspace(&engine, None, Some(("status", "active")), None, None, 20).await;
    for r in &results {
        assert!(
            !r.uri.as_str().ends_with(".yaml"),
            "structured docs should be excluded when frontmatter filter is active"
        );
    }
}

#[tokio::test]
async fn search_workspace_no_query_includes_structured_docs() {
    let (_ws, engine) = engine_with_mixed_files(
        "sw-no-query-struct",
        &[
            ("readme.md", "# Readme\n"),
            ("config.json", r#"{"key": "value"}"#),
        ],
    )
    .await;

    let results = search_workspace(&engine, None, None, None, None, 20).await;
    assert!(
        results.iter().any(|r| r.uri.as_str().contains("config.json")),
        "no-query mode should include structured docs"
    );
    // All should score 1.0 in no-query mode.
    for r in &results {
        assert!(
            (r.score - 1.0).abs() < f32::EPSILON,
            "no-query results should all score 1.0"
        );
    }
}

#[tokio::test]
async fn search_workspace_structured_doc_uri_title_strips_extension() {
    let (_ws, engine) = engine_with_mixed_files(
        "sw-struct-title",
        &[("my-config.toml", "[database]\nhost = \"localhost\"\n")],
    )
    .await;

    let results = search_workspace(&engine, Some("database"), None, None, None, 20).await;
    assert_eq!(results.len(), 1, "should find the TOML file");
    assert_eq!(
        results[0].title, "my config",
        "title should strip .toml extension and convert separators"
    );
}

#[tokio::test]
async fn search_workspace_empty_structured_doc_excluded_by_query() {
    // An empty JSON file has 0 keys and empty source — should not match any query.
    let (_ws, engine) = engine_with_mixed_files(
        "sw-empty-struct",
        &[
            ("readme.md", "# Readme\n"),
            ("empty.json", "{}"),
        ],
    )
    .await;

    let results = search_workspace(&engine, Some("anything"), None, None, None, 20).await;
    assert!(
        !results.iter().any(|r| r.uri.as_str().contains("empty.json")),
        "empty structured doc should not match a query"
    );
}
