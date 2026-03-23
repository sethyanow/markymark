//! Free helper functions extracted from `server.rs` to keep it under 1000 lines.

use std::collections::HashMap;

use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::Location;

use crate::state::{ServerState, StructuredKeyInfo};
use markymark_core::{DocumentUri, Range as CoreRange};
use markymark_index::resolution::{resolve_wiki_link, ResolvedTarget};
use markymark_index::{
    CodeSpanEntry, DocumentIndex, HeadingEntry, MarkdownLinkEntry, WikiLinkEntry, XmlTagEntry,
};

/// Convert a `ResolvedTarget` to an `ls_types::Location`, looking up heading/block ranges.
pub(crate) fn resolved_target_to_location(
    state: &ServerState,
    target: &ResolvedTarget,
) -> Result<Option<Location>> {
    let zero_range = CoreRange::new(
        markymark_core::Position::new(0, 0),
        markymark_core::Position::new(0, 0),
    );

    match target {
        ResolvedTarget::Document(uri) => crate::convert::to_lsp_location(uri, zero_range)
            .map(Some)
            .map_err(|_| tower_lsp_server::jsonrpc::Error::internal_error()),
        ResolvedTarget::Heading { uri, slug, .. } => {
            let range = state
                .get_document_index(uri)
                .and_then(|idx| idx.heading_by_slug(slug))
                .map(|h| h.range)
                .unwrap_or(zero_range);
            crate::convert::to_lsp_location(uri, range)
                .map(Some)
                .map_err(|_| tower_lsp_server::jsonrpc::Error::internal_error())
        }
        ResolvedTarget::Block { uri, id } => {
            let range = state
                .get_document_index(uri)
                .and_then(|idx| idx.block_by_id(id))
                .map(|b| b.range)
                .unwrap_or(zero_range);
            crate::convert::to_lsp_location(uri, range)
                .map(Some)
                .map_err(|_| tower_lsp_server::jsonrpc::Error::internal_error())
        }
        ResolvedTarget::KeyPath { uri, range, .. } => crate::convert::to_lsp_location(uri, *range)
            .map(Some)
            .map_err(|_| tower_lsp_server::jsonrpc::Error::internal_error()),
    }
}

/// Iterate over all `(DocumentUri, DocumentIndex)` pairs in the realm.
pub(crate) fn iter_realm_documents(
    state: &ServerState,
) -> impl Iterator<Item = (&DocumentUri, &DocumentIndex)> {
    state.realm().iter_documents()
}

#[derive(Debug, Default)]
pub(crate) struct XmlHoverStats {
    pub(crate) occurrences: usize,
    pub(crate) document_count: usize,
    pub(crate) attribute_counts: Vec<(String, usize)>,
}

pub(crate) fn xml_hover_stats(state: &ServerState, tag_name: &str) -> XmlHoverStats {
    let mut occurrences = 0usize;
    let mut document_count = 0usize;
    let mut attribute_counts: HashMap<String, usize> = HashMap::new();

    for (_uri, index) in iter_realm_documents(state) {
        let mut has_tag_in_document = false;
        for tag in index.xml_tags() {
            if tag.tag_name != tag_name {
                continue;
            }
            has_tag_in_document = true;
            occurrences += 1;
            for attr_name in tag.attributes.keys() {
                *attribute_counts
                    .entry((*attr_name).to_string())
                    .or_insert(0) += 1;
            }
        }

        if has_tag_in_document {
            document_count += 1;
        }
    }

    let mut attribute_counts: Vec<(String, usize)> = attribute_counts.into_iter().collect();
    attribute_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    XmlHoverStats {
        occurrences,
        document_count,
        attribute_counts,
    }
}

/// Build hover markdown for a structured document key.
pub(crate) fn structured_key_hover_markdown(info: &StructuredKeyInfo) -> String {
    let mut lines = Vec::new();
    lines.push(format!("**Key:** `{}`", info.path));
    lines.push(format!("**Type:** {:?}", info.value_kind));
    lines.push(format!("**Depth:** {}", info.depth));
    lines.push(format!("**Format:** {:?}", info.document_kind));
    lines.join("\n\n")
}

