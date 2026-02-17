//! MCP prompt handler implementations for MarkymarkMcp.
//!
//! Provides prompt definitions and get_prompt dispatch for:
//! - `explain-link` — analyze a markdown link target in document context
//! - `suggest-references` — suggest relevant references for content at a position

use markymark_core::engine::{CoreOperation, CoreOperationResult};
use markymark_core::{DocumentUri, Position, Range};
use rmcp::{
    model::{GetPromptResult, Prompt, PromptArgument, PromptMessage, PromptMessageRole},
    ErrorData as McpError,
};
use serde_json::Map;

use crate::MarkymarkMcp;

impl MarkymarkMcp {
    /// Return the prompt definitions this server advertises (for testing/introspection).
    pub fn list_prompt_definitions(&self) -> Vec<Prompt> {
        vec![
            Prompt::new(
                "explain-link",
                Some("Analyze and explain a markdown link target in document context"),
                Some(vec![
                    PromptArgument {
                        name: "uri".to_string(),
                        title: None,
                        description: Some("Document URI (file://) containing the link".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "target".to_string(),
                        title: None,
                        description: Some(
                            "Link target (e.g. page-name, page#heading, #local-heading)"
                                .to_string(),
                        ),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "realm".to_string(),
                        title: None,
                        description: Some(
                            "Optional realm name (defaults to 'default')".to_string(),
                        ),
                        required: Some(false),
                    },
                ]),
            ),
            Prompt::new(
                "suggest-references",
                Some(
                    "Suggest relevant references for content at a position in a markdown document",
                ),
                Some(vec![
                    PromptArgument {
                        name: "uri".to_string(),
                        title: None,
                        description: Some("Document URI (file://) to analyze".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "line".to_string(),
                        title: None,
                        description: Some("0-based line number".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "character".to_string(),
                        title: None,
                        description: Some("0-based character offset".to_string()),
                        required: Some(true),
                    },
                    PromptArgument {
                        name: "realm".to_string(),
                        title: None,
                        description: Some(
                            "Optional realm name (defaults to 'default')".to_string(),
                        ),
                        required: Some(false),
                    },
                ]),
            ),
        ]
    }

    /// Dispatch a prompt request by name and optional arguments.
    ///
    /// Returns `GetPromptResult` on success or `McpError` on invalid params.
    pub fn get_prompt_by_name(
        &self,
        name: &str,
        arguments: Option<Map<String, serde_json::Value>>,
    ) -> Result<GetPromptResult, McpError> {
        match name {
            "explain-link" => self.explain_link_prompt(arguments),
            "suggest-references" => self.suggest_references_prompt(arguments),
            _ => Err(McpError::invalid_params(
                format!("unknown prompt: {name}"),
                None,
            )),
        }
    }

    fn explain_link_prompt(
        &self,
        arguments: Option<Map<String, serde_json::Value>>,
    ) -> Result<GetPromptResult, McpError> {
        let args = arguments.ok_or_else(|| {
            McpError::invalid_params("explain-link requires arguments".to_string(), None)
        })?;

        let uri_str = args.get("uri").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::invalid_params("uri argument is required".to_string(), None)
        })?;

        let target = args.get("target").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::invalid_params("target argument is required".to_string(), None)
        })?;

