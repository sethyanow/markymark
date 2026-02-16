//! LSP server: LanguageServer trait implementation using tower-lsp-server.

use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService};

use crate::state::{
    DiagnosticSeverity as MarkyDiagSeverity, ServerState, StructuredKeyInfo, SymbolAtPosition,
};
use markymark_core::{DocumentUri, Range as CoreRange};
use markymark_index::resolution::{resolve_markdown_link, resolve_wiki_link, ResolvedTarget};
use markymark_index::DocumentIndex;

use crate::symbols::{key_entries_to_symbols, outline_children_to_symbols, xml_tags_to_symbols};
use std::collections::HashMap;

/// The LSP server backend.
pub struct Backend {
    /// The tower-lsp client for sending notifications.
    client: Client,
    /// Shared server state behind a read-write lock.
    state: Arc<RwLock<ServerState>>,
}

impl Backend {
    /// Create a new Backend with the given tower-lsp client.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(ServerState::new())),
        }
    }

    /// Create a Backend with pre-existing state (for testing).
    pub fn with_state(client: Client, state: ServerState) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Get a reference to the shared state (for testing).
    pub fn state(&self) -> &Arc<RwLock<ServerState>> {
        &self.state
    }

    /// Compute and publish diagnostics for a document.
    ///
    /// Acquires a read lock on state, computes diagnostics, drops the lock,
    /// then sends the diagnostics notification to the client.
    async fn publish_diagnostics_for(&self, lsp_uri: Uri, doc_uri: &DocumentUri) {
        let diagnostics = {
            let state = self.state.read().await;
            state.compute_diagnostics(doc_uri)
        };
        // Lock is dropped before the async client call (deadlock prevention)

        let lsp_diagnostics: Vec<Diagnostic> = diagnostics
            .into_iter()
            .map(|d| Diagnostic {
                range: crate::convert::to_lsp_range(d.range),
                severity: Some(match d.severity {
                    MarkyDiagSeverity::Error => DiagnosticSeverity::ERROR,
                    MarkyDiagSeverity::Warning => DiagnosticSeverity::WARNING,
                }),
                source: Some("markymark".to_string()),
                message: d.message,
                ..Default::default()
            })
            .collect();

        self.client
            .publish_diagnostics(lsp_uri, lsp_diagnostics, None)
            .await;
    }
}

/// Create an `LspService` and socket pair for the markymark LSP server.
pub fn create_service() -> (LspService<Backend>, tower_lsp_server::ClientSocket) {
    LspService::new(Backend::new)
}

impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        ..Default::default()
                    },
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "[".to_string(),
                        "#".to_string(),
                        "(".to_string(),
                        "<".to_string(),
                    ]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        // No-op
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri_str = params.text_document.uri;
        if let Ok(doc_uri) = crate::convert::from_lsp_uri(&uri_str) {
            {
                let mut state = self.state.write().await;
                state.open_document(doc_uri.clone(), params.text_document.text);
            }
            self.publish_diagnostics_for(uri_str, &doc_uri).await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri_str = params.text_document.uri;
        if let Ok(doc_uri) = crate::convert::from_lsp_uri(&uri_str) {
            let changes: Vec<crate::state::DocumentChange> = params
                .content_changes
                .into_iter()
                .map(|change| match change.range {
                    Some(range) => crate::state::DocumentChange::Incremental {
                        start_line: range.start.line,
                        start_character: range.start.character,
                        end_line: range.end.line,
                        end_character: range.end.character,
                        text: change.text,
                    },
                    None => crate::state::DocumentChange::Full(change.text),
                })
                .collect();

            if !changes.is_empty() {
                {
                    let mut state = self.state.write().await;
                    state.apply_document_changes(&doc_uri, changes);
                }
                self.publish_diagnostics_for(uri_str, &doc_uri).await;
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri_str = params.text_document.uri;
        if let Ok(doc_uri) = crate::convert::from_lsp_uri(&uri_str) {
            {
                let mut state = self.state.write().await;
                state.close_document(&doc_uri);
            }
            // Clear diagnostics on close
            self.client
                .publish_diagnostics(uri_str, Vec::new(), None)
                .await;
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri_str = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let core_pos = crate::convert::from_lsp_position(pos);

        let state = self.state.read().await;
        let doc_uri = match crate::convert::from_lsp_uri(uri_str) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        let symbol = match state.symbol_at_position(&doc_uri, core_pos) {
            Some(s) => s,
            None => return Ok(None),
        };

        let resolved = match &symbol {
            SymbolAtPosition::WikiLink(wl) => {
                resolve_wiki_link(state.realm(), &doc_uri, wl.target, wl.heading)
            }
            SymbolAtPosition::MarkdownLink(ml) => {
                resolve_markdown_link(state.realm(), &doc_uri, ml.url, ml.anchor)
            }
            SymbolAtPosition::Heading(_) | SymbolAtPosition::StructuredKey(_) => return Ok(None),
            SymbolAtPosition::XmlTag(ref xt) => {
                // Jump to the first occurrence of this tag name in the workspace.
                // Sort documents by URI for deterministic ordering.
                let tag_name = &xt.tag_name;
                let mut first_uri: Option<DocumentUri> = None;
                let mut first_range: Option<markymark_core::Range> = None;
                let mut docs: Vec<_> = iter_realm_documents(&state).collect();
                docs.sort_by_key(|(uri, _)| uri.as_str().to_string());
                'outer: for (uri, index) in &docs {
                    for xml in index.xml_tags() {
                        if xml.tag_name == *tag_name {
                            first_uri = Some((*uri).clone());
                            first_range = Some(xml.range);
                            break 'outer;
                        }
                    }
                }
                match (first_uri.as_ref(), first_range) {
                    (Some(target_uri), Some(range)) => {
                        // If first occurrence is at the cursor position, nothing to navigate to
                        if *target_uri == doc_uri && range == xt.range {
                            return Ok(None);
                        }
                        match crate::convert::to_lsp_location(target_uri, range) {
                            Ok(loc) => {
                                return Ok(Some(GotoDefinitionResponse::Scalar(loc)));
                            }
                            Err(_) => return Ok(None),
                        }
                    }
                    _ => return Ok(None),
                }
            }
        };

        let resolved = match resolved {
            Some(r) => r,
            None => return Ok(None),
        };

        let location = resolved_target_to_location(&state, &resolved)?;
        match location {
            Some(loc) => Ok(Some(GotoDefinitionResponse::Scalar(loc))),
            None => Ok(None),
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri_str = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let core_pos = crate::convert::from_lsp_position(pos);
        let include_declaration = params.context.include_declaration;

        let state = self.state.read().await;
        let doc_uri = match crate::convert::from_lsp_uri(uri_str) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        let symbol = match state.symbol_at_position(&doc_uri, core_pos) {
            Some(s) => s,
            None => return Ok(None),
        };

        let mut locations = Vec::new();

        match symbol {
            SymbolAtPosition::Heading(ref heading) => {
                let slug = &heading.slug;
                // Search all documents for wiki links and markdown links referencing this slug
                for (uri, index) in iter_realm_documents(&state) {
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
            }
            SymbolAtPosition::XmlTag(ref xt) => {
                let tag_name = &xt.tag_name;
                // Search all documents for XML tags with the same name
                for (uri, index) in iter_realm_documents(&state) {
                    for xml in index.xml_tags() {
                        if !include_declaration && uri == &doc_uri && xml.range == xt.range {
                            continue;
                        }
                        if xml.tag_name == *tag_name {
                            if let Ok(loc) = crate::convert::to_lsp_location(uri, xml.range) {
                                locations.push(loc);
                            }
                        }
                    }
                }
            }
            SymbolAtPosition::StructuredKey(ref info) => {
                // Direction 1: Cursor on a structured doc key -> find all markdown wiki-links
                // that resolve to this key path in this document.
                let key_path = &info.path;

                // Optionally include the declaration (the key itself)
                if include_declaration {
                    if let Some(st_idx) = state.get_structured_document_index(&doc_uri) {
                        if let Some(entry) = st_idx.key_by_path(key_path) {
                            if let Ok(loc) =
                                crate::convert::to_lsp_location(&doc_uri, entry.key_range)
                            {
                                locations.push(loc);
                            }
                        }
                    }
                }

                // Search all markdown documents for wiki-links that resolve to this key path
                for (md_uri, md_index) in iter_realm_documents(&state) {
                    for wl in md_index.wiki_links() {
                        if let Some(ResolvedTarget::KeyPath {
                            uri: ref target_uri,
                            ref path,
                            ..
                        }) = resolve_wiki_link(state.realm(), md_uri, wl.target, wl.heading)
                        {
                            if target_uri == &doc_uri && path == key_path {
                                if let Ok(loc) = crate::convert::to_lsp_location(md_uri, wl.range) {
                                    locations.push(loc);
                                }
                            }
                        }
                    }
                }
            }
            SymbolAtPosition::WikiLink(ref wl) => {
                // Direction 2: Cursor on a wiki-link -> if it resolves to a KeyPath,
                // find the key definition + other wiki-links referencing the same key.
                let resolved = resolve_wiki_link(state.realm(), &doc_uri, wl.target, wl.heading);
                match resolved {
                    Some(ResolvedTarget::KeyPath {
                        ref uri,
                        ref path,
                        range,
                        ..
                    }) => {
                        let target_uri = uri.clone();
                        let target_path = path.clone();

                        // Include the key definition location
                        if let Ok(loc) = crate::convert::to_lsp_location(&target_uri, range) {
                            locations.push(loc);
                        }

                        // Find other wiki-links referencing the same key path
                        for (md_uri, md_index) in iter_realm_documents(&state) {
                            for other_wl in md_index.wiki_links() {
                                // Skip the current wiki-link
                                if !include_declaration
                                    && md_uri == &doc_uri
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
                    }
                    _ => return Ok(None),
                }
            }
            _ => return Ok(None),
        }

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri_str = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let core_pos = crate::convert::from_lsp_position(pos);

        let state = self.state.read().await;
        let doc_uri = match crate::convert::from_lsp_uri(uri_str) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        let symbol = match state.symbol_at_position(&doc_uri, core_pos) {
            Some(s) => s,
            None => return Ok(None),
        };

        let markdown = match &symbol {
            SymbolAtPosition::Heading(h) => {
                let prefix = "#".repeat(h.level as usize);
                format!("{} {}\n\nHeading (level {})", prefix, h.text, h.level)
            }
            SymbolAtPosition::WikiLink(wl) => {
                let resolved = resolve_wiki_link(state.realm(), &doc_uri, wl.target, wl.heading);
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
            SymbolAtPosition::MarkdownLink(ml) => {
                format!("Markdown link: [{}]({})", ml.text, ml.url)
            }
            SymbolAtPosition::XmlTag(xt) => {
                let mut lines = vec![format!("**`<{}>`** XML tag", xt.tag_name)];
                let stats = xml_hover_stats(&state, xt.tag_name);
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
            SymbolAtPosition::StructuredKey(ref info) => structured_key_hover_markdown(info),
        };

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        }))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri_str = &params.text_document.uri;

        let state = self.state.read().await;
        let doc_uri = match crate::convert::from_lsp_uri(uri_str) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        match state.get_any_document_index(&doc_uri) {
            Some(markymark_index::AnyDocumentIndex::Markdown(index)) => {
                let outline = index.outline();
                let mut symbols = outline_children_to_symbols(outline.children);
                symbols.extend(xml_tags_to_symbols(index.xml_tags()));

                if symbols.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(DocumentSymbolResponse::Nested(symbols)))
                }
            }
            Some(markymark_index::AnyDocumentIndex::Structured(index)) => {
                let symbols = key_entries_to_symbols(index);
                if symbols.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(DocumentSymbolResponse::Nested(symbols)))
                }
            }
            None => Ok(None),
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri_str = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let core_pos = crate::convert::from_lsp_position(pos);

        let state = self.state.read().await;
        let doc_uri = match crate::convert::from_lsp_uri(uri_str) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        let candidates = state.completion_at(&doc_uri, core_pos);
        if candidates.is_empty() {
            return Ok(None);
        }

        let items: Vec<CompletionItem> = candidates
            .into_iter()
            .map(|c| CompletionItem {
                label: c.label,
                kind: Some(match c.kind {
                    crate::state::CompletionCandidateKind::Page => CompletionItemKind::FILE,
                    crate::state::CompletionCandidateKind::Heading => CompletionItemKind::REFERENCE,
                    crate::state::CompletionCandidateKind::Tag => CompletionItemKind::KEYWORD,
                    crate::state::CompletionCandidateKind::BlockRef => CompletionItemKind::SNIPPET,
                    crate::state::CompletionCandidateKind::XmlTag => CompletionItemKind::CLASS,
                }),
                detail: c.detail,
                ..Default::default()
            })
            .collect();

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri_str = &params.text_document.uri;
        let pos = params.position;
        let core_pos = crate::convert::from_lsp_position(pos);

        let state = self.state.read().await;
        let doc_uri = match crate::convert::from_lsp_uri(uri_str) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        let result = match state.prepare_rename_at(&doc_uri, core_pos) {
            Some(r) => r,
            None => return Ok(None),
        };

        Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: crate::convert::to_lsp_range(result.range),
            placeholder: result.placeholder,
        }))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri_str = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let core_pos = crate::convert::from_lsp_position(pos);
        let new_name = &params.new_name;

        let state = self.state.read().await;
        let doc_uri = match crate::convert::from_lsp_uri(uri_str) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };

        let edits = match state.rename_at(&doc_uri, core_pos, new_name) {
            Some(e) => e,
            None => return Ok(None),
        };

        // Group edits by URI
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        for edit in edits {
            let lsp_uri = match crate::convert::to_lsp_uri(&edit.uri) {
                Ok(u) => u,
                Err(_) => continue,
            };
            changes.entry(lsp_uri).or_default().push(TextEdit {
                range: crate::convert::to_lsp_range(edit.range),
                new_text: edit.new_text,
            });
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<WorkspaceSymbolResponse>> {
        let query = params.query.to_lowercase();
        let state = self.state.read().await;

        let zero_range = markymark_core::Range::new(
            markymark_core::Position::new(0, 0),
            markymark_core::Position::new(0, 0),
        );

        let mut symbols = Vec::new();

        for (uri, index) in iter_realm_documents(&state) {
            let lsp_uri = match crate::convert::to_lsp_uri(uri) {
                Ok(u) => u,
                Err(_) => continue,
            };

            for heading in index.headings() {
                if query.is_empty() || heading.text.to_lowercase().contains(&query) {
                    let range = crate::convert::to_lsp_range(heading.range);
                    #[expect(
                        deprecated,
                        reason = "SymbolInformation.deprecated field is deprecated by LSP spec but struct still required"
                    )]
                    symbols.push(SymbolInformation {
                        name: heading.text.to_string(),
                        kind: SymbolKind::STRING,
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri: lsp_uri.clone(),
                            range,
                        },
                        container_name: None,
                    });
                }
            }

            for tag in index.tags() {
                let tag_name = format!("#{}", tag.name);
                if query.is_empty() || tag_name.to_lowercase().contains(&query) {
                    let range = crate::convert::to_lsp_range(zero_range);
                    #[expect(
                        deprecated,
                        reason = "SymbolInformation.deprecated field is deprecated by LSP spec but struct still required"
                    )]
                    symbols.push(SymbolInformation {
                        name: tag_name,
                        kind: SymbolKind::CONSTANT,
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri: lsp_uri.clone(),
                            range,
                        },
                        container_name: None,
                    });
                }
            }

            for xt in index.xml_tags() {
                let xml_name = format!("<{}>", xt.tag_name);
                if query.is_empty() || xml_name.to_lowercase().contains(&query) {
                    let range = crate::convert::to_lsp_range(xt.range);
                    #[expect(
                        deprecated,
                        reason = "SymbolInformation.deprecated field is deprecated by LSP spec but struct still required"
                    )]
                    symbols.push(SymbolInformation {
                        name: xml_name,
                        kind: SymbolKind::OBJECT,
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri: lsp_uri.clone(),
                            range,
                        },
                        container_name: None,
                    });
                }
            }
        }

        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(WorkspaceSymbolResponse::Flat(symbols)))
        }
    }
}

/// Convert a `ResolvedTarget` to an `ls_types::Location`, looking up heading/block ranges.
fn resolved_target_to_location(
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
fn iter_realm_documents(
    state: &ServerState,
) -> impl Iterator<Item = (&DocumentUri, &DocumentIndex)> {
    state.realm().iter_documents()
}

#[derive(Debug, Default)]
struct XmlHoverStats {
    occurrences: usize,
    document_count: usize,
    attribute_counts: Vec<(String, usize)>,
}

fn xml_hover_stats(state: &ServerState, tag_name: &str) -> XmlHoverStats {
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
fn structured_key_hover_markdown(info: &StructuredKeyInfo) -> String {
    let mut lines = Vec::new();
    lines.push(format!("**Key:** `{}`", info.path));
    lines.push(format!("**Type:** {:?}", info.value_kind));
    lines.push(format!("**Depth:** {}", info.depth));
    lines.push(format!("**Format:** {:?}", info.document_kind));
    lines.join("\n\n")
}
