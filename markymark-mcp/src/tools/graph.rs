//! graph-analysis tool handler.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{tool_error_from_core, unexpected_result_error};
use crate::dto::*;

pub(crate) fn handle_graph_analysis(
    engine: &dyn CoreEngine,
    req: GraphAnalysisRequest,
) -> Result<CallToolResult, McpError> {
    match engine.execute(CoreOperation::GraphAnalysis {
        realm: req.realm.clone(),
        top_n_hubs: req.top_n_hubs,
        include_clusters: req.include_clusters,
    }) {
        CoreOperationResult::GraphAnalysis {
            realm,
            total_docs,
            total_internal_links,
            orphans,
            hubs,
            broken_links,
            clusters,
        } => {
            let orphan_count = orphans.len().try_into().unwrap_or(u32::MAX);
            let broken_link_count = broken_links.len().try_into().unwrap_or(u32::MAX);
            let cluster_count = clusters
                .as_ref()
                .map(|c| c.len().try_into().unwrap_or(u32::MAX));
            let stats = GraphStatsDto {
                total_docs,
                total_internal_links,
                orphan_count,
                broken_link_count,
                cluster_count,
            };
            let orphan_dtos: Vec<OrphanDto> = orphans
                .into_iter()
                .map(|u| OrphanDto {
                    uri: u.as_str().to_string(),
                })
                .collect();
            let hub_dtos: Vec<HubDto> = hubs
                .into_iter()
                .map(|(u, count)| HubDto {
                    uri: u.as_str().to_string(),
                    incoming_count: count,
                })
                .collect();
            let broken_dtos: Vec<BrokenLinkDto> = broken_links
                .into_iter()
                .map(|(src, target, kind)| BrokenLinkDto {
                    source_uri: src.as_str().to_string(),
                    target,
                    kind,
                })
                .collect();
            let cluster_dtos: Option<Vec<ClusterDto>> = clusters.map(|cs| {
                cs.into_iter()
                    .enumerate()
                    .map(|(id, members)| {
                        let size = members.len();
                        ClusterDto {
                            id,
                            members: members
                                .into_iter()
                                .map(|u| u.as_str().to_string())
                                .collect(),
                            size,
                        }
                    })
                    .collect()
            });
            Ok(CallToolResult::structured(json!(GraphAnalysisResponse {
                realm,
                stats,
                orphans: orphan_dtos,
                hubs: hub_dtos,
                broken_links: broken_dtos,
                clusters: cluster_dtos,
            })))
        }
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("graph-analysis", &other)),
    }
}
