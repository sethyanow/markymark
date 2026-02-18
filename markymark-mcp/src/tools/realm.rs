//! Realm management tool handlers: create, destroy, add-root, remove-root, realm-stats.

use markymark_core::engine::{CoreEngine, CoreOperation, CoreOperationResult};
use rmcp::{model::CallToolResult, ErrorData as McpError};
use serde_json::json;

use super::{tool_error, tool_error_from_core, unexpected_result_error};
use crate::dto::*;

/// Result of a realm tool operation. Carries the `CallToolResult` plus whether
/// subscriptions should be notified (true for mutating realm operations on success).
pub(crate) struct RealmToolResult {
    pub(crate) result: Result<CallToolResult, McpError>,
    pub(crate) notify: bool,
}

pub(crate) fn handle_create_realm(
    engine: &dyn CoreEngine,
    req: CreateRealmRequest,
) -> RealmToolResult {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return RealmToolResult {
            result: Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for create-realm",
            )),
            notify: false,
        };
    }

    match engine.execute(CoreOperation::CreateRealm { name }) {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => RealmToolResult {
            result: Ok(CallToolResult::structured(json!(RealmInfoResponse {
                name,
                root_count,
                document_count,
            }))),
            notify: true,
        },
        CoreOperationResult::Error(err) => RealmToolResult {
            result: Ok(tool_error_from_core(err)),
            notify: false,
        },
        other => RealmToolResult {
            result: Ok(unexpected_result_error("create-realm", &other)),
            notify: false,
        },
    }
}

pub(crate) fn handle_destroy_realm(
    engine: &dyn CoreEngine,
    req: DestroyRealmRequest,
) -> RealmToolResult {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return RealmToolResult {
            result: Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for destroy-realm",
            )),
            notify: false,
        };
    }

    match engine.execute(CoreOperation::DestroyRealm { name }) {
        CoreOperationResult::Ok => RealmToolResult {
            result: Ok(CallToolResult::structured(json!(DestroyRealmResponse {
                success: true
            }))),
            notify: true,
        },
        CoreOperationResult::Error(err) => RealmToolResult {
            result: Ok(tool_error_from_core(err)),
            notify: false,
        },
        other => RealmToolResult {
            result: Ok(unexpected_result_error("destroy-realm", &other)),
            notify: false,
        },
    }
}

pub(crate) fn handle_add_root(engine: &dyn CoreEngine, req: AddRootRequest) -> RealmToolResult {
    let realm = req.realm.trim().to_string();
    if realm.is_empty() {
        return RealmToolResult {
            result: Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for add-root",
            )),
            notify: false,
        };
    }

    let root = std::path::PathBuf::from(&req.root);

    match engine.execute(CoreOperation::AddRoot { realm, root }) {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => RealmToolResult {
            result: Ok(CallToolResult::structured(json!(RealmInfoResponse {
                name,
                root_count,
                document_count,
            }))),
            notify: true,
        },
        CoreOperationResult::Error(err) => RealmToolResult {
            result: Ok(tool_error_from_core(err)),
            notify: false,
        },
        other => RealmToolResult {
            result: Ok(unexpected_result_error("add-root", &other)),
            notify: false,
        },
    }
}

pub(crate) fn handle_remove_root(
    engine: &dyn CoreEngine,
    req: RemoveRootRequest,
) -> RealmToolResult {
    let realm = req.realm.trim().to_string();
    if realm.is_empty() {
        return RealmToolResult {
            result: Ok(tool_error(
                "invalid_name",
                "realm name must not be empty for remove-root",
            )),
            notify: false,
        };
    }

    let root = std::path::PathBuf::from(&req.root);

    match engine.execute(CoreOperation::RemoveRoot { realm, root }) {
        CoreOperationResult::RealmInfo {
            name,
            root_count,
            document_count,
        } => RealmToolResult {
            result: Ok(CallToolResult::structured(json!(RealmInfoResponse {
                name,
                root_count,
                document_count,
            }))),
            notify: true,
        },
        CoreOperationResult::Error(err) => RealmToolResult {
            result: Ok(tool_error_from_core(err)),
            notify: false,
        },
        other => RealmToolResult {
            result: Ok(unexpected_result_error("remove-root", &other)),
            notify: false,
        },
    }
}

pub(crate) fn handle_realm_stats(
    engine: &dyn CoreEngine,
    req: RealmStatsRequest,
) -> Result<CallToolResult, McpError> {
    let realm = req.realm.trim().to_string();
    if realm.is_empty() {
        return Ok(tool_error(
            "invalid_name",
            "realm name must not be empty for realm-stats",
        ));
    }

    match engine.execute(CoreOperation::RealmStats {
        realm,
        check_duplicates: req.check_duplicates,
        include_token_counts: req.include_token_counts,
    }) {
        CoreOperationResult::RealmStats {
            name,
            root_count,
            document_count,
            heading_count,
            xml_tag_count,
            wiki_link_count,
            markdown_link_count,
            structured_doc_count,
            key_path_count,
            duplicate_pairs,
            total_tokens,
        } => Ok(CallToolResult::structured(json!(RealmStatsResponse {
            name,
            root_count,
            document_count,
            heading_count,
            xml_tag_count,
            wiki_link_count,
            markdown_link_count,
            structured_doc_count,
            key_path_count,
            duplicate_pairs,
            total_tokens,
        }))),
        CoreOperationResult::Error(err) => Ok(tool_error_from_core(err)),
        other => Ok(unexpected_result_error("realm-stats", &other)),
    }
}
