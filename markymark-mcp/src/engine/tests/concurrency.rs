use super::*;
use async_trait::async_trait;
use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_core::prelude::{EmbedError, EmbeddingProvider};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Slow embedding provider that simulates a Voyage HTTP round-trip.
///
/// `embed()` signals `embed_started` before sleeping, then delegates to
/// the inner hash-based provider for deterministic output.
struct SlowEmbeddingProvider {
    inner: HashEmbeddingProvider,
    delay: Duration,
    embed_started: Arc<tokio::sync::Notify>,
}

impl SlowEmbeddingProvider {
    fn new(dims: u32, delay: Duration) -> (Self, Arc<tokio::sync::Notify>) {
        let notify = Arc::new(tokio::sync::Notify::new());
        let provider = Self {
            inner: HashEmbeddingProvider::new(dims),
            delay,
            embed_started: Arc::clone(&notify),
        };
        (provider, notify)
    }
}

#[async_trait]
impl EmbeddingProvider for SlowEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        self.embed_started.notify_one();
        tokio::time::sleep(self.delay).await;
        self.inner.embed(text).await
    }

    fn dimensions(&self) -> u32 {
        self.inner.dimensions()
    }
}

/// Semantic search with a slow provider must not block concurrent write
/// operations on the realm state.
///
/// This test:
/// 1. Creates an engine with a slow embedding provider (150ms per embed).
/// 2. Spawns a SemanticSearch that will hold the inner Mutex for ~150ms.
/// 3. Concurrently runs a CreateRealm write operation.
/// 4. Asserts CreateRealm completes quickly (well under the search time).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn semantic_search_does_not_block_realm_writes() {
    let delay = Duration::from_millis(150);
    let (slow_provider, embed_started) = SlowEmbeddingProvider::new(32, delay);
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(slow_provider);

    let dir = make_temp_realm_dir("concurrency");
    fs::write(
        dir.path().join("doc.md"),
        "# Hello World\n\nSome content.\n",
    )
    .unwrap();

    let engine = Arc::new(
        RuntimeEngine::from_workspace_roots_with_provider(
            vec![dir.path().to_path_buf()],
            Some(provider),
        )
        .await
        .unwrap(),
    );

    // Spawn a slow semantic search in the background.
    let engine_search = Arc::clone(&engine);
    let search_handle = tokio::spawn(async move {
        engine_search
            .execute(CoreOperation::SemanticSearch {
                query: "hello".to_string(),
                realm: None,
                top_k: 5,
                min_score: 0.0,
            })
            .await
    });

    // Wait until the search task has entered embed() — deterministic sync.
    embed_started.notified().await;

    // Now run a write operation concurrently — it should NOT be blocked
    // by the search because the outer RwLock is released before search.
    let engine_write = Arc::clone(&engine);
    let write_start = Instant::now();
    let write_result = engine_write
        .execute(CoreOperation::CreateRealm {
            name: "write-test".to_string(),
        })
        .await;
    let write_elapsed = write_start.elapsed();

    // The write operation should complete in well under the search delay.
    // If the old lock-contention bug exists, this would take >=150ms.
    assert!(
        write_elapsed < Duration::from_millis(100),
        "CreateRealm took {write_elapsed:?}, expected <100ms — outer read lock may still be held across search",
    );

    assert!(
        matches!(write_result, CoreOperationResult::RealmInfo { .. }),
        "CreateRealm should succeed, got {write_result:?}"
    );

    // Let the search complete and verify it worked.
    let search_result = search_handle.await.expect("search task should not panic");
    assert!(
        matches!(search_result, CoreOperationResult::SemanticMatches(_)),
        "SemanticSearch should succeed, got {search_result:?}"
    );
}

