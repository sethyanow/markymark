use std::fs;

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use markymark_mcp::RuntimeEngine;

use super::TempWorkspace;

#[tokio::test]
async fn search_symbols_prefers_prefix_over_plain_substring() {
    let ws = TempWorkspace::new("search-symbols-fuzzy-prefix");
    let first = ws.root().join("a.md");
    let second = ws.root().join("b.md");

    fs::write(&first, "# setup\n# stage\n").expect("first markdown should be created");
    fs::write(&second, "# close\n").expect("second markdown should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let symbols = engine
        .execute(CoreOperation::SearchSymbols {
            query: "st".to_string(),
            realm: None,
        })
        .await;

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(names, vec!["stage".to_string(), "setup".to_string()]);
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[tokio::test]
async fn search_symbols_matches_case_insensitively() {
    let ws = TempWorkspace::new("search-symbols-fuzzy-case");
    let file = ws.root().join("case.md");
    fs::write(&file, "# Setup\n# stage\n").expect("markdown should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let symbols = engine
        .execute(CoreOperation::SearchSymbols {
            query: "ST".to_string(),
            realm: None,
        })
        .await;

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(names, vec!["stage".to_string(), "Setup".to_string()]);
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[tokio::test]
async fn search_symbols_supports_subsequence_matching() {
    let ws = TempWorkspace::new("search-symbols-fuzzy-subseq");
    let file = ws.root().join("subseq.md");
    fs::write(&file, "# setup\n# stop\n").expect("markdown should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let symbols = engine
        .execute(CoreOperation::SearchSymbols {
            query: "stp".to_string(),
            realm: None,
        })
        .await;

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(names, vec!["stop".to_string(), "setup".to_string()]);
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[tokio::test]
async fn search_symbols_returns_no_results_when_query_cannot_be_matched() {
    let ws = TempWorkspace::new("search-symbols-fuzzy-none");
    let file = ws.root().join("none.md");
    fs::write(&file, "# setup\n# stage\n").expect("markdown should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let symbols = engine
        .execute(CoreOperation::SearchSymbols {
            query: "zzz".to_string(),
            realm: None,
        })
        .await;

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            assert!(matches.is_empty(), "expected no fuzzy matches");
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

#[tokio::test]
async fn search_symbols_uses_batch_ranked_results_ordering() {
    let ws = TempWorkspace::new("search-symbols-batch-ranked-order");
    let file = ws.root().join("ranked.md");
    fs::write(&file, "# acb\n# adb\n# aeb\n").expect("markdown should be created");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let symbols = engine
        .execute(CoreOperation::SearchSymbols {
            query: "ab".to_string(),
            realm: None,
        })
        .await;

    match symbols {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            assert_eq!(
                names,
                vec!["acb".to_string(), "adb".to_string(), "aeb".to_string()]
            );
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}

// Regression test for: fuzzy_match_batch path lacked alphabetical tie-breaking.
// Candidates from multiple files arrive in HashMap iteration order (non-deterministic).
// When all scores are equal, the sort must fall back to name ASC so the result is
// stable regardless of which document the realm iterates first.
// Bug introduced in feat(marky-8xt), fixed by adding the same comparator used in
// the single-candidate fallback path.
#[tokio::test]
async fn search_symbols_batch_path_uses_alphabetical_tiebreak_across_files() {
    let ws = TempWorkspace::new("batch-tiebreak-cross-file");
    // Three files, one heading each. All headings contain 'a', so all score equally
    // for query "a". The correct result is alphabetical regardless of which file the
    // realm's HashMap happens to iterate first.
    fs::write(ws.root().join("c.md"), "# Zebra\n").expect("c.md");
    fs::write(ws.root().join("a.md"), "# Alpha\n").expect("a.md");
    fs::write(ws.root().join("b.md"), "# Beta\n").expect("b.md");

    let engine = RuntimeEngine::from_workspace_roots(vec![ws.root()])
        .await
        .expect("workspace should index");

    let result = engine
        .execute(CoreOperation::SearchSymbols {
            query: "a".to_string(),
            realm: None,
        })
        .await;

    match result {
        CoreOperationResult::Symbols(matches) => {
            let names: Vec<_> = matches.into_iter().map(|(name, _, _)| name).collect();
            // Must be alphabetical: Alpha < Beta < Zebra.
            // Before the fix this returned in HashMap iteration order, e.g. ["Zebra", "Alpha", "Beta"].
            assert_eq!(
                names,
                vec!["Alpha".to_string(), "Beta".to_string(), "Zebra".to_string()],
                "batch path must use alphabetical tiebreak; got {names:?}"
            );
        }
        other => panic!("expected symbol matches, got: {other:?}"),
    }
}
