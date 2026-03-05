//! Workspace-wide search implementation for the `search-workspace` MCP tool.

use markymark_core::engine::{CoreOperationResult, WorkspaceSearchResult};
use markymark_core::DocumentUri;
use markymark_index::{
    DocumentIndex, FrontmatterValueEntry, PropertyValueEntry, RealmIndex,
    StructuredDocumentIndex,
};

/// Execute a workspace search across all documents in a realm.
///
/// Filters are ANDed together — a document must pass every active filter.
/// Scoring is based on where the query matches: title=1.0, heading=0.8,
/// frontmatter/property value=0.6. With no query, all matching docs score 1.0.
/// Results are sorted score DESC, then URI ASC for determinism.
pub(crate) fn execute_search_workspace(
    realm_key: &str,
    realm: &RealmIndex,
    query: Option<String>,
    frontmatter_filter: Option<(String, String)>,
    property_filter: Option<(String, String)>,
    tag_filter: Option<String>,
    limit: u32,
) -> CoreOperationResult {
    // Clamp limit; limit=0 returns empty results immediately.
    let limit = (limit as usize).min(100);
    if limit == 0 {
        return CoreOperationResult::WorkspaceSearchResults {
            realm: realm_key.to_string(),
            query,
            results: vec![],
        };
    }

    let query_lc = query.as_deref().map(|q| q.to_lowercase());

    let mut results: Vec<WorkspaceSearchResult> = realm
        .iter_documents()
        .filter_map(|(uri, doc)| {
            score_document(
                uri,
                doc,
                realm,
                &query_lc,
                &frontmatter_filter,
                &property_filter,
                &tag_filter,
            )
        })
        .collect();

    // Also search structured documents (JSON, YAML, TOML, etc.).
    let structured: Vec<WorkspaceSearchResult> = realm
        .iter_structured_documents()
        .filter_map(|(uri, sdoc)| {
            score_structured_document(
                uri,
                sdoc,
                &query_lc,
                &frontmatter_filter,
                &property_filter,
                &tag_filter,
            )
        })
        .collect();
    results.extend(structured);

    // Sort: score DESC, then URI ASC for determinism.
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.uri.as_str().cmp(b.uri.as_str()))
    });

    results.truncate(limit);

    CoreOperationResult::WorkspaceSearchResults {
        realm: realm_key.to_string(),
        query,
        results,
    }
}