pub(crate) fn hover_heading(h: &HeadingEntry<'_>) -> String {
    let prefix = "#".repeat(h.level as usize);
    format!("{} {}\n\nHeading (level {})", prefix, h.text, h.level)
}

pub(crate) fn hover_wiki_link(
    state: &ServerState,
    doc_uri: &DocumentUri,
    wl: &WikiLinkEntry<'_>,
) -> String {
    let resolved = resolve_wiki_link(state.realm(), doc_uri, wl.target, wl.heading);
    match resolved {
        Some(ResolvedTarget::Document(uri)) => {
            format!("Wiki link to **{}**", uri.as_str())
        }
        Some(ResolvedTarget::Heading { uri, text, .. }) => {
            format!("Wiki link to heading **{}** in {}", text, uri.as_str())
        }
        Some(ResolvedTarget::Block { uri, id }) => {
            format!("Wiki link to block `{}` in {}", id, uri.as_str())
        }
        Some(ResolvedTarget::KeyPath {
            uri,
            path,
            value_kind,
            ..
        }) => {
            format!(
                "Wiki link to key `{}` ({:?}) in {}",
                path,
                value_kind,
                uri.as_str()
            )
        }
        None => {
            format!("Wiki link to **{}** (unresolved)", wl.target)
        }
    }
}

pub(crate) fn hover_markdown_link(ml: &MarkdownLinkEntry<'_>) -> String {
    format!("Markdown link: [{}]({})", ml.text, ml.url)
}

pub(crate) fn hover_xml_tag(state: &ServerState, xt: &XmlTagEntry<'_>) -> String {
    let mut lines = vec![format!("**`<{}>`** XML tag", xt.tag_name)];
    let stats = xml_hover_stats(state, xt.tag_name);
    if !xt.attributes.is_empty() {
        let mut attrs: Vec<_> = xt.attributes.iter().collect();
        attrs.sort_by_key(|(k, _)| *k);
        let attr_list: Vec<String> = attrs
            .iter()
            .map(|(k, v)| format!("- `{}` = `{}`", k, v))
            .collect();
        lines.push(String::new());
        lines.push("**Attributes:**".to_string());
        lines.extend(attr_list);
    }
    lines.push(String::new());
    lines.push("**Workspace usage:**".to_string());
    lines.push(format!(
        "- Occurrences in workspace: **{}**",
        stats.occurrences
    ));
    lines.push(format!(
        "- Documents with this tag: **{}**",
        stats.document_count
    ));
    if !stats.attribute_counts.is_empty() {
        lines.push(String::new());
        lines.push("**Common attributes:**".to_string());
        lines.extend(
            stats
                .attribute_counts
                .iter()
                .map(|(name, count)| format!("- `{}` ({})", name, count)),
        );
    }
    if xt.is_self_closing {
        lines.push(String::new());
        lines.push("*Self-closing tag*".to_string());
    }
    if xt.is_unclosed {
        lines.push(String::new());
        lines.push("**Warning: unclosed tag**".to_string());
    }
    lines.join("\n")
}

pub(crate) fn hover_code_span(state: &ServerState, cs: &CodeSpanEntry<'_>) -> String {
    let mut lines = vec![format!("**`{}`** — inline code span", cs.text)];
    let refs = state.realm().lookup_code_span(cs.text);
    if refs.len() > 1 {
        lines.push(String::new());
        lines.push(format!("**Referenced in {} documents:**", refs.len()));
        for (ref_uri, _) in refs.iter().take(10) {
            lines.push(format!("- {}", ref_uri.as_str()));
        }
        if refs.len() > 10 {
            lines.push(format!("- ... and {} more", refs.len() - 10));
        }
    }
    lines.join("\n")
}

pub(crate) fn hover_structured_key(info: &StructuredKeyInfo) -> String {
    structured_key_hover_markdown(info)
}

