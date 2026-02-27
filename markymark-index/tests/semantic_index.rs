#![cfg(feature = "embeddings")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use markymark_core::prelude::{EmbedError, EmbeddingProvider};
use markymark_core::DocumentUri;
use markymark_index::{DocumentIndex, RealmIndex, SemanticIndex};
use markymark_parser::Parser;

fn uri(name: &str) -> DocumentUri {
    DocumentUri::from_file_path(&PathBuf::from(format!("/vault/{name}")))
}

fn index_from(source: &str) -> DocumentIndex {
    let mut parser = Parser::new().expect("parser init");
    let ast = parser.parse(source).expect("parse");
    DocumentIndex::from_ast(ast)
}

#[derive(Debug)]
struct KeywordEmbeddingProvider;

#[async_trait]
impl EmbeddingProvider for KeywordEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let lower = text.to_ascii_lowercase();
        Ok(vec![
            if lower.contains("rust") { 1.0 } else { 0.0 },
            if lower.contains("simd") { 1.0 } else { 0.0 },
            if lower.contains("graph") { 1.0 } else { 0.0 },
            if lower.contains("search") { 1.0 } else { 0.0 },
            if lower.contains("notes") { 1.0 } else { 0.0 },
        ])
    }

    fn dimensions(&self) -> u32 {
        5
    }
}

#[derive(Debug)]
struct FailingEmbeddingProvider;

#[async_trait]
impl EmbeddingProvider for FailingEmbeddingProvider {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
        Err(EmbedError::ProviderUnavailable(
            "provider unavailable".to_string(),
        ))
    }

    fn dimensions(&self) -> u32 {
        5
    }
}

#[derive(Debug)]
struct FailOnNthEmbeddingProvider {
    fail_on: usize,
    calls: AtomicUsize,
}

impl FailOnNthEmbeddingProvider {
    fn new(fail_on: usize) -> Self {
        Self {
            fail_on,
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for FailOnNthEmbeddingProvider {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call >= self.fail_on {
            return Err(EmbedError::ProviderUnavailable(
                "provider unavailable".to_string(),
            ));
        }
        Ok(vec![1.0; 5])
    }

    fn dimensions(&self) -> u32 {
        5
    }
}

#[derive(Debug)]
struct BatchToggleProvider {
    fail_batch: bool,
    embed_calls: AtomicUsize,
    batch_calls: AtomicUsize,
    batch_items: AtomicUsize,
}

impl BatchToggleProvider {
    fn new(fail_batch: bool) -> Self {
        Self {
            fail_batch,
            embed_calls: AtomicUsize::new(0),
            batch_calls: AtomicUsize::new(0),
            batch_items: AtomicUsize::new(0),
        }
    }

    fn embed_vector(text: &str) -> Vec<f32> {
        let lower = text.to_ascii_lowercase();
        vec![
            if lower.contains("alpha") { 1.0 } else { 0.0 },
            if lower.contains("beta") { 1.0 } else { 0.0 },
            if lower.contains("gamma") { 1.0 } else { 0.0 },
            if lower.contains("zeta") { 1.0 } else { 0.0 },
            if lower.contains("topic") { 1.0 } else { 0.0 },
        ]
    }
}

#[async_trait]
impl EmbeddingProvider for BatchToggleProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        self.embed_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Self::embed_vector(text))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.batch_calls.fetch_add(1, Ordering::SeqCst);
        self.batch_items.fetch_add(texts.len(), Ordering::SeqCst);
        if self.fail_batch {
            return Err(EmbedError::InternalError(
                "forced batch failure".to_string(),
            ));
        }
        Ok(texts.iter().map(|text| Self::embed_vector(text)).collect())
    }

    fn dimensions(&self) -> u32 {
        5
    }
}

#[tokio::test]
async fn test_semantic_index_empty() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let results = index
        .search("rust", 10, 0.0)
        .await
        .expect("search should succeed");
    assert!(results.is_empty());

    let duplicates = index.detect_duplicates(0.8);
    assert!(duplicates.is_empty());
}