/// AddRoot with a slow embedding provider must not block concurrent write
/// operations on the realm state.
///
/// This test:
/// 1. Creates an engine with a slow embedding provider (200ms per embed).
/// 2. Creates a realm, then spawns an AddRoot for a dir with markdown files.
/// 3. Waits until the embedding provider has started (deterministic sync).
/// 4. Concurrently runs a CreateRealm write operation.
/// 5. Asserts CreateRealm completes quickly (well under the embedding delay).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_root_does_not_block_realm_writes() {
    let delay = Duration::from_millis(200);
    let (slow_provider, embed_started) = SlowEmbeddingProvider::new(32, delay);
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(slow_provider);

    // Build an engine with the slow provider and a "default" realm (no roots yet).
    let engine = Arc::new(RuntimeEngine {
        state: tokio::sync::RwLock::new({
            let mut map = std::collections::HashMap::new();
            map.insert(
                "default".to_string(),
                RealmData::new(Some(Arc::clone(&provider))),
            );
            map
        }),
        provider: Some(provider),
    });

    // Create a temp dir with a markdown file that has a heading (triggers embedding).
    let dir = make_temp_realm_dir("add-root-concurrency");
    fs::write(
        dir.path().join("doc.md"),
        "# Slow Embedding Test\n\nSome content.\n",
    )
    .unwrap();

    // Spawn AddRoot in the background (will be slow due to embedding).
    let engine_add = Arc::clone(&engine);
    let root_path = dir.path().to_path_buf();
    let add_root_handle = tokio::spawn(async move {
        engine_add
            .execute(CoreOperation::AddRoot {
                realm: "default".to_string(),
                root: root_path,
            })
            .await
    });

    // Wait until the embedding provider has been called — deterministic sync.
    embed_started.notified().await;

    // Now run a write operation concurrently — it should NOT be blocked.
    let engine_write = Arc::clone(&engine);
    let write_start = Instant::now();
    let write_result = engine_write
        .execute(CoreOperation::CreateRealm {
            name: "add-root-write-test".to_string(),
        })
        .await;
    let write_elapsed = write_start.elapsed();

    // The write operation should complete in well under the embedding delay.
    // If the write lock is held across indexing, this would take >=200ms.
    assert!(
        write_elapsed < Duration::from_millis(100),
        "CreateRealm took {write_elapsed:?}, expected <100ms — write lock may be held across AddRoot indexing",
    );

    assert!(
        matches!(write_result, CoreOperationResult::RealmInfo { .. }),
        "CreateRealm should succeed, got {write_result:?}"
    );

    // Let AddRoot complete and verify it succeeded.
    let add_root_result = add_root_handle
        .await
        .expect("add_root task should not panic");
    assert!(
        matches!(add_root_result, CoreOperationResult::RealmInfo { .. }),
        "AddRoot should succeed, got {add_root_result:?}"
    );
}

