//! get-outline and export-index tool handlers.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult, OutlineTreeNode};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{parse_file_uri, tool_error, tool_error_from_core, unexpected_result_error};
use crate::dto::*;

/// Convert an owned `OutlineTreeNode` to the DTO for serialization.
fn outline_tree_node_to_dto(node: OutlineTreeNode) -> OutlineTreeNodeDto {
    OutlineTreeNodeDto {
        title: node.title,
        level: node.level,
        range: range_to_dto(node.range),
        text: node.text,
        summary: node.summary,
        children: node
            .children
            .into_iter()
            .map(outline_tree_node_to_dto)
            .collect(),
    }
}

pub(crate) async fn handle_get_outline(
    engine: &dyn CoreEngine,
    req: OutlineRequest,
) -> Result<CallToolResult, McpError> {
    let uri = match parse_file_uri(&req.uri) {
        Ok(uri) => uri,
        Err(err) => return Ok(tool_error(&err.code, err.message)),
    };

    let format = req.format.as_deref().unwrap_or("flat").to_string();
    if format != "flat" && format != "tree" {
        return Ok(tool_error(
            "invalid_params",
            format!(
                "Unsupported outline format '{}'. Expected 'flat' or 'tree'.",
                format
            ),
        ));
    }
    let include_text = req.include_text;

    match engine
        .execute(CoreOperation::GetOutline {
            uri,
            realm: req.realm.clone(),
            format: format.clone(),
            include_text,
        })
        .await
    {
        CoreOperationResult::Outline(headings) => {
            Ok(CallToolResult::structured(json!(OutlineResponse {
                uri: req.uri,
                headings,
            })))
        }
        CoreOperationResult::OutlineTree(tree) => {
            let tree_dto = outline_tree_node_to_dto(tree);
            Ok(CallToolResult::structured(json!(OutlineTreeResponse {
                uri: req.uri,
                tree: tree_dto,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("get-outline", &other)),
    }
}

pub(crate) async fn handle_export_index(
    engine: &dyn CoreEngine,
    req: ExportIndexRequest,
) -> Result<CallToolResult, McpError> {
    let uri = match parse_file_uri(&req.uri) {
        Ok(uri) => uri,
        Err(err) => return Ok(tool_error(&err.code, err.message)),
    };

    match engine
        .execute(CoreOperation::ExportIndex {
            uri,
            realm: req.realm.clone(),
            include_blocks: req.include_blocks,
        })
        .await
    {
        CoreOperationResult::DocumentExport {
            uri,
            headings,
            xml_tags,
            wiki_links,
            markdown_links,
            frontmatter,
            properties,
            content_blocks,
            ..
        } => {
            let headings: Vec<ExportedHeadingDto> = headings
                .into_iter()
                .map(|(text, level, range)| ExportedHeadingDto {
                    text,
                    level,
                    range: range_to_dto(range),
                })
                .collect();

            let xml_tags: Vec<ExportedXmlTagDto> = xml_tags
                .into_iter()
                .map(|(tag_name, range)| ExportedXmlTagDto {
                    tag_name,
                    range: range_to_dto(range),
                })
                .collect();

            let wiki_links: Vec<ExportedWikiLinkDto> = wiki_links
                .into_iter()
                .map(|(target, heading, range)| ExportedWikiLinkDto {
                    target,
                    heading,
                    range: range_to_dto(range),
                })
                .collect();

            let markdown_links: Vec<ExportedMarkdownLinkDto> = markdown_links
                .into_iter()
                .map(|(text, url, range)| ExportedMarkdownLinkDto {
                    text,
                    url,
                    range: range_to_dto(range),
                })
                .collect();

            let frontmatter: Vec<ExportedFrontmatterEntryDto> = frontmatter
                .into_iter()
                .map(|(key, value)| ExportedFrontmatterEntryDto { key, value })
                .collect();

            let properties: Vec<ExportedPropertyEntryDto> = properties
                .into_iter()
                .map(|(key, value)| ExportedPropertyEntryDto { key, value })
                .collect();

            let content_blocks_dto: Option<Vec<ContentBlockDto>> = content_blocks.map(|blocks| {
                blocks
                    .into_iter()
                    .map(|b| ContentBlockDto {
                        kind: b.kind,
                        range: range_to_dto(b.range),
                        parent_heading_slug: b.parent_heading_slug,
                        block_id: b.block_id,
                        text: b.text,
                    })
                    .collect()
            });

            Ok(CallToolResult::structured(json!(ExportIndexResponse {
                uri: uri.as_str().to_string(),
                headings,
                xml_tags,
                wiki_links,
                markdown_links,
                frontmatter,
                properties,
                content_blocks: content_blocks_dto,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("export-index", &other)),
    }
}