#[tokio::test]
async fn test_add_document_and_search() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let rust_uri = uri("rust.md");
    index
        .add_document(
            rust_uri.clone(),
            &index_from("# Rust SIMD\n\nFast vectorized markdown scanning."),
        )
        .await
        .expect("add rust doc");

    let graph_uri = uri("graph.md");
    index
        .add_document(
            graph_uri.clone(),
            &index_from("# Link Graph\n\nGraph based backlink traversal."),
        )
        .await
        .expect("add graph doc");

    let results = index
        .search("rust simd search", 5, 0.0)
        .await
        .expect("search should succeed");

    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
    assert!(results[0].score >= results.last().expect("non-empty").score);
}

#[tokio::test]
async fn test_add_document_no_headings_uses_fallback() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let notes_uri = uri("notes.md");
    index
        .add_document(
            notes_uri.clone(),
            &index_from("plain content without headings"),
        )
        .await
        .expect("fallback indexing should succeed");

    let results = index
        .search("notes", 5, 0.0)
        .await
        .expect("search should succeed");
    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, notes_uri);
}

#[tokio::test]
async fn test_embedding_provider_failure_propagates() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(FailingEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let err = index
        .add_document(uri("broken.md"), &index_from("# Broken"))
        .await
        .expect_err("expected provider failure");

    assert!(matches!(err, EmbedError::ProviderUnavailable(_)));
}

#[tokio::test]
async fn test_add_document_failure_does_not_leave_partial_entries() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(FailOnNthEmbeddingProvider::new(2));
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    let err = index
        .add_document(
            uri("partial.md"),
            &index_from("# First Heading\n\n## Second Heading"),
        )
        .await
        .expect_err("expected provider failure on second heading");
    assert!(matches!(err, EmbedError::ProviderUnavailable(_)));

    assert_eq!(
        index.entry_count(),
        0,
        "failed add_document should not commit partial semantic entries",
    );
    let results = index
        .search("first", 10, 0.0)
        .await
        .expect("search after failed add_document should succeed");
    assert!(
        results.is_empty(),
        "failed add_document should not leak stale semantic matches",
    );
}

#[tokio::test]
async fn test_detect_duplicates_threshold() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut index = SemanticIndex::new(provider).expect("semantic index should initialize");

    index
        .add_document(
            uri("doc-a.md"),
            &index_from("# Rust SIMD\n\nsearch graph rust simd"),
        )
        .await
        .expect("add doc-a");
    index
        .add_document(
            uri("doc-b.md"),
            &index_from("# Rust SIMD\n\nsearch graph rust simd"),
        )
        .await
        .expect("add doc-b");

    let duplicates = index.detect_duplicates(0.8);
    assert_eq!(duplicates.len(), 1);
    assert!(duplicates[0].similarity >= 0.8);
}

#[tokio::test]
async fn test_realm_semantic_search_integration() {
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(KeywordEmbeddingProvider);
    let mut realm = RealmIndex::new_with_embeddings(provider).expect("realm with embeddings");

    let rust_uri = uri("r.md");
    realm
        .add_document(
            rust_uri.clone(),
            index_from("# Rust Search\n\nSemantic search via embeddings"),
        )
        .await;

    let results = realm
        .semantic_search("rust", 3, 0.0)
        .await
        .expect("semantic search should succeed");

    assert!(!results.is_empty());
    assert_eq!(results[0].doc_uri, rust_uri);
}

