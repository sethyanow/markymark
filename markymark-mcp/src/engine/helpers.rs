//! Standalone helper functions extracted from the engine module.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::bail;
use ignore::WalkBuilder;
use markymark_core::structured::DocumentKind;
use markymark_core::DocumentUri;
#[cfg(feature = "semantic-search")]
use markymark_core::Range;
use markymark_index::RealmIndex;
use markymark_kernels::tokens;

/// Directory names that are hard-ignored regardless of user `.gitignore`
/// configuration. Matches build-artefact and vendored-dep conventions common
/// across Rust, Python, JS, and Bazel ecosystems.
const HARD_IGNORE_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
];

/// Count total estimated tokens across all documents in a realm.
pub(crate) fn total_tokens_for_realm(realm: &RealmIndex) -> (u64, usize) {
    let mut total_tokens = 0_u64;
    let mut unreadable_docs = 0_usize;
    for (uri, _) in realm.iter_all_documents() {
        let Some(path) = uri.to_file_path() else {
            unreadable_docs += 1;
            continue;
        };
        match fs::read_to_string(path) {
            Ok(source) => {
                total_tokens += u64::from(tokens::estimate_tokens(&source));
            }
            Err(_) => {
                unreadable_docs += 1;
            }
        }
    }
    (total_tokens, unreadable_docs)
}

#[cfg(feature = "semantic-search")]
pub(crate) fn preview_for_range(uri: &DocumentUri, range: Range, fallback: &str) -> String {
    let Some(path) = uri.to_file_path() else {
        return truncate_preview(fallback);
    };
    let Ok(source) = fs::read_to_string(path) else {
        return truncate_preview(fallback);
    };
    let Some(start_idx) = byte_offset_for_line(&source, range.start.line) else {
        return truncate_preview(fallback);
    };
    truncate_preview(&source[start_idx..])
}

#[cfg(feature = "semantic-search")]
fn byte_offset_for_line(source: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return Some(0);
    }

    let mut current_line = 0_u32;
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            current_line += 1;
            if current_line == line {
                return Some(idx + 1);
            }
        }
    }
    None
}

#[cfg(feature = "semantic-search")]
pub(crate) fn truncate_preview(text: &str) -> String {
    const MAX_PREVIEW_BYTES: usize = 200;
    let mut end = text.len().min(MAX_PREVIEW_BYTES);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Convert a `BlockKind` enum variant to its wire-format string.
pub(crate) fn block_kind_str(kind: &markymark_index::document::BlockKind) -> &'static str {
    use markymark_index::document::BlockKind;
    match kind {
        BlockKind::Paragraph => "paragraph",
        BlockKind::ListItem => "list_item",
        BlockKind::CodeBlock => "code_block",
        BlockKind::BlockQuote => "blockquote",
        BlockKind::ThematicBreak => "thematic_break",
        BlockKind::Table => "table",
    }
}

/// Parse a wire-format block kind string into a `BlockKind` enum variant.
/// Returns `None` for unrecognized kinds (silently filters them out).
pub(crate) fn parse_block_kind(kind: &str) -> Option<markymark_index::document::BlockKind> {
    use markymark_index::document::BlockKind;
    match kind {
        "paragraph" => Some(BlockKind::Paragraph),
        "list_item" => Some(BlockKind::ListItem),
        "code_block" => Some(BlockKind::CodeBlock),
        "blockquote" => Some(BlockKind::BlockQuote),
        "thematic_break" => Some(BlockKind::ThematicBreak),
        "table" => Some(BlockKind::Table),
        _ => None,
    }
}

pub(crate) fn validate_workspace_root(root: &Path) -> anyhow::Result<()> {
    if !root.exists() {
        bail!("workspace root does not exist: {}", root.display());
    }
    if !root.is_dir() {
        bail!("workspace root is not a directory: {}", root.display());
    }
    Ok(())
}

pub(crate) fn collect_documents(root: &Path) -> Vec<(PathBuf, DocumentKind)> {
    let walker = WalkBuilder::new(root)
        .add_custom_ignore_filename(".markymarkignore")
        // Apply .gitignore rules in any markymark workspace, not only inside
        // a git repo. Users expect a .gitignore placed next to their notes
        // to be honoured regardless of whether `git init` has been run.
        .require_git(false)
        // Walk hidden files — `.env` is a first-class `DocumentKind::DotEnv`
        // and the prior `fs::read_dir` walker did not filter hidden entries.
        // Hidden dirs we actually want to skip (`.git/`) are handled by the
        // explicit `HARD_IGNORE_DIRS` filter below.
        .hidden(false)
        // Follow symlinks — Bazel runfiles expose test fixtures as symlinks,
        // and the prior `fs::read_dir` + `Path::is_dir` walker followed links
        // by default. `ignore` has built-in cycle detection (terminates on
        // loops) so we do not need to keep links off for safety.
        .follow_links(true)
        .filter_entry(|entry| {
            // Apply hard-ignore to directories only — a regular file named
            // "target" should not be rejected here; file-kind filtering happens
            // below via DocumentKind::from_path.
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let name = entry.file_name().to_string_lossy();
                if HARD_IGNORE_DIRS.contains(&name.as_ref()) {
                    return false;
                }
                if name.starts_with("bazel-") {
                    return false;
                }
            }
            true
        })
        .build();

    let mut files = Vec::new();
    for entry in walker {
        // Log-and-continue on permission / I/O / loop errors to match the
        // prior fs::read_dir contract (helpers.rs had `Err(_) => continue`).
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.into_path();
        if let Some(kind) = DocumentKind::from_path(&path) {
            files.push((path, kind));
        }
    }

    // ignore::Walk visit order is unspecified; preserve existing sort contract.
    files.sort_by(|(a, _), (b, _)| a.cmp(b));
    files
}

