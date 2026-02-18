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
mod tests;