/// RemoveRoot must complete while a concurrent semantic search is running.
///
/// This specifically guards against calling `blocking_lock()` from async
/// context inside RealmIndex::remove_document.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remove_root_does_not_deadlock_during_search() {
    let delay = Duration::from_millis(200);
    let (slow_provider, embed_started) = SlowEmbeddingProvider::new(32, delay);
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(slow_provider);

    let engine = Arc::new(RuntimeEngine {
        state: tokio::sync::RwLock::new({
            let mut map = std::collections::HashMap::new();
            map.insert(
                "default".to_string(),
                RealmData::new(Some(Arc::clone(&provider))),
            );
            map
        }),
        provider: Some(provider),
    });

    let dir = make_temp_realm_dir("remove-root-concurrency");
    fs::write(
        dir.path().join("doc.md"),
        "# Slow Embedding Test\n\nSome content.\n",
    )
    .unwrap();
    let root = dir.path().to_path_buf();

    let add_result = engine
        .execute(CoreOperation::AddRoot {
            realm: "default".to_string(),
            root: root.clone(),
        })
        .await;
    assert!(
        matches!(add_result, CoreOperationResult::RealmInfo { .. }),
        "AddRoot setup should succeed, got {add_result:?}"
    );

    // Drain setup embed notification from AddRoot.
    embed_started.notified().await;

    let engine_search = Arc::clone(&engine);
    let search_handle = tokio::spawn(async move {
        engine_search
            .execute(CoreOperation::SemanticSearch {
                query: "slow".to_string(),
                realm: None,
                top_k: 5,
                min_score: 0.0,
            })
            .await
    });

    // Wait until SemanticSearch entered embed().
    embed_started.notified().await;

    let engine_remove = Arc::clone(&engine);
    let remove_result = tokio::time::timeout(
        Duration::from_secs(2),
        engine_remove.execute(CoreOperation::RemoveRoot {
            realm: "default".to_string(),
            root,
        }),
    )
    .await
    .expect("RemoveRoot timed out while semantic search was running");

    assert!(
        matches!(
            remove_result,
            CoreOperationResult::RealmInfo {
                root_count: 0,
                document_count: 0,
                ..
            }
        ),
        "RemoveRoot should succeed, got {remove_result:?}"
    );

    let search_result = search_handle.await.expect("search task should not panic");
    assert!(
        matches!(search_result, CoreOperationResult::SemanticMatches(_)),
        "SemanticSearch should succeed, got {search_result:?}"
    );
}

/// RealmStats(check_duplicates=true) must complete while a concurrent
/// semantic search is running.
///
/// This specifically guards against calling `blocking_lock()` from async
/// context inside RealmIndex::detect_semantic_duplicates.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn realm_stats_does_not_deadlock_during_search() {
    let delay = Duration::from_millis(200);
    let (slow_provider, embed_started) = SlowEmbeddingProvider::new(32, delay);
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(slow_provider);

    let engine = Arc::new(RuntimeEngine {
        state: tokio::sync::RwLock::new({
            let mut map = std::collections::HashMap::new();
            map.insert(
                "default".to_string(),
                RealmData::new(Some(Arc::clone(&provider))),
            );
            map
        }),
        provider: Some(provider),
    });

    let dir = make_temp_realm_dir("realm-stats-concurrency");
    fs::write(dir.path().join("doc.md"), "# Alpha\n\nsemantic content\n").unwrap();
    let add_result = engine
        .execute(CoreOperation::AddRoot {
            realm: "default".to_string(),
            root: dir.path().to_path_buf(),
        })
        .await;
    assert!(
        matches!(add_result, CoreOperationResult::RealmInfo { .. }),
        "AddRoot setup should succeed, got {add_result:?}"
    );

    // Drain setup embed notification from AddRoot.
    embed_started.notified().await;

    let engine_search = Arc::clone(&engine);
    let search_handle = tokio::spawn(async move {
        engine_search
            .execute(CoreOperation::SemanticSearch {
                query: "semantic".to_string(),
                realm: None,
                top_k: 5,
                min_score: 0.0,
            })
            .await
    });

    // Wait until SemanticSearch entered embed().
    embed_started.notified().await;

    let engine_stats = Arc::clone(&engine);
    let stats_result = tokio::time::timeout(
        Duration::from_secs(2),
        engine_stats.execute(CoreOperation::RealmStats {
            realm: "default".to_string(),
            check_duplicates: true,
            include_token_counts: false,
        }),
    )
    .await
    .expect("RealmStats timed out while semantic search was running");

    match stats_result {
        CoreOperationResult::RealmStats {
            duplicate_pairs, ..
        } => {
            assert!(
                duplicate_pairs.is_some(),
                "duplicate_pairs should be present when check_duplicates=true"
            );
        }
        other => panic!("RealmStats should succeed, got {other:?}"),
    }

    let search_result = search_handle.await.expect("search task should not panic");
    assert!(
        matches!(search_result, CoreOperationResult::SemanticMatches(_)),
        "SemanticSearch should succeed, got {search_result:?}"
    );
}