/// Build a dependency graph from the realm's indexed documents.
///
/// Returns the graph as either JSON or DOT format.
pub(crate) fn build_dependency_graph(realm: &RealmIndex, format: &str) -> Result<String, String> {
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};

    // Collect document URIs and outgoing links.
    let mut nodes: BTreeSet<String> = BTreeSet::new();
    let mut edges: Vec<(String, String, String)> = Vec::new();

    for (uri, index) in realm.iter_documents() {
        let from = uri.as_str().to_string();
        nodes.insert(from.clone());

        for wl in index.wiki_links() {
            // Wiki links target page names, not full URIs.
            // Record the target as-is for the graph.
            let to = format!("wiki:{}", wl.target);
            nodes.insert(to.clone());
            edges.push((from.clone(), to, "wiki_link".to_string()));
        }

        for ml in index.markdown_links() {
            if ml.url.starts_with("http://") || ml.url.starts_with("https://") {
                continue; // Skip external URLs.
            }
            let to = ml.url.to_string();
            nodes.insert(to.clone());
            edges.push((from.clone(), to, "markdown_link".to_string()));
        }
    }

    match format {
        "json" => {
            let nodes_json: Vec<_> = nodes.iter().map(|n| json!({ "id": n })).collect();
            let edges_json: Vec<_> = edges
                .iter()
                .map(|(from, to, kind)| json!({ "from": from, "to": to, "kind": kind }))
                .collect();
            let graph = json!({ "nodes": nodes_json, "edges": edges_json });
            serde_json::to_string_pretty(&graph).map_err(|e| e.to_string())
        }
        "dot" => {
            let mut out = String::from("digraph dependency_graph {\n");
            // Assign short labels for readability.
            let label_map: BTreeMap<&str, usize> = nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (n.as_str(), i))
                .collect();
            for (name, idx) in &label_map {
                // Use the file stem or short name as label.
                let label = name.rsplit('/').next().unwrap_or(name);
                let escaped = escape_dot_label(label);
                out.push_str(&format!("  n{idx} [label=\"{escaped}\"];\n"));
            }
            for (from, to, kind) in &edges {
                let from_idx = label_map[from.as_str()];
                let to_idx = label_map[to.as_str()];
                let kind_escaped = escape_dot_label(kind);
                out.push_str(&format!(
                    "  n{from_idx} -> n{to_idx} [label=\"{kind_escaped}\"];\n"
                ));
            }
            out.push_str("}\n");
            Ok(out)
        }
        other => Err(format!("unsupported dependency graph format: {other}")),
    }
}

/// Escape a string for use as a DOT (Graphviz) label value.
///
/// DOT requires backslashes, double-quotes, and newlines to be escaped
/// inside quoted label strings.
fn escape_dot_label(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Resolve a local markdown link URL to a URI string.
///
/// Takes the source document's URI and the raw link URL (e.g. `../other/file.md`),
/// resolves the path relative to the source's parent directory, normalizes it,
/// and returns the target URI string if it exists in `known_uris`.
///
/// Wiki links should NOT use this function — they use stem-based resolution by design.
pub(crate) fn resolve_markdown_link(
    source_uri: &DocumentUri,
    link_url: &str,
    known_uris: &std::collections::HashSet<String>,
) -> Option<String> {
    let url_path = link_url.split('#').next().unwrap_or(link_url);
    if url_path.is_empty() {
        return None;
    }
    let source_path = source_uri.to_file_path()?;
    let parent = source_path.parent()?;
    let target_path = parent.join(url_path);
    let normalized = normalize_path(&target_path);
    let target_uri = DocumentUri::from_file_path(&normalized);
    let target_str = target_uri.as_str().to_string();
    if known_uris.contains(&target_str) {
        Some(target_str)
    } else {
        None
    }
}

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_dot_label_plain_text() {
        assert_eq!(escape_dot_label("hello.md"), "hello.md");
    }

    #[test]
    fn escape_dot_label_with_quotes() {
        assert_eq!(escape_dot_label(r#"say "hi""#), r#"say \"hi\""#);
    }

    #[test]
    fn escape_dot_label_with_backslash() {
        assert_eq!(escape_dot_label(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn escape_dot_label_with_newline() {
        assert_eq!(escape_dot_label("line1\nline2"), "line1\\nline2");
    }
}