#[tokio::test]
async fn test_add_documents_batch_results_match_sequential_fallback() {
    let batch_provider = Arc::new(BatchToggleProvider::new(false));
    let fallback_provider = Arc::new(BatchToggleProvider::new(true));

    let mut batch_index =
        SemanticIndex::new(batch_provider.clone()).expect("batch semantic index should initialize");
    let mut fallback_index = SemanticIndex::new(fallback_provider.clone())
        .expect("fallback semantic index should initialize");

    let docs = [
        (
            uri("alpha.md"),
            index_from("# Alpha Topic\n\nalpha body text"),
        ),
        (uri("beta.md"), index_from("# Beta Topic\n\nbeta body text")),
        (
            uri("gamma.md"),
            index_from("# Gamma Topic\n\ngamma body text"),
        ),
    ];

    let batch_docs: Vec<(DocumentUri, &DocumentIndex)> =
        docs.iter().map(|(uri, doc)| (uri.clone(), doc)).collect();
    let fallback_docs: Vec<(DocumentUri, &DocumentIndex)> =
        docs.iter().map(|(uri, doc)| (uri.clone(), doc)).collect();

    batch_index
        .add_documents(batch_docs)
        .await
        .expect("batch add_documents should succeed");
    fallback_index
        .add_documents(fallback_docs)
        .await
        .expect("fallback add_documents should succeed");

    let batch_results = batch_index
        .search("alpha topic", 3, 0.0)
        .await
        .expect("batch search should succeed");
    let fallback_results = fallback_index
        .search("alpha topic", 3, 0.0)
        .await
        .expect("fallback search should succeed");

    assert_eq!(
        batch_results.len(),
        fallback_results.len(),
        "batch and sequential fallback should return same number of results",
    );
    let batch_uris: Vec<String> = batch_results
        .iter()
        .map(|result| result.doc_uri.as_str().to_string())
        .collect();
    let fallback_uris: Vec<String> = fallback_results
        .iter()
        .map(|result| result.doc_uri.as_str().to_string())
        .collect();
    assert_eq!(
        batch_uris, fallback_uris,
        "batch and sequential fallback should rank documents identically",
    );

    assert_eq!(
        batch_provider.batch_calls.load(Ordering::SeqCst),
        1,
        "batch provider should be called once",
    );
    assert_eq!(
        batch_provider.embed_calls.load(Ordering::SeqCst),
        1,
        "query embedding should use one embed() call",
    );
    assert_eq!(
        fallback_provider.batch_calls.load(Ordering::SeqCst),
        1,
        "fallback provider should still attempt one batch call",
    );
    assert_eq!(
        fallback_provider.embed_calls.load(Ordering::SeqCst),
        4,
        "fallback provider should embed each heading plus one query",
    );
}

#[tokio::test]
async fn test_add_documents_batch_skips_empty_headings() {
    let provider = Arc::new(BatchToggleProvider::new(false));
    let mut index = SemanticIndex::new(provider.clone()).expect("semantic index should initialize");

    let doc = index_from("# Alpha Topic\n# \n## Beta Topic\n");
    index
        .add_documents(vec![(uri("empty.md"), &doc)])
        .await
        .expect("batch add_documents should succeed with empty heading");

    assert_eq!(index.entry_count(), 2, "empty heading should be skipped");
    assert_eq!(
        provider.batch_calls.load(Ordering::SeqCst),
        1,
        "batch mode should issue one embed_batch call",
    );
    assert_eq!(
        provider.batch_items.load(Ordering::SeqCst),
        2,
        "only two non-empty headings should be embedded",
    );
}

#[tokio::test]
async fn test_add_documents_batch_handles_mixed_headings_and_fallback_docs() {
    let provider = Arc::new(BatchToggleProvider::new(false));
    let mut index = SemanticIndex::new(provider.clone()).expect("semantic index should initialize");

    let doc_with_heading = index_from("# Alpha Topic\n\nalpha details");
    let doc_without_heading = index_from("plain content no heading");
    let doc_with_other_heading = index_from("# Gamma Topic\n\ngamma details");

    let docs = vec![
        (uri("alpha.md"), &doc_with_heading),
        (uri("zeta-fallback.md"), &doc_without_heading),
        (uri("gamma.md"), &doc_with_other_heading),
    ];
    index
        .add_documents(docs)
        .await
        .expect("mixed add_documents should succeed");

    assert_eq!(
        index.entry_count(),
        3,
        "one heading + one fallback + one heading should produce three entries",
    );
    assert_eq!(
        provider.batch_calls.load(Ordering::SeqCst),
        1,
        "all mixed docs should be embedded in one batch call",
    );
    assert_eq!(
        provider.batch_items.load(Ordering::SeqCst),
        3,
        "two headings plus one fallback heading should be embedded",
    );

    let alpha_results = index
        .search("alpha", 1, 0.0)
        .await
        .expect("alpha search should succeed");
    assert_eq!(alpha_results[0].doc_uri, uri("alpha.md"));

    let zeta_results = index
        .search("zeta", 1, 0.0)
        .await
        .expect("zeta search should succeed");
    assert_eq!(zeta_results[0].doc_uri, uri("zeta-fallback.md"));

    let gamma_results = index
        .search("gamma", 1, 0.0)
        .await
        .expect("gamma search should succeed");
    assert_eq!(gamma_results[0].doc_uri, uri("gamma.md"));
}
