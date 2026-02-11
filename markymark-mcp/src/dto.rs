//! Request/response DTOs for markymark MCP tool handlers.
//!
//! These types define the wire format for structured MCP tool calls.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use markymark_core::{Position, Range};

/// Request payload for `get-outline`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutlineRequest {
    /// Document URI (`file://...`) to inspect.
    pub uri: String,
}

/// Response payload for `get-outline`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct OutlineResponse {
    /// Input document URI.
    pub uri: String,
    /// Heading outline entries.
    pub headings: Vec<String>,
}

/// Request payload for `search-symbols`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchSymbolsRequest {
    /// Query text to match against symbols.
    pub query: String,
}

/// Position payload in MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct PositionDto {
    /// 0-based line.
    pub line: u32,
    /// 0-based character offset.
    pub character: u32,
}

/// Range payload in MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct RangeDto {
    /// Inclusive start.
    pub start: PositionDto,
    /// Exclusive end.
    pub end: PositionDto,
}

/// Symbol match payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolMatchDto {
    /// Symbol text.
    pub name: String,
    /// Document URI where symbol appears.
    pub uri: String,
    /// Symbol location.
    pub range: RangeDto,
}

/// Response payload for `search-symbols`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SearchSymbolsResponse {
    /// Query text used for search.
    pub query: String,
    /// Deterministically ordered matches.
    pub symbols: Vec<SymbolMatchDto>,
}

/// Request payload for `find-references`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FindReferencesRequest {
    /// Document URI (`file://...`) containing the symbol.
    pub uri: String,
    /// 0-based line of the symbol.
    pub line: u32,
    /// 0-based character offset of the symbol.
    pub character: u32,
}

/// Location payload in MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocationDto {
    /// Document URI where the reference appears.
    pub uri: String,
    /// Range of the reference.
    pub range: RangeDto,
}

/// Response payload for `find-references`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct FindReferencesResponse {
    /// Input document URI.
    pub uri: String,
    /// Deterministically ordered reference locations.
    pub locations: Vec<LocationDto>,
}

/// Request payload for `rename`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RenameRequest {
    /// Document URI (`file://...`) containing the symbol.
    pub uri: String,
    /// 0-based line of the symbol.
    pub line: u32,
    /// 0-based character offset of the symbol.
    pub character: u32,
    /// New name for the symbol.
    pub new_name: String,
}

/// A single text edit within a document.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextEditDto {
    /// Range to replace.
    pub range: RangeDto,
    /// Replacement text.
    pub new_text: String,
}

/// Per-document edits in a workspace edit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocumentEditDto {
    /// Document URI.
    pub uri: String,
    /// Text edits for this document, sorted by range.
    pub edits: Vec<TextEditDto>,
}

/// Response payload for `rename`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RenameResponse {
    /// Per-document text edits to apply.
    pub changes: Vec<DocumentEditDto>,
}

/// Request payload for `create-realm`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateRealmRequest {
    /// Unique name for the new realm.
    pub name: String,
}

/// Request payload for `destroy-realm`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DestroyRealmRequest {
    /// Name of the realm to destroy.
    pub name: String,
}

/// Request payload for `add-root`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddRootRequest {
    /// Name of the realm to add the root to.
    pub realm: String,
    /// Filesystem path of the workspace root to add.
    pub root: String,
}

/// Request payload for `remove-root`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemoveRootRequest {
    /// Name of the realm to remove the root from.
    pub realm: String,
    /// Filesystem path of the workspace root to remove.
    pub root: String,
}

/// Response payload for realm operations that return realm info.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealmInfoResponse {
    /// Realm name.
    pub name: String,
    /// Number of tracked workspace roots.
    pub root_count: usize,
    /// Number of indexed documents.
    pub document_count: usize,
}

/// Response payload for `destroy-realm`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DestroyRealmResponse {
    /// Whether the realm was destroyed.
    pub success: bool,
}

/// Tool error envelope for consistent structured failures.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ToolErrorEnvelope {
    /// Error body.
    pub error: ToolErrorPayload,
}

/// Tool error payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ToolErrorPayload {
    /// Stable machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
}

/// Convert a core `Range` to a DTO `RangeDto`.
pub fn range_to_dto(range: Range) -> RangeDto {
    RangeDto {
        start: position_to_dto(range.start),
        end: position_to_dto(range.end),
    }
}

/// Convert a core `Position` to a DTO `PositionDto`.
pub fn position_to_dto(position: Position) -> PositionDto {
    PositionDto {
        line: position.line,
        character: position.character,
    }
}
