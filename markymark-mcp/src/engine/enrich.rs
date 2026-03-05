//! EnrichDocument operation handler.
//!
//! Enriches a document's outline with LLM-generated summaries, stored in
//! sidecar JSON files under `.markymark/` (or a configurable directory).

use std::path::Path;

use markymark_core::engine::{CoreOperationResult, OutlineTreeNode};
use markymark_core::inference::InferenceProvider;
use markymark_core::sidecar::{
    content_hash, sidecar_path, DocumentSidecar, SectionSummary, DEFAULT_SIDECAR_DIR,
};
use markymark_core::{CoreError, DocumentUri};
use markymark_index::RealmIndex;

/// Handle the EnrichDocument operation.
///
/// Reads the document's outline, generates summaries via the inference provider,
/// and stores them in a sidecar JSON file. Skips enrichment if the sidecar is
/// fresh (content hash matches) unless `force` is true.
pub(crate) async fn handle_enrich_document(
    realm: &RealmIndex,
    roots: &[std::path::PathBuf],
    uri: &DocumentUri,
    sidecar_dir_override: Option<&Path>,
    force: bool,
    provider: Option<&dyn InferenceProvider>,
) -> CoreOperationResult {
    // Require an inference provider.
    let Some(provider) = provider else {
        return CoreOperationResult::Error(CoreError::NotImplemented(
            "no inference provider configured — set one to enable enrichment".to_string(),
        ));
    };

    // Resolve the document in the index.
    let Some(markymark_index::AnyDocumentIndex::Markdown(index)) =
        realm.get_any_document(uri)
    else {
        return CoreOperationResult::Error(CoreError::Message(format!(
            "document is not indexed as markdown: {}",
            uri.as_str()
        )));
    };

    // Read the source file.
    let Some(file_path) = uri.to_file_path() else {
        return CoreOperationResult::Error(CoreError::Message(format!(
            "cannot resolve file path for: {}",
            uri.as_str()
        )));
    };
    let source = match std::fs::read_to_string(&file_path) {
        Ok(s) => s,
        Err(e) => {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "cannot read file {}: {e}",
                file_path.display()
            )));
        }
    };

    let current_hash = content_hash(source.as_bytes());

    // Determine sidecar directory.
    // Find the workspace root that contains this file to place the sidecar relative to it.
    let (sidecar_base_dir, relative_doc_path) =
        resolve_sidecar_location(&file_path, sidecar_dir_override, roots);

    let sidecar_file = sidecar_path(&sidecar_base_dir, &relative_doc_path);

    // Check existing sidecar for freshness.
    if !force {
        if let Ok(existing_json) = std::fs::read_to_string(&sidecar_file) {
            if let Ok(existing) = serde_json::from_str::<DocumentSidecar>(&existing_json) {
                if !existing.is_stale(&current_hash) {
                    return CoreOperationResult::EnrichmentResult {
                        uri: uri.clone(),
                        sections_enriched: existing.sections.len(),
                        was_stale: false,
                        model_id: existing.model_id,
                    };
                }
            }
        }
    }

    // Build the outline tree to enumerate sections.
    let headings = index.headings();

    // Collect sections to summarize: (slug, heading_path, level, section_text).
    let sections_to_enrich = collect_sections(headings, &source);

    if sections_to_enrich.is_empty() {
        // No headings → write a minimal sidecar and return.
        let sidecar = DocumentSidecar::new(current_hash, provider.model_id().to_string());
        write_sidecar(&sidecar_file, &sidecar);
        return CoreOperationResult::EnrichmentResult {
            uri: uri.clone(),
            sections_enriched: 0,
            was_stale: true,
            model_id: provider.model_id().to_string(),
        };
    }

    // Build batch of (text, context) pairs for summarization.
    let batch: Vec<(&str, Option<&str>)> = sections_to_enrich
        .iter()
        .map(|(_, heading_path, _, text)| (text.as_str(), Some(heading_path.as_str())))
        .collect();

    let summaries = match provider.summarize_batch(&batch).await {
        Ok(s) => s,
        Err(e) => {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "inference failed: {e}"
            )));
        }
    };

    // Build sidecar.
    let mut sidecar = DocumentSidecar::new(current_hash, provider.model_id().to_string());
    for (i, (slug, heading_path, level, _)) in sections_to_enrich.iter().enumerate() {
        sidecar.sections.push(SectionSummary {
            slug: slug.clone(),
            heading_path: heading_path.clone(),
            level: *level,
            summary: summaries[i].clone(),
        });
    }

    // Generate document-level summary from section summaries.
    let doc_context = sections_to_enrich
        .iter()
        .zip(summaries.iter())
        .map(|((_, path, _, _), summary)| format!("{path}: {summary}"))
        .collect::<Vec<_>>()
        .join("\n");
    if let Ok(doc_summary) = provider
        .summarize(&doc_context, Some("Summarize this document based on its sections"))
        .await
    {
        sidecar.document_summary = Some(doc_summary);
    }

    let sections_count = sidecar.sections.len();
    write_sidecar(&sidecar_file, &sidecar);

    CoreOperationResult::EnrichmentResult {
        uri: uri.clone(),
        sections_enriched: sections_count,
        was_stale: true,
        model_id: provider.model_id().to_string(),
    }
}