/// Score and build a `WorkspaceSearchResult` for one document, returning `None` if the
/// document fails any active filter or (when a query is given) does not match the query.
fn score_document(
    uri: &DocumentUri,
    doc: &DocumentIndex,
    realm: &RealmIndex,
    query_lc: &Option<String>,
    frontmatter_filter: &Option<(String, String)>,
    property_filter: &Option<(String, String)>,
    tag_filter: &Option<String>,
) -> Option<WorkspaceSearchResult> {
    // --- Filter phase (AND logic) ---
    if let Some((fk, fv)) = frontmatter_filter {
        let fv_lc = fv.to_lowercase();
        let passes = doc.frontmatter().iter().any(|entry| {
            entry.key.eq_ignore_ascii_case(fk) && fm_value_contains(&entry.value, &fv_lc)
        });
        if !passes {
            return None;
        }
    }

    if let Some((pk, pv)) = property_filter {
        let pv_lc = pv.to_lowercase();
        let passes = doc.properties().iter().any(|entry| {
            entry.key.eq_ignore_ascii_case(pk) && prop_value_contains(&entry.value, &pv_lc)
        });
        if !passes {
            return None;
        }
    }

    if let Some(tf) = tag_filter {
        let passes = doc
            .tags()
            .iter()
            .any(|entry| entry.name.eq_ignore_ascii_case(tf));
        if !passes {
            return None;
        }
    }

    // --- Score phase ---
    let title = extract_title(uri, doc);
    let mut score: f32 = if query_lc.is_none() { 1.0 } else { 0.0 };
    let mut matched_fields: Vec<String> = Vec::new();

    if let Some(q) = query_lc {
        let title_lc = title.to_lowercase();

        // Title match: score 1.0
        if title_lc.contains(q.as_str()) {
            score = score.max(1.0);
            matched_fields.push("title".to_string());
        }

        // Heading matches: score 0.8 (fixed ceiling regardless of how many headings
        // match, so we break after the first hit to avoid redundant iteration).
        for heading in doc.headings() {
            if heading.text.to_lowercase().contains(q.as_str()) {
                score = score.max(0.8);
                if !matched_fields.contains(&"heading".to_string()) {
                    matched_fields.push("heading".to_string());
                }
                break;
            }
        }

        // Frontmatter value matches: score 0.6
        for entry in doc.frontmatter() {
            if fm_value_contains(&entry.value, q) {
                score = score.max(0.6);
                let field = format!("frontmatter:{}", entry.key);
                if !matched_fields.contains(&field) {
                    matched_fields.push(field);
                }
                break;
            }
        }

        // Property value matches: score 0.6
        for entry in doc.properties() {
            if prop_value_contains(&entry.value, q) {
                score = score.max(0.6);
                let field = format!("property:{}", entry.key);
                if !matched_fields.contains(&field) {
                    matched_fields.push(field);
                }
                break;
            }
        }

        // No match at all: skip this document.
        if score == 0.0 {
            return None;
        }
    }

    // --- Build result ---
    let frontmatter_preview: Vec<(String, String)> = doc
        .frontmatter()
        .iter()
        .take(3)
        .map(|e| (e.key.to_string(), fm_value_to_string(&e.value)))
        .collect();

    let property_preview: Vec<(String, String)> = doc
        .properties()
        .iter()
        .take(3)
        .map(|e| (e.key.to_string(), prop_value_to_string(&e.value)))
        .collect();

    let tags: Vec<String> = doc.tags().iter().map(|t| t.name.to_string()).collect();

    let journal_date = realm.journal_date(uri);
    let is_journal = journal_date.is_some();

    Some(WorkspaceSearchResult {
        uri: uri.clone(),
        title,
        score,
        matched_fields,
        frontmatter_preview,
        property_preview,
        tags,
        is_journal,
        journal_date,
    })
}

/// Score and build a `WorkspaceSearchResult` for one structured document,
/// returning `None` if the document fails any active filter or does not match the query.
///
/// Structured docs have no frontmatter, properties, or tags, so any active filter
/// immediately excludes them. Scoring mirrors the markdown tiers:
/// URI stem match = 1.0, key-path match = 0.8, source-text match = 0.6.
fn score_structured_document(
    uri: &DocumentUri,
    sdoc: &StructuredDocumentIndex,
    query_lc: &Option<String>,
    frontmatter_filter: &Option<(String, String)>,
    property_filter: &Option<(String, String)>,
    tag_filter: &Option<String>,
) -> Option<WorkspaceSearchResult> {
    // Structured docs cannot satisfy any markdown-oriented filter.
    if frontmatter_filter.is_some() || property_filter.is_some() || tag_filter.is_some() {
        return None;
    }

    let title = uri_to_title(uri);
    let mut score: f32 = if query_lc.is_none() { 1.0 } else { 0.0 };
    let mut matched_fields: Vec<String> = Vec::new();

    if let Some(q) = query_lc {
        let title_lc = title.to_lowercase();

        // URI stem / title match: score 1.0
        if title_lc.contains(q.as_str()) {
            score = score.max(1.0);
            matched_fields.push("title".to_string());
        }

        // Key-path match: score 0.8
        if !sdoc.search_keys(q).is_empty() {
            score = score.max(0.8);
            matched_fields.push("key_path".to_string());
        }

        // Source-text (value) match: score 0.6
        if sdoc.source_contains(q) {
            score = score.max(0.6);
            if !matched_fields.iter().any(|f| f == "content") {
                matched_fields.push("content".to_string());
            }
        }

        // No match at all: skip this document.
        if score == 0.0 {
            return None;
        }
    }

    Some(WorkspaceSearchResult {
        uri: uri.clone(),
        title,
        score,
        matched_fields,
        frontmatter_preview: vec![],
        property_preview: vec![],
        tags: vec![],
        is_journal: false,
        journal_date: None,
    })
}

