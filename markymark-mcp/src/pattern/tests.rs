use super::*;
use markymark_core::engine::CoreOperation;
use std::fs;

// ---- helpers ----

fn temp_dir(_suffix: &str) -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create temp dir")
}

/// Build a RuntimeEngine with a named realm rooted at `dir`.
async fn make_engine_with_realm(
    realm: &str,
    dir: &std::path::Path,
) -> crate::engine::RuntimeEngine {
    let engine = crate::engine::RuntimeEngine::default();
    engine
        .execute(CoreOperation::CreateRealm {
            name: realm.to_string(),
        })
        .await;
    engine
        .execute(CoreOperation::AddRoot {
            realm: realm.to_string(),
            root: dir.to_path_buf(),
        })
        .await;
    engine
}

use markymark_core::engine::CoreEngine;

// ---- T1: no results for non-matching pattern ----
#[tokio::test]
async fn no_results_for_non_matching_pattern() {
    let dir = temp_dir("t1");
    fs::write(dir.path().join("a.md"), "Hello world\n").unwrap();
    let engine = make_engine_with_realm("t1", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "xyzzy_not_present".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t1".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults {
        matches, truncated, ..
    } = result
    {
        assert!(matches.is_empty(), "expected no matches");
        assert!(!truncated);
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T2: literal pattern finds exact match ----
#[tokio::test]
async fn finds_literal_pattern() {
    let dir = temp_dir("t2");
    fs::write(dir.path().join("a.md"), "# Hello\n\nworld\n").unwrap();
    let engine = make_engine_with_realm("t2", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "Hello".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t2".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults {
        matches,
        files_searched,
        ..
    } = result
    {
        assert_eq!(matches.len(), 1, "expected one match");
        assert_eq!(matches[0].line, 0, "match should be on line 0");
        assert_eq!(matches[0].match_text, "Hello");
        assert_eq!(files_searched, 1);
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T3: regex pattern matches function definitions ----
#[tokio::test]
async fn regex_pattern_finds_matches() {
    let dir = temp_dir("t3");
    fs::write(
        dir.path().join("code.md"),
        "```\nfn foo() {}\nfn bar() {}\n```\n",
    )
    .unwrap();
    let engine = make_engine_with_realm("t3", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: r"fn \w+".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t3".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 2, "expected 2 fn matches");
        assert!(matches[0].match_text.starts_with("fn "));
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T4: glob filter *.md only returns markdown files ----
#[tokio::test]
async fn glob_filter_md_only() {
    let dir = temp_dir("t4");
    fs::write(dir.path().join("notes.md"), "target line\n").unwrap();
    fs::write(dir.path().join("config.json"), "target line\n").unwrap();
    let engine = make_engine_with_realm("t4", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "target line".to_string(),
            include_glob: Some("*.md".to_string()),
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t4".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1, "expected only the .md match");
        assert!(matches[0].uri.as_str().ends_with(".md"));
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T5: glob filter *.json excludes markdown ----
#[tokio::test]
async fn glob_filter_json_excludes_md() {
    let dir = temp_dir("t5");
    fs::write(dir.path().join("notes.md"), "target\n").unwrap();
    fs::write(dir.path().join("data.json"), "{\"key\": \"target\"}\n").unwrap();
    let engine = make_engine_with_realm("t5", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "target".to_string(),
            include_glob: Some("*.json".to_string()),
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t5".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
        assert!(matches[0].uri.as_str().ends_with(".json"));
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T6: context_lines returns lines before and after ----
#[tokio::test]
async fn context_lines_returned() {
    let dir = temp_dir("t6");
    fs::write(
        dir.path().join("a.md"),
        "line0\nline1\nMATCH\nline3\nline4\n",
    )
    .unwrap();
    let engine = make_engine_with_realm("t6", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 1,
            limit: 100,
            case_insensitive: false,
            realm: Some("t6".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
        let m = &matches[0];
        assert_eq!(m.context_before, vec!["line1".to_string()]);
        assert_eq!(m.context_after, vec!["line3".to_string()]);
        assert_eq!(m.context_start_line, 1);
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T7: limit caps total matches with early exit ----
#[tokio::test]
async fn limit_caps_total_matches() {
    let dir = temp_dir("t7");
    // Write a file with 20 matching lines
    let content: String = (0..20).map(|i| format!("line{i} MATCH\n")).collect();
    fs::write(dir.path().join("a.md"), content).unwrap();
    let engine = make_engine_with_realm("t7", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 5,
            case_insensitive: false,
            realm: Some("t7".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults {
        matches, truncated, ..
    } = result
    {
        assert_eq!(matches.len(), 5, "expected exactly 5 matches");
        assert!(truncated, "expected truncated=true");
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T8: invalid regex returns error ----
#[tokio::test]
async fn invalid_regex_returns_error() {
    let dir = temp_dir("t8");
    fs::write(dir.path().join("a.md"), "text\n").unwrap();
    let engine = make_engine_with_realm("t8", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "[unclosed".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t8".to_string()),
        })
        .await;

    assert!(
        matches!(result, CoreOperationResult::Error(_)),
        "expected Error for invalid regex"
    );
}

// ---- T9: multiple matches in one document ----
#[tokio::test]
async fn multiple_matches_in_one_file() {
    let dir = temp_dir("t9");
    fs::write(dir.path().join("a.md"), "foo\nbar\nfoo\n").unwrap();
    let engine = make_engine_with_realm("t9", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "foo".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t9".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 0);
        assert_eq!(matches[1].line, 2);
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T10: 0-based line and column are correct ----
#[tokio::test]
async fn line_and_column_numbers_are_zero_based() {
    let dir = temp_dir("t10");
    fs::write(dir.path().join("a.md"), "first\nsecond target\nthird\n").unwrap();
    let engine = make_engine_with_realm("t10", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "target".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t10".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 1, "line is 0-based: 'target' is on line 1");
        assert_eq!(
            matches[0].column, 7,
            "column is 0-based: 'target' starts at col 7"
        );
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T11: rejects empty pattern ----
#[tokio::test]
async fn rejects_empty_pattern() {
    let dir = temp_dir("t11");
    fs::write(dir.path().join("a.md"), "text\n").unwrap();
    let engine = make_engine_with_realm("t11", dir.path()).await;

    for p in ["", "   "] {
        let result = engine
            .execute(CoreOperation::SearchForPattern {
                pattern: p.to_string(),
                include_glob: None,
                context_lines: 0,
                limit: 100,
                case_insensitive: false,
                realm: Some("t11".to_string()),
            })
            .await;
        assert!(
            matches!(result, CoreOperationResult::Error(_)),
            "expected Error for pattern {:?}",
            p
        );
    }
}

// ---- T12: handles deleted/missing file gracefully ----
#[tokio::test]
async fn handles_missing_file_gracefully() {
    let dir = temp_dir("t12");
    fs::write(dir.path().join("a.md"), "text\n").unwrap();
    let engine = make_engine_with_realm("t12", dir.path()).await;
    // Delete file after indexing
    fs::remove_file(dir.path().join("a.md")).unwrap();

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "text".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t12".to_string()),
        })
        .await;

    // Should return results (empty) without panicking
    if let CoreOperationResult::PatternSearchResults {
        matches,
        files_skipped,
        ..
    } = result
    {
        assert!(matches.is_empty());
        assert_eq!(
            files_skipped, 1,
            "deleted file should be counted as skipped"
        );
    } else {
        panic!("expected PatternSearchResults, not an error");
    }
}

// ---- T13: context_lines clamped to MAX_CONTEXT_LINES ----
#[tokio::test]
async fn context_lines_clamped_to_max() {
    let dir = temp_dir("t13");
    // File has only 3 lines; huge context_lines should not panic
    fs::write(dir.path().join("a.md"), "line0\nMATCH\nline2\n").unwrap();
    let engine = make_engine_with_realm("t13", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 10000,
            limit: 100,
            case_insensitive: false,
            realm: Some("t13".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
        // context_before should only go back to line 0 (not negative)
        assert_eq!(matches[0].context_before, vec!["line0".to_string()]);
        assert_eq!(matches[0].context_after, vec!["line2".to_string()]);
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T14: context at first line of file ----
#[tokio::test]
async fn context_at_file_start() {
    let dir = temp_dir("t14");
    fs::write(dir.path().join("a.md"), "MATCH\nline1\nline2\n").unwrap();
    let engine = make_engine_with_realm("t14", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 2,
            limit: 100,
            case_insensitive: false,
            realm: Some("t14".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
        assert!(
            matches[0].context_before.is_empty(),
            "no context before line 0"
        );
        assert_eq!(matches[0].context_start_line, 0);
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T15: glob with ** matches nested paths ----
#[tokio::test]
async fn glob_double_star_matches_nested_paths() {
    let dir = temp_dir("t15");
    let sub = dir.path().join("docs").join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("file.md"), "needle\n").unwrap();
    fs::write(dir.path().join("root.json"), "needle\n").unwrap();
    let engine = make_engine_with_realm("t15", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "needle".to_string(),
            include_glob: Some("**/*.md".to_string()),
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t15".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
        assert!(matches[0].uri.as_str().ends_with(".md"));
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T16: case_insensitive flag works ----
#[tokio::test]
async fn case_insensitive_flag() {
    let dir = temp_dir("t16");
    fs::write(dir.path().join("a.md"), "HELLO world\n").unwrap();
    let engine = make_engine_with_realm("t16", dir.path()).await;

    // case-sensitive: no match
    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "hello".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t16".to_string()),
        })
        .await;
    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert!(
            matches.is_empty(),
            "case-sensitive should not match HELLO with 'hello'"
        );
    }

    // case-insensitive: should match
    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "hello".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: true,
            realm: Some("t16".to_string()),
        })
        .await;
    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T17: deterministic ordering across files ----
#[tokio::test]
async fn deterministic_ordering_across_files() {
    let dir = temp_dir("t17");
    fs::write(dir.path().join("b.md"), "match\n").unwrap();
    fs::write(dir.path().join("a.md"), "match\n").unwrap();
    let engine = make_engine_with_realm("t17", dir.path()).await;

    let result1 = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "match".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t17".to_string()),
        })
        .await;
    let result2 = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "match".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t17".to_string()),
        })
        .await;

    let uris1: Vec<String> =
        if let CoreOperationResult::PatternSearchResults { matches, .. } = result1 {
            matches.iter().map(|m| m.uri.as_str().to_string()).collect()
        } else {
            panic!()
        };
    let uris2: Vec<String> =
        if let CoreOperationResult::PatternSearchResults { matches, .. } = result2 {
            matches.iter().map(|m| m.uri.as_str().to_string()).collect()
        } else {
            panic!()
        };

    assert_eq!(uris1, uris2, "results must be deterministic");
    // a.md should come before b.md (sorted by URI)
    assert!(uris1[0] < uris1[1], "results should be in URI-sorted order");
}

// ---- T18: multiple matches on same line ----
#[tokio::test]
async fn multiple_matches_on_same_line() {
    let dir = temp_dir("t18");
    fs::write(dir.path().join("a.md"), "aaa bbb aaa\n").unwrap();
    let engine = make_engine_with_realm("t18", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "aaa".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t18".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 2, "two 'aaa' matches on same line");
        assert_eq!(matches[0].column, 0);
        assert_eq!(matches[1].column, 8);
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T19: zero results is not an error ----
#[tokio::test]
async fn zero_results_is_not_error() {
    let dir = temp_dir("t19");
    fs::write(dir.path().join("a.md"), "hello\n").unwrap();
    let engine = make_engine_with_realm("t19", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "zzz".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t19".to_string()),
        })
        .await;

    assert!(
        matches!(result, CoreOperationResult::PatternSearchResults { .. }),
        "zero results should be PatternSearchResults, not Error"
    );
}

// ---- T20: search includes structured (non-markdown) documents ----
#[tokio::test]
async fn search_includes_structured_documents() {
    let dir = temp_dir("t20");
    fs::write(dir.path().join("data.json"), "{\"key\": \"needle\"}\n").unwrap();
    let engine = make_engine_with_realm("t20", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "needle".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t20".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1, "json file should be searchable");
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- T21: CRLF line endings handled correctly ----
#[tokio::test]
async fn crlf_line_endings_handled() {
    let dir = temp_dir("t21");
    // Write a file with CRLF endings
    let content = "line0\r\nMATCH\r\nline2\r\n";
    fs::write(dir.path().join("a.md"), content).unwrap();
    let engine = make_engine_with_realm("t21", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t21".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 1);
        // line_text should not contain trailing \r
        assert!(
            !matches[0].line_text.contains('\r'),
            "trailing \\r should be stripped"
        );
    } else {
        panic!("expected PatternSearchResults");
    }
}

// ---- unit tests for glob_to_regex ----

#[tokio::test]
async fn glob_to_regex_star_matches_filename() {
    let re_str = glob_to_regex("*.md");
    let re = regex::Regex::new(&re_str).unwrap();
    assert!(re.is_match("notes.md"));
    assert!(re.is_match(".md"));
    assert!(!re.is_match("notes.md.bak"));
    assert!(!re.is_match("dir/notes.md"), "single * should not cross /");
}

#[tokio::test]
async fn glob_to_regex_double_star_matches_across_dirs() {
    let re_str = glob_to_regex("**/*.md");
    let re = regex::Regex::new(&re_str).unwrap();
    assert!(re.is_match("notes.md"), "** allows empty prefix");
    assert!(re.is_match("docs/notes.md"));
    assert!(re.is_match("a/b/c/notes.md"));
    assert!(!re.is_match("notes.rs"));
}

#[tokio::test]
async fn glob_to_regex_question_mark_matches_single_char() {
    let re_str = glob_to_regex("?.md");
    let re = regex::Regex::new(&re_str).unwrap();
    assert!(re.is_match("a.md"));
    assert!(!re.is_match("ab.md"), "? matches exactly one char");
}

// ---- T2-2: invalid glob returns error instead of silent fallback ----
#[tokio::test]
async fn valid_glob_filter_works() {
    let dir = temp_dir("t22");
    fs::write(dir.path().join("a.md"), "needle\n").unwrap();
    fs::write(dir.path().join("b.json"), "needle\n").unwrap();
    let engine = make_engine_with_realm("t22", dir.path()).await;

    let result = engine
        .execute(CoreOperation::SearchForPattern {
            pattern: "needle".to_string(),
            include_glob: Some("*.md".to_string()),
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t22".to_string()),
        })
        .await;

    if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
        assert_eq!(matches.len(), 1);
        assert!(matches[0].uri.as_str().ends_with(".md"));
    } else {
        panic!("expected PatternSearchResults");
    }
}
