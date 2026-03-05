//! ExportDocsIndex operation handler.
//!
//! Generates pipe-delimited `docs_index` entries from realm state,
//! matching the format used in CLAUDE.md for ambient doc awareness.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use markymark_core::engine::CoreOperationResult;

use super::RealmData;

/// Build a docs_index export from a realm's indexed documents and roots.
///
/// Each root produces one pipe-delimited entry:
/// ```text
/// [name]|root: ./relative/path|category1:{file1.md,file2.md}|category2:{...}
/// ```
pub(crate) fn handle_export_docs_index(
    realm_data: &RealmData,
    realm_name: String,
    name_override: Option<String>,
) -> CoreOperationResult {
    let mut entries = Vec::new();
    let mut total_doc_count: usize = 0;

    // Sort roots for deterministic output.
    let mut sorted_roots: Vec<&PathBuf> = realm_data.roots.iter().collect();
    sorted_roots.sort();

    for root in &sorted_roots {
        let root_str = root_to_file_uri(root);

        // Collect markdown document URIs belonging to this root.
        // A doc belongs to a root if its URI starts with the root's file:// URI prefix.
        let mut category_files: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut doc_count_for_root: usize = 0;

        for (uri, _) in realm_data.index.iter_documents() {
            let uri_str = uri.as_str();
            if !uri_str.starts_with(&root_str) {
                continue;
            }

            // Get the relative path from root.
            let relative = &uri_str[root_str.len()..].trim_start_matches('/');

            // Only include .md files.
            if !relative.ends_with(".md") {
                continue;
            }

            let rel_path = Path::new(relative);
            let components: Vec<&str> = rel_path
                .components()
                .map(|c| c.as_os_str().to_str().unwrap_or(""))
                .collect();

            let (category, filename) = if components.len() == 1 {
                // File directly in root → "." category
                (".".to_string(), components[0].to_string())
            } else {
                // First directory component is the category.
                // For deeper nesting (a/b/c.md), category is "a", file listed as "b/c.md".
                let cat = components[0].to_string();
                let rest = components[1..].join("/");
                (cat, rest)
            };

            category_files
                .entry(category)
                .or_default()
                .push(filename);
            doc_count_for_root += 1;
        }

        // Skip roots with zero markdown docs.
        if doc_count_for_root == 0 {
            continue;
        }

        // Sort files within each category: _index.md first, then alphabetical.
        for files in category_files.values_mut() {
            files.sort_by(|a, b| {
                let a_is_index = a == "_index.md" || a.ends_with("/_index.md");
                let b_is_index = b == "_index.md" || b.ends_with("/_index.md");
                match (a_is_index, b_is_index) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.cmp(b),
                }
            });
        }

        // Build the entry name.
        let name = match &name_override {
            Some(n) => n.clone(),
            None => root_to_kebab_name(root),
        };

        // Build the pipe-delimited entry.
        let root_display = format!("./{}", root.display());
        let mut parts = vec![format!("[{name}]"), format!("root: {root_display}")];

        for (category, files) in &category_files {
            let file_list = files.join(",");
            parts.push(format!("{category}:{{{file_list}}}"));
        }

        entries.push(parts.join("|"));
        total_doc_count += doc_count_for_root;
    }

    // Count skipped: documents not matching any root.
    let all_doc_count = realm_data.index.iter_documents().count();
    let total_skipped = all_doc_count.saturating_sub(total_doc_count);

    CoreOperationResult::DocsIndexExport {
        realm: realm_name,
        entries,
        doc_count: total_doc_count,
        root_count: sorted_roots.len(),
        skipped_count: total_skipped,
    }
}

/// Convert a root PathBuf to a file:// URI string (without trailing slash).
fn root_to_file_uri(root: &Path) -> String {
    format!("file://{}", root.display())
}

/// Derive a kebab-case name from the last path component of a root.
fn root_to_kebab_name(root: &Path) -> String {
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Convert to kebab-case: replace underscores and spaces with hyphens, lowercase.
    name.replace(['_', ' '], "-").to_lowercase()
}