/// Extract a human-readable title from a document.
/// Uses the first H1 heading if present; otherwise derives a title from the URI filename.
fn extract_title(uri: &DocumentUri, doc: &DocumentIndex) -> String {
    if let Some(h1) = doc.headings().iter().find(|h| h.level == 1) {
        return h1.text.to_string();
    }
    uri_to_title(uri)
}

/// Known file extensions to strip when deriving a display title from a URI.
const TITLE_STRIP_EXTENSIONS: &[&str] = &[
    ".mdx", ".md", ".json", ".jsonc", ".json5", ".jsonl", ".yaml", ".yml", ".toml", ".env",
    ".ini", ".cfg",
];

/// Derive a display title from a URI by extracting the filename, stripping known
/// extensions, and converting underscores and hyphens to spaces.
fn uri_to_title(uri: &DocumentUri) -> String {
    uri.as_str()
        .rsplit('/')
        .next()
        .map(|filename| {
            let stem = TITLE_STRIP_EXTENSIONS
                .iter()
                .find_map(|ext| filename.strip_suffix(ext))
                .unwrap_or(filename);
            stem.replace(['_', '-'], " ")
        })
        .unwrap_or_else(|| uri.as_str().to_string())
}

/// Check whether a `FrontmatterValueEntry` contains a given lowercase substring.
/// For list/map variants, any element matching is sufficient.
fn fm_value_contains(value: &FrontmatterValueEntry<'_>, needle: &str) -> bool {
    match value {
        FrontmatterValueEntry::String(s) => s.to_lowercase().contains(needle),
        FrontmatterValueEntry::Integer(n) => n.to_string().contains(needle),
        FrontmatterValueEntry::Float(f) => f.to_string().contains(needle),
        FrontmatterValueEntry::Boolean(b) => b.to_string().contains(needle),
        FrontmatterValueEntry::List(items) => {
            items.iter().any(|item| fm_value_contains(item, needle))
        }
        FrontmatterValueEntry::Map(entries) => entries
            .iter()
            .any(|(k, v)| k.to_lowercase().contains(needle) || fm_value_contains(v, needle)),
        FrontmatterValueEntry::Null => false,
    }
}

/// Render a `FrontmatterValueEntry` as a display string for the search result preview.
fn fm_value_to_string(value: &FrontmatterValueEntry<'_>) -> String {
    match value {
        FrontmatterValueEntry::String(s) => s.to_string(),
        FrontmatterValueEntry::Integer(n) => n.to_string(),
        FrontmatterValueEntry::Float(f) => f.to_string(),
        FrontmatterValueEntry::Boolean(b) => b.to_string(),
        FrontmatterValueEntry::List(items) => items
            .iter()
            .map(fm_value_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        FrontmatterValueEntry::Map(entries) => entries
            .iter()
            .map(|(k, v)| format!("{}: {}", k, fm_value_to_string(v)))
            .collect::<Vec<_>>()
            .join(", "),
        FrontmatterValueEntry::Null => String::new(),
    }
}

/// Check whether a `PropertyValueEntry` contains a given lowercase substring.
fn prop_value_contains(value: &PropertyValueEntry<'_>, needle: &str) -> bool {
    match value {
        PropertyValueEntry::String(s) => s.to_lowercase().contains(needle),
        PropertyValueEntry::List(items) => items
            .iter()
            .any(|item| item.to_lowercase().contains(needle)),
        PropertyValueEntry::PageRef(r) => r.to_lowercase().contains(needle),
    }
}

/// Render a `PropertyValueEntry` as a display string for the search result preview.
fn prop_value_to_string(value: &PropertyValueEntry<'_>) -> String {
    match value {
        PropertyValueEntry::String(s) => s.to_string(),
        PropertyValueEntry::List(items) => items.join(", "),
        PropertyValueEntry::PageRef(r) => format!("[[{r}]]"),
    }
}
