//! Regex pattern search across workspace files.
//!
//! Implements `search-for-pattern`: iterates all indexed documents in a realm,
//! reads each from disk, applies a compiled regex, and returns matches with
//! optional context lines and glob file filtering.

use std::path::Path;

use markymark_core::engine::{CoreOperationResult, PatternMatch};
use markymark_core::CoreError;
use markymark_index::RealmIndex;

/// Maximum file size to read (10 MiB). Larger files are skipped.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum allowed `context_lines` value. Larger values are clamped.
const MAX_CONTEXT_LINES: u32 = 20;

/// Maximum allowed `limit` value. Larger values are clamped.
const MAX_LIMIT: u32 = 500;

/// Execute a regex pattern search across all indexed files in `realm`.
///
/// Returns `CoreOperationResult::PatternSearchResults` on success or
/// `CoreOperationResult::Error` when `pattern` is invalid.
pub fn execute_search_for_pattern(
    realm_key: &str,
    realm: &RealmIndex,
    pattern: &str,
    include_glob: Option<&str>,
    context_lines: u32,
    limit: u32,
    case_insensitive: bool,
) -> CoreOperationResult {
    // 1. Validate pattern — empty/whitespace is rejected (would match everywhere).
    if pattern.trim().is_empty() {
        return CoreOperationResult::Error(CoreError::Message(
            "invalid_pattern: pattern must not be empty".to_string(),
        ));
    }

    // 2. Compile regex with an explicit DFA size limit to bound execution time.
    let re = match regex::RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .size_limit(1_000_000)
        .build()
    {
        Ok(re) => re,
        Err(err) => {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "invalid_pattern: {err}"
            )));
        }
    };

    // 3. Clamp parameters to safe bounds.
    let context_lines = context_lines.min(MAX_CONTEXT_LINES);
    let effective_limit = limit.clamp(1, MAX_LIMIT) as usize;

    // 4. Compile glob filter (optional).
    let glob_filter = include_glob.and_then(compile_glob);

    // 5. Collect URIs in deterministic order.
    let mut uris: Vec<_> = realm
        .iter_all_documents()
        .map(|(uri, _)| uri.clone())
        .collect();
    uris.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    // 6. Search each file.
    let mut all_matches: Vec<PatternMatch> = Vec::new();
    let mut files_searched: u32 = 0;
    let mut files_skipped: u32 = 0;
    let mut truncated = false;

    'outer: for uri in &uris {
        let Some(path) = uri.to_file_path() else {
            files_skipped += 1;
            continue;
        };

        // Glob filter: if provided, skip files that don't match.
        if let Some(ref gf) = glob_filter {
            if !glob_matches_path(gf, &path) {
                // filtered out — don't count as searched or skipped
                continue;
            }
        }

        // Size check: skip files that are too large or unreadable metadata.
        match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_FILE_SIZE => {
                files_skipped += 1;
                continue;
            }
            Err(_) => {
                files_skipped += 1;
                continue;
            }
            Ok(_) => {}
        }

        // Read file content: skip on any error (deleted, non-UTF-8, permission).
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                files_skipped += 1;
                continue;
            }
        };

        files_searched += 1;

        // Strip trailing line endings before splitting to avoid a phantom empty line
        // at the end of files that end with '\n' or '\r\n'.
        let content_trimmed = content.trim_end_matches(['\n', '\r']);
        // Split into lines; each raw line may still carry a trailing '\r' for CRLF files.
        let lines: Vec<&str> = content_trimmed.split('\n').collect();

        // Find all matches in this file.
        for (line_idx, raw_line) in lines.iter().enumerate() {
            let line = raw_line.trim_end_matches('\r');
            for mat in re.find_iter(line) {
                let m = build_match(
                    uri.clone(),
                    line_idx as u32,
                    mat.start() as u32,
                    mat.as_str(),
                    line,
                    &lines,
                    context_lines,
                );
                all_matches.push(m);

                if all_matches.len() >= effective_limit {
                    truncated = true;
                    break 'outer;
                }
            }
        }
    }

    CoreOperationResult::PatternSearchResults {
        realm: realm_key.to_string(),
        pattern: pattern.to_string(),
        files_searched,
        files_skipped,
        matches: all_matches,
        truncated,
    }
}

