//! Diagnostic computation for markdown documents.
//!
//! Checks for broken wiki links, broken markdown link anchors,
//! duplicate heading slugs, and unclosed XML tags.

use std::collections::HashMap;

use markymark_core::{DocumentUri, Range};
use markymark_index::{resolution::{resolve_markdown_link, resolve_wiki_link}, slugify, DocumentIndex, RealmIndex};

/// Severity level for a diagnostic.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticSeverity {
    /// An error (e.g., broken link).
    Error,
    /// A warning (e.g., duplicate slug).
    Warning,
}

/// A diagnostic produced by document analysis.
#[derive(Debug, Clone)]
pub struct MarkyDiagnostic {
    /// Source range of the problem.
    pub range: Range,
    /// Severity level.
    pub severity: DiagnosticSeverity,
    /// Human-readable message.
    pub message: String,
}

/// Compute diagnostics for a document given its index and realm.
///
/// Checks for:
/// - Broken wiki links (target page or heading doesn't exist)
/// - Broken markdown link anchors (heading slug doesn't exist in current doc)
/// - Duplicate heading slugs within the same document
/// - Unclosed XML tags
pub fn compute_diagnostics(
    index: &DocumentIndex,
    realm: &RealmIndex,
    uri: &DocumentUri,
) -> Vec<MarkyDiagnostic> {
    let mut diagnostics = Vec::new();

    // 1. Check wiki links for broken references
    for wl in index.wiki_links() {
        let resolved = resolve_wiki_link(realm, uri, wl.target, wl.heading);
        if resolved.is_none() {
            let target_desc = match &wl.heading {
                Some(h) => format!("{}#{}", wl.target, h),
                None => wl.target.to_string(),
            };
            diagnostics.push(MarkyDiagnostic {
                range: wl.range,
                severity: DiagnosticSeverity::Error,
                message: format!("Broken wiki link: [[{}]]", target_desc),
            });
        }
    }

    // 2. Check markdown link anchors for broken references
    for ml in index.markdown_links() {
        if let Some(anchor) = &ml.anchor {
            let raw_url = ml
                .url
                .strip_suffix(&format!("#{}", anchor))
                .unwrap_or(ml.url);
            let resolved = resolve_markdown_link(realm, uri, raw_url, Some(*anchor));
            if resolved.is_none() {
                diagnostics.push(MarkyDiagnostic {
                    range: ml.range,
                    severity: DiagnosticSeverity::Error,
                    message: format!("Broken link: heading '{}' not found", anchor),
                });
            }
        }
    }

    // 3. Check for duplicate heading slugs
    let mut slug_counts: HashMap<String, Vec<Range>> = HashMap::new();
    for h in index.headings() {
        let base_slug = slugify(h.text);
        slug_counts.entry(base_slug).or_default().push(h.range);
    }
    for (slug, ranges) in &slug_counts {
        if ranges.len() > 1 {
            for range in ranges {
                diagnostics.push(MarkyDiagnostic {
                    range: *range,
                    severity: DiagnosticSeverity::Warning,
                    message: format!(
                        "Duplicate heading slug '{}' ({} occurrences)",
                        slug,
                        ranges.len()
                    ),
                });
            }
        }
    }

    // 4. Check for unclosed XML tags
    for xt in index.xml_tags() {
        if xt.is_unclosed {
            diagnostics.push(MarkyDiagnostic {
                range: xt.range,
                severity: DiagnosticSeverity::Warning,
                message: format!("Unclosed XML tag: <{}>", xt.tag_name),
            });
        }
    }

    diagnostics
}