        let realm = args
            .get("realm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Validate URI scheme
        if !uri_str.starts_with("file://") {
            return Err(McpError::invalid_params(
                format!("only file:// URIs are supported, got: {uri_str}"),
                None,
            ));
        }

        let uri = DocumentUri::new(uri_str)
            .map_err(|e| McpError::invalid_params(format!("invalid URI: {e}"), None))?;

        // Gather document context from the core engine
        let mut context_lines = Vec::new();

        // Get document structure via export-index
        match self.engine.execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm: realm.clone(),
        }) {
            CoreOperationResult::DocumentExport {
                headings,
                wiki_links,
                markdown_links,
                ..
            } => {
                if !headings.is_empty() {
                    context_lines.push("Document outline:".to_string());
                    for (text, level, _) in &headings {
                        let indent = "  ".repeat(*level as usize);
                        context_lines.push(format!("{indent}- {text}"));
                    }
                }
                if !wiki_links.is_empty() {
                    context_lines.push("\nWiki links in document:".to_string());
                    for (link_target, heading, _) in &wiki_links {
                        match heading {
                            Some(h) => context_lines.push(format!("  - [[{link_target}#{h}]]")),
                            None => context_lines.push(format!("  - [[{link_target}]]")),
                        }
                    }
                }
                if !markdown_links.is_empty() {
                    context_lines.push("\nMarkdown links in document:".to_string());
                    for (text, url, _) in &markdown_links {
                        context_lines.push(format!("  - [{text}]({url})"));
                    }
                }
            }
            _ => {
                context_lines.push("(document context unavailable)".to_string());
            }
        }

        let context_block = context_lines.join("\n");

        let prompt_text = format!(
            "Analyze the following markdown link and explain what it refers to, \
             whether it is valid, and how it fits in the document structure.\n\n\
             Document: {uri_str}\n\
             Link target: {target}\n\n\
             {context_block}"
        );

        Ok(GetPromptResult {
            description: Some(format!("Explain link target '{target}' in {uri_str}")),
            messages: vec![PromptMessage::new_text(
                PromptMessageRole::User,
                prompt_text,
            )],
        })
    }

    fn suggest_references_prompt(
        &self,
        arguments: Option<Map<String, serde_json::Value>>,
    ) -> Result<GetPromptResult, McpError> {
        let args = arguments.ok_or_else(|| {
            McpError::invalid_params("suggest-references requires arguments".to_string(), None)
        })?;

        let uri_str = args.get("uri").and_then(|v| v.as_str()).ok_or_else(|| {
            McpError::invalid_params("uri argument is required".to_string(), None)
        })?;

        let line = args.get("line").and_then(|v| v.as_u64()).ok_or_else(|| {
            McpError::invalid_params("line argument is required".to_string(), None)
        })? as u32;

        let character = args
            .get("character")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                McpError::invalid_params("character argument is required".to_string(), None)
            })? as u32;

        let realm = args
            .get("realm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Validate URI scheme
        if !uri_str.starts_with("file://") {
            return Err(McpError::invalid_params(
                format!("only file:// URIs are supported, got: {uri_str}"),
                None,
            ));
        }

        let uri = DocumentUri::new(uri_str)
            .map_err(|e| McpError::invalid_params(format!("invalid URI: {e}"), None))?;

        // Find references at this position
        let position = Range::new(
            Position::new(line, character),
            Position::new(line, character),
        );

        let mut context_lines = Vec::new();

        match self.engine.execute(CoreOperation::FindReferences {
            uri: uri.clone(),
            position,
            realm: realm.clone(),
        }) {
            CoreOperationResult::Locations(locations) => {
                if locations.is_empty() {
                    context_lines
                        .push("No existing references found at this position.".to_string());
                } else {
                    context_lines.push(format!("Existing references ({} found):", locations.len()));
                    for (ref_uri, range) in &locations {
                        context_lines.push(format!(
                            "  - {}:{}:{}",
                            ref_uri.as_str(),
                            range.start.line,
                            range.start.character
                        ));
                    }
                }
            }
            _ => {
                context_lines.push("(reference lookup unavailable)".to_string());
            }
        }

        // Get document structure for broader context
        if let CoreOperationResult::DocumentExport {
            headings,
            wiki_links,
            ..
        } = self.engine.execute(CoreOperation::ExportIndex {
            uri: uri.clone(),
            realm,
        }) {
            if !headings.is_empty() {
                context_lines.push("\nDocument headings:".to_string());
                for (text, level, _) in &headings {
                    let indent = "  ".repeat(*level as usize);
                    context_lines.push(format!("{indent}- {text}"));
                }
            }
            if !wiki_links.is_empty() {
                context_lines.push("\nExisting wiki links:".to_string());
                for (target, heading, _) in &wiki_links {
                    match heading {
                        Some(h) => context_lines.push(format!("  - [[{target}#{h}]]")),
                        None => context_lines.push(format!("  - [[{target}]]")),
                    }
                }
            }
        }

        let context_block = context_lines.join("\n");

        let prompt_text = format!(
            "Suggest relevant internal references (wiki links, block references, or \
             markdown links) that could be added at or near the specified position to \
             improve document navigation and cross-document connectivity.\n\n\
             Document: {uri_str}\n\
             Position: line {line}, character {character}\n\n\
             {context_block}"
        );

        Ok(GetPromptResult {
            description: Some(format!(
                "Suggest references for {uri_str} at {line}:{character}"
            )),
            messages: vec![PromptMessage::new_text(
                PromptMessageRole::User,
                prompt_text,
            )],
        })
    }
}