/// Constructs a `PatternMatch` for a single regex hit.
fn build_match(
    uri: markymark_core::DocumentUri,
    line_idx: u32,
    col: u32,
    match_text: &str,
    line_text: &str,
    all_lines: &[&str],
    context_lines: u32,
) -> PatternMatch {
    let n_lines = all_lines.len() as u32;
    let ctx_start = line_idx.saturating_sub(context_lines);
    let ctx_end = (line_idx + context_lines + 1).min(n_lines);

    let context_before: Vec<String> = (ctx_start..line_idx)
        .map(|i| all_lines[i as usize].trim_end_matches('\r').to_string())
        .collect();

    let context_after: Vec<String> = ((line_idx + 1)..ctx_end)
        .map(|i| all_lines[i as usize].trim_end_matches('\r').to_string())
        .collect();

    PatternMatch {
        uri,
        line: line_idx,
        column: col,
        match_text: match_text.to_string(),
        line_text: line_text.to_string(),
        context_before,
        context_after,
        context_start_line: ctx_start,
    }
}

/// A compiled glob filter: a flag indicating whether to match the full path or
/// filename only, plus the compiled regex.
struct GlobFilter {
    /// `true` → match against full file path; `false` → match against filename only.
    full_path: bool,
    re: regex::Regex,
}

/// Compile a glob pattern into a `GlobFilter`.
///
/// Returns `None` if the resulting regex is invalid (shouldn't happen for
/// well-formed globs, but we handle it gracefully).
fn compile_glob(glob: &str) -> Option<GlobFilter> {
    let full_path = glob.contains('/');
    let regex_str = glob_to_regex(glob);
    let re = regex::Regex::new(&regex_str).ok()?;
    Some(GlobFilter { full_path, re })
}

/// Test whether `path` matches the compiled `GlobFilter`.
fn glob_matches_path(gf: &GlobFilter, path: &Path) -> bool {
    if gf.full_path {
        let s = path.to_string_lossy();
        // Normalise backslashes (Windows).
        let s = s.replace('\\', "/");
        gf.re.is_match(&s)
    } else {
        path.file_name()
            .and_then(|f| f.to_str())
            .map(|f| gf.re.is_match(f))
            .unwrap_or(false)
    }
}