/// Collect sections from headings and source text.
///
/// Returns Vec<(slug, heading_path, level, section_text)>.
fn collect_sections(
    headings: &[markymark_index::HeadingEntry<'_>],
    source: &str,
) -> Vec<(String, String, u8, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut sections = Vec::new();

    for (i, heading) in headings.iter().enumerate() {
        let start_line = heading.range.start.line as usize + 1;
        if start_line >= lines.len() {
            continue;
        }

        // Section ends at the next heading or EOF.
        let end_line = headings
            .get(i + 1)
            .map(|h| h.range.start.line as usize)
            .unwrap_or(lines.len());

        let text = lines[start_line..end_line].join("\n");
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue; // Skip empty sections.
        }

        // Build heading path from ancestors.
        let heading_path = build_heading_path(headings, i);

        sections.push((
            heading.slug.to_string(),
            heading_path,
            heading.level,
            trimmed.to_string(),
        ));
    }

    sections
}

/// Build a breadcrumb heading path like "Overview > Getting Started > Installation".
fn build_heading_path(headings: &[markymark_index::HeadingEntry<'_>], index: usize) -> String {
    let target_level = headings[index].level;
    let mut path_parts = vec![headings[index].text.to_string()];

    // Walk backwards to find ancestors.
    let mut current_level = target_level;
    for i in (0..index).rev() {
        if headings[i].level < current_level {
            path_parts.push(headings[i].text.to_string());
            current_level = headings[i].level;
            if current_level == 1 {
                break;
            }
        }
    }

    path_parts.reverse();
    path_parts.join(" > ")
}

/// Resolve the sidecar directory and relative document path.
fn resolve_sidecar_location(
    file_path: &Path,
    sidecar_dir_override: Option<&Path>,
    roots: &[std::path::PathBuf],
) -> (std::path::PathBuf, std::path::PathBuf) {
    if let Some(override_dir) = sidecar_dir_override {
        // Use override dir directly; relative path from the override dir parent.
        let relative = file_path
            .file_name()
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        return (override_dir.to_path_buf(), relative);
    }

    // Find the workspace root containing this file.
    // Walk realm roots to find the best match (longest prefix).
    let file_str = file_path.to_string_lossy();
    let mut best_root: Option<&Path> = None;
    let mut best_len = 0;

    for root in roots {
        let root_str = root.to_string_lossy();
        if file_str.starts_with(root_str.as_ref()) && root_str.len() > best_len {
            best_root = Some(root);
            best_len = root_str.len();
        }
    }

    if let Some(root) = best_root {
        let relative = file_path.strip_prefix(root).unwrap_or(file_path);
        let sidecar_dir = root.join(DEFAULT_SIDECAR_DIR);
        (sidecar_dir, relative.to_path_buf())
    } else {
        // Fallback: use the file's parent directory.
        let parent = file_path.parent().unwrap_or(Path::new("."));
        let sidecar_dir = parent.join(DEFAULT_SIDECAR_DIR);
        let relative = file_path
            .file_name()
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        (sidecar_dir, relative)
    }
}

/// Write sidecar JSON to disk, creating parent directories as needed.
fn write_sidecar(path: &Path, sidecar: &DocumentSidecar) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(sidecar) {
        let _ = std::fs::write(path, json);
    }
}

/// Inject sidecar summaries into an OutlineTreeNode tree.
///
/// Walks the tree and matches nodes by slug to sidecar section summaries.
pub(crate) fn inject_summaries(
    node: &mut OutlineTreeNode,
    sidecar: &DocumentSidecar,
) {
    inject_summaries_recursive(node, sidecar);
}

fn inject_summaries_recursive(
    node: &mut OutlineTreeNode,
    sidecar: &DocumentSidecar,
) {
    // Match by title (case-insensitive) — slugs aren't available on OutlineTreeNode.
    // For the root node (level 0), inject document summary.
    if node.level == 0 {
        if let Some(ref doc_summary) = sidecar.document_summary {
            node.summary = Some(doc_summary.clone());
        }
    } else {
        // Find matching section summary by heading text in heading_path.
        for section in &sidecar.sections {
            if section.level == node.level && heading_path_ends_with(&section.heading_path, &node.title) {
                node.summary = Some(section.summary.clone());
                break;
            }
        }
    }

    for child in &mut node.children {
        inject_summaries_recursive(child, sidecar);
    }
}

/// Check if a heading path ends with the given title.
fn heading_path_ends_with(path: &str, title: &str) -> bool {
    path.ends_with(title)
}
