//! Reference resolution: resolves wiki links, markdown links, and block refs
//! to their target symbols within a [`RealmIndex`].

use crate::realm::RealmIndex;
use markymark_core::DocumentUri;

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
}

/// Find a document in the realm by matching the file stem of its URI
/// against a target page name (case-insensitive).
fn find_document_by_page_name(realm: &RealmIndex, target: &str) -> Option<DocumentUri> {
    realm.find_uri_by_stem(target)
}

/// Resolve a wiki link to its target.
///
/// - `[[page-name]]` → document
/// - `[[page#heading]]` → heading in document
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
        // [[page-name#heading]] - heading in another document
        (false, Some(slug)) => {
            let doc_uri = find_document_by_page_name(realm, target)?;
            let doc = realm.get_document(&doc_uri)?;
            let entry = doc.heading_by_slug(slug)?;
            Some(ResolvedTarget::Heading {
                uri: doc_uri,
                slug: entry.slug.to_string(),
                text: entry.text.to_string(),
            })
        }
        // Empty target with no heading - nothing to resolve
        (true, None) => None,
    }
}

/// Resolve a markdown link anchor to its target.
///
/// - `[text](#heading-slug)` → heading in current document
/// - `[text](other.md#heading)` → heading in another document
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
        _ => None,
    }
}

/// Resolve a block reference to its target.
///
/// - `((block-id))` → block location
pub fn resolve_block_ref(realm: &RealmIndex, id: &str) -> Option<ResolvedTarget> {
    let (uri, block) = realm.lookup_block(id)?;
    Some(ResolvedTarget::Block {
        uri,
        id: block.id,
    })
}