/// Convert a glob pattern to a regex string.
///
/// Rules:
/// - `**` followed by `/` → `(.*[/])?`  (optional path prefix, any depth)
/// - `**` at end           → `.*`        (match anything remaining)
/// - `*`                   → `[^/]*`     (match within one path component)
/// - `?`                   → `[^/]`      (match one non-separator character)
/// - All other regex metacharacters are escaped.
fn glob_to_regex(glob: &str) -> String {
    let mut result = String::from("^");
    let mut chars = glob.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '*' {
            if chars.peek() == Some(&'*') {
                chars.next(); // consume the second `*`
                if chars.peek() == Some(&'/') {
                    chars.next(); // consume the `/` after `**`
                    result.push_str("(.*[/])?");
                } else {
                    result.push_str(".*");
                }
            } else {
                result.push_str("[^/]*");
            }
        } else if c == '?' {
            result.push_str("[^/]");
        } else if matches!(
            c,
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\'
        ) {
            result.push('\\');
            result.push(c);
        } else {
            result.push(c);
        }
    }

    result.push('$');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use markymark_core::engine::CoreOperation;
    use std::fs;

    // ---- helpers ----

    fn temp_dir(suffix: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("marky-pattern-{}-{}", suffix, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Build a RuntimeEngine with a named realm rooted at `dir`.
    fn make_engine_with_realm(realm: &str, dir: &std::path::Path) -> crate::engine::RuntimeEngine {
        let engine = crate::engine::RuntimeEngine::default();
        engine.execute(CoreOperation::CreateRealm {
            name: realm.to_string(),
        });
        engine.execute(CoreOperation::AddRoot {
            realm: realm.to_string(),
            root: dir.to_path_buf(),
        });
        engine
    }

    use markymark_core::engine::CoreEngine;

    // ---- T1: no results for non-matching pattern ----
    #[test]
    fn no_results_for_non_matching_pattern() {
        let dir = temp_dir("t1");
        fs::write(dir.join("a.md"), "Hello world\n").unwrap();
        let engine = make_engine_with_realm("t1", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "xyzzy_not_present".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t1".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults {
            matches, truncated, ..
        } = result
        {
            assert!(matches.is_empty(), "expected no matches");
            assert!(!truncated);
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T2: literal pattern finds exact match ----
    #[test]
    fn finds_literal_pattern() {
        let dir = temp_dir("t2");
        fs::write(dir.join("a.md"), "# Hello\n\nworld\n").unwrap();
        let engine = make_engine_with_realm("t2", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "Hello".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t2".to_string()),
        });

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

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T3: regex pattern matches function definitions ----
    #[test]
    fn regex_pattern_finds_matches() {
        let dir = temp_dir("t3");
        fs::write(dir.join("code.md"), "```\nfn foo() {}\nfn bar() {}\n```\n").unwrap();
        let engine = make_engine_with_realm("t3", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: r"fn \w+".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t3".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 2, "expected 2 fn matches");
            assert!(matches[0].match_text.starts_with("fn "));
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T4: glob filter *.md only returns markdown files ----
    #[test]
    fn glob_filter_md_only() {
        let dir = temp_dir("t4");
        fs::write(dir.join("notes.md"), "target line\n").unwrap();
        fs::write(dir.join("config.json"), "target line\n").unwrap();
        let engine = make_engine_with_realm("t4", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "target line".to_string(),
            include_glob: Some("*.md".to_string()),
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t4".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 1, "expected only the .md match");
            assert!(matches[0].uri.as_str().ends_with(".md"));
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T5: glob filter *.json excludes markdown ----
    #[test]
    fn glob_filter_json_excludes_md() {
        let dir = temp_dir("t5");
        fs::write(dir.join("notes.md"), "target\n").unwrap();
        fs::write(dir.join("data.json"), "{\"key\": \"target\"}\n").unwrap();
        let engine = make_engine_with_realm("t5", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "target".to_string(),
            include_glob: Some("*.json".to_string()),
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t5".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 1);
            assert!(matches[0].uri.as_str().ends_with(".json"));
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T6: context_lines returns lines before and after ----
    #[test]
    fn context_lines_returned() {
        let dir = temp_dir("t6");
        fs::write(dir.join("a.md"), "line0\nline1\nMATCH\nline3\nline4\n").unwrap();
        let engine = make_engine_with_realm("t6", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 1,
            limit: 100,
            case_insensitive: false,
            realm: Some("t6".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 1);
            let m = &matches[0];
            assert_eq!(m.context_before, vec!["line1".to_string()]);
            assert_eq!(m.context_after, vec!["line3".to_string()]);
            assert_eq!(m.context_start_line, 1);
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T7: limit caps total matches with early exit ----
    #[test]
    fn limit_caps_total_matches() {
        let dir = temp_dir("t7");
        // Write a file with 20 matching lines
        let content: String = (0..20).map(|i| format!("line{i} MATCH\n")).collect();
        fs::write(dir.join("a.md"), content).unwrap();
        let engine = make_engine_with_realm("t7", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 5,
            case_insensitive: false,
            realm: Some("t7".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults {
            matches, truncated, ..
        } = result
        {
            assert_eq!(matches.len(), 5, "expected exactly 5 matches");
            assert!(truncated, "expected truncated=true");
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T8: invalid regex returns error ----
    #[test]
    fn invalid_regex_returns_error() {
        let dir = temp_dir("t8");
        fs::write(dir.join("a.md"), "text\n").unwrap();
        let engine = make_engine_with_realm("t8", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "[unclosed".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t8".to_string()),
        });

        assert!(
            matches!(result, CoreOperationResult::Error(_)),
            "expected Error for invalid regex"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T9: multiple matches in one document ----
    #[test]
    fn multiple_matches_in_one_file() {
        let dir = temp_dir("t9");
        fs::write(dir.join("a.md"), "foo\nbar\nfoo\n").unwrap();
        let engine = make_engine_with_realm("t9", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "foo".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t9".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 2);
            assert_eq!(matches[0].line, 0);
            assert_eq!(matches[1].line, 2);
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T10: 0-based line and column are correct ----
    #[test]
    fn line_and_column_numbers_are_zero_based() {
        let dir = temp_dir("t10");
        fs::write(dir.join("a.md"), "first\nsecond target\nthird\n").unwrap();
        let engine = make_engine_with_realm("t10", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "target".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t10".to_string()),
        });

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

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T11: rejects empty pattern ----
    #[test]
    fn rejects_empty_pattern() {
        let dir = temp_dir("t11");
        fs::write(dir.join("a.md"), "text\n").unwrap();
        let engine = make_engine_with_realm("t11", &dir);

        for p in ["", "   "] {
            let result = engine.execute(CoreOperation::SearchForPattern {
                pattern: p.to_string(),
                include_glob: None,
                context_lines: 0,
                limit: 100,
                case_insensitive: false,
                realm: Some("t11".to_string()),
            });
            assert!(
                matches!(result, CoreOperationResult::Error(_)),
                "expected Error for pattern {:?}",
                p
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T12: handles deleted/missing file gracefully ----
    #[test]
    fn handles_missing_file_gracefully() {
        let dir = temp_dir("t12");
        fs::write(dir.join("a.md"), "text\n").unwrap();
        let engine = make_engine_with_realm("t12", &dir);
        // Delete file after indexing
        fs::remove_file(dir.join("a.md")).unwrap();

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "text".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t12".to_string()),
        });

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

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T13: context_lines clamped to MAX_CONTEXT_LINES ----
    #[test]
    fn context_lines_clamped_to_max() {
        let dir = temp_dir("t13");
        // File has only 3 lines; huge context_lines should not panic
        fs::write(dir.join("a.md"), "line0\nMATCH\nline2\n").unwrap();
        let engine = make_engine_with_realm("t13", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 10000,
            limit: 100,
            case_insensitive: false,
            realm: Some("t13".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 1);
            // context_before should only go back to line 0 (not negative)
            assert_eq!(matches[0].context_before, vec!["line0".to_string()]);
            assert_eq!(matches[0].context_after, vec!["line2".to_string()]);
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T14: context at first line of file ----
    #[test]
    fn context_at_file_start() {
        let dir = temp_dir("t14");
        fs::write(dir.join("a.md"), "MATCH\nline1\nline2\n").unwrap();
        let engine = make_engine_with_realm("t14", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 2,
            limit: 100,
            case_insensitive: false,
            realm: Some("t14".to_string()),
        });

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

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T15: glob with ** matches nested paths ----
    #[test]
    fn glob_double_star_matches_nested_paths() {
        let dir = temp_dir("t15");
        let sub = dir.join("docs").join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("file.md"), "needle\n").unwrap();
        fs::write(dir.join("root.json"), "needle\n").unwrap();
        let engine = make_engine_with_realm("t15", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "needle".to_string(),
            include_glob: Some("**/*.md".to_string()),
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t15".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 1);
            assert!(matches[0].uri.as_str().ends_with(".md"));
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T16: case_insensitive flag works ----
    #[test]
    fn case_insensitive_flag() {
        let dir = temp_dir("t16");
        fs::write(dir.join("a.md"), "HELLO world\n").unwrap();
        let engine = make_engine_with_realm("t16", &dir);

        // case-sensitive: no match
        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "hello".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t16".to_string()),
        });
        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert!(
                matches.is_empty(),
                "case-sensitive should not match HELLO with 'hello'"
            );
        }

        // case-insensitive: should match
        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "hello".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: true,
            realm: Some("t16".to_string()),
        });
        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 1);
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T17: deterministic ordering across files ----
    #[test]
    fn deterministic_ordering_across_files() {
        let dir = temp_dir("t17");
        fs::write(dir.join("b.md"), "match\n").unwrap();
        fs::write(dir.join("a.md"), "match\n").unwrap();
        let engine = make_engine_with_realm("t17", &dir);

        let result1 = engine.execute(CoreOperation::SearchForPattern {
            pattern: "match".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t17".to_string()),
        });
        let result2 = engine.execute(CoreOperation::SearchForPattern {
            pattern: "match".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t17".to_string()),
        });

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

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T18: multiple matches on same line ----
    #[test]
    fn multiple_matches_on_same_line() {
        let dir = temp_dir("t18");
        fs::write(dir.join("a.md"), "aaa bbb aaa\n").unwrap();
        let engine = make_engine_with_realm("t18", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "aaa".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t18".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 2, "two 'aaa' matches on same line");
            assert_eq!(matches[0].column, 0);
            assert_eq!(matches[1].column, 8);
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T19: zero results is not an error ----
    #[test]
    fn zero_results_is_not_error() {
        let dir = temp_dir("t19");
        fs::write(dir.join("a.md"), "hello\n").unwrap();
        let engine = make_engine_with_realm("t19", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "zzz".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t19".to_string()),
        });

        assert!(
            matches!(result, CoreOperationResult::PatternSearchResults { .. }),
            "zero results should be PatternSearchResults, not Error"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T20: search includes structured (non-markdown) documents ----
    #[test]
    fn search_includes_structured_documents() {
        let dir = temp_dir("t20");
        fs::write(dir.join("data.json"), "{\"key\": \"needle\"}\n").unwrap();
        let engine = make_engine_with_realm("t20", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "needle".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t20".to_string()),
        });

        if let CoreOperationResult::PatternSearchResults { matches, .. } = result {
            assert_eq!(matches.len(), 1, "json file should be searchable");
        } else {
            panic!("expected PatternSearchResults");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- T21: CRLF line endings handled correctly ----
    #[test]
    fn crlf_line_endings_handled() {
        let dir = temp_dir("t21");
        // Write a file with CRLF endings
        let content = "line0\r\nMATCH\r\nline2\r\n";
        fs::write(dir.join("a.md"), content).unwrap();
        let engine = make_engine_with_realm("t21", &dir);

        let result = engine.execute(CoreOperation::SearchForPattern {
            pattern: "MATCH".to_string(),
            include_glob: None,
            context_lines: 0,
            limit: 100,
            case_insensitive: false,
            realm: Some("t21".to_string()),
        });

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

        let _ = fs::remove_dir_all(&dir);
    }

    // ---- unit tests for glob_to_regex ----

    #[test]
    fn glob_to_regex_star_matches_filename() {
        let re_str = glob_to_regex("*.md");
        let re = regex::Regex::new(&re_str).unwrap();
        assert!(re.is_match("notes.md"));
        assert!(re.is_match(".md"));
        assert!(!re.is_match("notes.md.bak"));
        assert!(!re.is_match("dir/notes.md"), "single * should not cross /");
    }

    #[test]
    fn glob_to_regex_double_star_matches_across_dirs() {
        let re_str = glob_to_regex("**/*.md");
        let re = regex::Regex::new(&re_str).unwrap();
        assert!(re.is_match("notes.md"), "** allows empty prefix");
        assert!(re.is_match("docs/notes.md"));
        assert!(re.is_match("a/b/c/notes.md"));
        assert!(!re.is_match("notes.rs"));
    }

    #[test]
    fn glob_to_regex_question_mark_matches_single_char() {
        let re_str = glob_to_regex("?.md");
        let re = regex::Regex::new(&re_str).unwrap();
        assert!(re.is_match("a.md"));
        assert!(!re.is_match("ab.md"), "? matches exactly one char");
    }
}