pub(crate) fn references_for_heading(
    state: &ServerState,
    heading: &HeadingEntry<'_>,
) -> Vec<Location> {
    let slug = &heading.slug;
    let mut locations = Vec::new();
    for (uri, index) in iter_realm_documents(state) {
        for wl in index.wiki_links() {
            if wl.heading == Some(slug) {
                if let Ok(loc) = crate::convert::to_lsp_location(uri, wl.range) {
                    locations.push(loc);
                }
            }
        }
        for ml in index.markdown_links() {
            if ml.anchor == Some(slug) {
                if let Ok(loc) = crate::convert::to_lsp_location(uri, ml.range) {
                    locations.push(loc);
                }
            }
        }
    }
    locations
}

pub(crate) fn references_for_xml_tag(
    state: &ServerState,
    doc_uri: &DocumentUri,
    xt: &XmlTagEntry<'_>,
    include_declaration: bool,
) -> Vec<Location> {
    let tag_name = &xt.tag_name;
    let mut locations = Vec::new();
    for (uri, index) in iter_realm_documents(state) {
        for xml in index.xml_tags() {
            if !include_declaration && uri == doc_uri && xml.range == xt.range {
                continue;
            }
            if xml.tag_name == *tag_name {
                if let Ok(loc) = crate::convert::to_lsp_location(uri, xml.range) {
                    locations.push(loc);
                }
            }
        }
    }
    locations
}

pub(crate) fn references_for_structured_key(
    state: &ServerState,
    doc_uri: &DocumentUri,
    info: &StructuredKeyInfo,
    include_declaration: bool,
) -> Vec<Location> {
    let key_path = &info.path;
    let mut locations = Vec::new();

    if include_declaration {
        if let Some(st_idx) = state.get_structured_document_index(doc_uri) {
            if let Some(entry) = st_idx.key_by_path(key_path) {
                if let Ok(loc) = crate::convert::to_lsp_location(doc_uri, entry.key_range) {
                    locations.push(loc);
                }
            }
        }
    }

    for (md_uri, md_index) in iter_realm_documents(state) {
        for wl in md_index.wiki_links() {
            if let Some(ResolvedTarget::KeyPath {
                uri: ref target_uri,
                ref path,
                ..
            }) = resolve_wiki_link(state.realm(), md_uri, wl.target, wl.heading)
            {
                if target_uri == doc_uri && path == key_path {
                    if let Ok(loc) = crate::convert::to_lsp_location(md_uri, wl.range) {
                        locations.push(loc);
                    }
                }
            }
        }
    }
    locations
}

pub(crate) fn references_for_wiki_link(
    state: &ServerState,
    doc_uri: &DocumentUri,
    wl: &WikiLinkEntry<'_>,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let resolved = resolve_wiki_link(state.realm(), doc_uri, wl.target, wl.heading);
    match resolved {
        Some(ResolvedTarget::KeyPath {
            ref uri,
            ref path,
            range,
            ..
        }) => {
            let target_uri = uri.clone();
            let target_path = path.clone();
            let mut locations = Vec::new();

            if let Ok(loc) = crate::convert::to_lsp_location(&target_uri, range) {
                locations.push(loc);
            }

            for (md_uri, md_index) in iter_realm_documents(state) {
                for other_wl in md_index.wiki_links() {
                    if !include_declaration
                        && md_uri == doc_uri
                        && other_wl.range == wl.range
                    {
                        continue;
                    }
                    if let Some(ResolvedTarget::KeyPath {
                        uri: ref resolved_uri,
                        ref path,
                        ..
                    }) = resolve_wiki_link(
                        state.realm(),
                        md_uri,
                        other_wl.target,
                        other_wl.heading,
                    ) {
                        if resolved_uri == &target_uri && path == &target_path {
                            if let Ok(loc) =
                                crate::convert::to_lsp_location(md_uri, other_wl.range)
                            {
                                locations.push(loc);
                            }
                        }
                    }
                }
            }
            Some(locations)
        }
        _ => None,
    }
}
