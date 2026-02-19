//! Reference resolution: resolves wiki links, markdown links, and block refs
//! to their target symbols within a [`RealmIndex`].

use crate::realm::RealmIndex;
use markymark_core::structured::ValueKind;
use markymark_core::{DocumentUri, Range};

/// A resolved reference target.
#[derive(Debug, Clone)]
pub enum ResolvedTarget {
    /// Resolved to a whole document.
    Document(DocumentUri),
    /// Resolved to a heading within a document.
    Heading {
        /// The document containing the heading.
        uri: DocumentUri,
        /// The heading slug.
        slug: String,
        /// The heading display text.
        text: String,
    },
    /// Resolved to a block within a document.
    Block {
        /// The document containing the block.
        uri: DocumentUri,
        /// The block identifier.
        id: String,
    },
    /// Resolved to a key path within a structured document.
    KeyPath {
        /// The document containing the key.
        uri: DocumentUri,
        /// The full key path (e.g. "database.host").
        path: String,
        /// The value kind at this key.
        value_kind: ValueKind,
        /// The source range of the key.
        range: Range,
    },
}

/// Find a document in the realm by matching the file stem of its URI
/// against a target page name (case-insensitive).
fn find_document_by_page_name(realm: &RealmIndex, target: &str) -> Option<DocumentUri> {
    realm.find_uri_by_stem(target)
}

/// Resolve a markdown link URL to a document URI.
///
/// Strategy:
/// - If `url` contains a `/` (directory segment), try path-relative resolution first.
/// - Fall back to stem-only matching (strips any `.md`/`.markdown` extension, then matches stem).
fn find_document_by_markdown_url(
    realm: &RealmIndex,
    from_uri: &DocumentUri,
    url: &str,
) -> Option<DocumentUri> {
    // Skip external URLs; they can never resolve to a workspace document.
    if url.contains("://") || url.starts_with("mailto:") {
        return None;
    }

    // Path-relative: only attempted when url contains a directory separator.
    if url.contains('/') {
        if let Some(uri) = realm.find_uri_by_relative_path(from_uri, url) {
            return Some(uri);
        }
    }

    // Stem-only fallback: strip extension from the last path component, then match by stem.
    let filename = url.rsplit('/').next().unwrap_or(url);
    let stem = filename
        .strip_suffix(".markdown")
        .or_else(|| filename.strip_suffix(".md"))
        .unwrap_or(filename);

    find_document_by_page_name(realm, stem)
}

/// Resolve a wiki link to its target.
///
/// - `[[page-name]]` → document
/// - `[[page#heading]]` → heading in document
/// - `[[page#key.path]]` → structured document key path
/// - `[[#heading]]` → heading in current document (from_uri)
pub fn resolve_wiki_link(
    realm: &RealmIndex,
    from_uri: &DocumentUri,
    target: &str,
    heading: Option<&str>,
) -> Option<ResolvedTarget> {
    match (target.is_empty(), heading) {
        // [[#heading]] - same-page heading link
        (true, Some(slug)) => {
            let doc = realm.get_document(from_uri)?;
            let entry = doc.heading_by_slug(slug)?;
            Some(ResolvedTarget::Heading {
                uri: from_uri.clone(),
                slug: entry.slug.to_string(),
                text: entry.text.to_string(),
            })
        }
        // [[page-name]] - document link
        (false, None) => {
            let doc_uri = find_document_by_page_name(realm, target)?;
            Some(ResolvedTarget::Document(doc_uri))
        }
        // [[page-name#heading-or-keypath]] - heading or key path in another document
        (false, Some(fragment)) => {
            let doc_uri = find_document_by_page_name(realm, target)?;

            // Try markdown heading first
            if let Some(doc) = realm.get_document(&doc_uri) {
                if let Some(entry) = doc.heading_by_slug(fragment) {
                    return Some(ResolvedTarget::Heading {
                        uri: doc_uri,
                        slug: entry.slug.to_string(),
                        text: entry.text.to_string(),
                    });
                }
            }

            // Try structured document key path
            if let Some(st_doc) = realm.get_structured_document(&doc_uri) {
                if let Some(entry) = st_doc.key_by_path(fragment) {
                    return Some(ResolvedTarget::KeyPath {
                        uri: doc_uri,
                        path: entry.path.clone(),
                        value_kind: entry.value_kind,
                        range: entry.key_range,
                    });
                }
            }

            None
        }
        // Empty target with no heading - nothing to resolve
        (true, None) => None,
    }
}

/// Resolve a markdown link anchor to its target.
///
/// - `[text](#heading-slug)` → heading in current document
/// - `[text](other.md)` → another document (stem-only match)
/// - `[text](other.md#heading)` → heading in another document
/// - `[text](dir/other.md)` → path-relative first, stem-only fallback
/// - `[text](dir/other.md#heading)` → heading in path-relative or stem-matched document
pub fn resolve_markdown_link(
    realm: &RealmIndex,
    from_uri: &DocumentUri,
    url: &str,
    anchor: Option<&str>,
) -> Option<ResolvedTarget> {
    match (url.is_empty(), anchor) {
        // [text](#heading-slug) - same-page anchor
        (true, Some(slug)) => {
            let doc = realm.get_document(from_uri)?;
            let entry = doc.heading_by_slug(slug)?;
            Some(ResolvedTarget::Heading {
                uri: from_uri.clone(),
                slug: entry.slug.to_string(),
                text: entry.text.to_string(),
            })
        }
        // [text](url) or [text](url#anchor) - cross-document link
        (false, anchor) => {
            let doc_uri = find_document_by_markdown_url(realm, from_uri, url)?;
            match anchor {
                None => Some(ResolvedTarget::Document(doc_uri)),
                Some(slug) => {
                    let doc = realm.get_document(&doc_uri)?;
                    let entry = doc.heading_by_slug(slug)?;
                    Some(ResolvedTarget::Heading {
                        uri: doc_uri,
                        slug: entry.slug.to_string(),
                        text: entry.text.to_string(),
                    })
                }
            }
        }
        // Empty url with no anchor - nothing to resolve
        (true, None) => None,
    }
}

/// Resolve a block reference to its target.
///
/// - `((block-id))` → block location
pub fn resolve_block_ref(realm: &RealmIndex, id: &str) -> Option<ResolvedTarget> {
    let (uri, block) = realm.lookup_block(id)?;
    Some(ResolvedTarget::Block { uri, id: block.id })
}
