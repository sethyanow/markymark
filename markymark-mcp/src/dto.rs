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
    /// Realm to query. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
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
    /// Realm to query. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
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
    /// Realm to query. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
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
    /// Realm to query. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
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

/// Request payload for `realm-stats`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RealmStatsRequest {
    /// Realm name (e.g. "default").
    pub realm: String,
}

/// Response payload for `realm-stats`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RealmStatsResponse {
    /// Realm name.
    pub name: String,
    /// Number of tracked workspace roots.
    pub root_count: usize,
    /// Number of indexed documents.
    pub document_count: usize,
    /// Total headings across all documents.
    pub heading_count: usize,
    /// Total XML tags across all documents.
    pub xml_tag_count: usize,
    /// Total wiki links across all documents.
    pub wiki_link_count: usize,
    /// Total markdown links across all documents.
    pub markdown_link_count: usize,
    /// Number of structured (non-markdown) documents indexed.
    pub structured_doc_count: usize,
    /// Total key paths across all structured documents.
    pub key_path_count: usize,
}

/// Request payload for `export-index`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportIndexRequest {
    /// Document URI (`file://...`) to export.
    pub uri: String,
    /// Realm to query. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
}

/// A heading entry in an exported document index.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExportedHeadingDto {
    /// Heading text.
    pub text: String,
    /// Heading level (1-6).
    pub level: u8,
    /// Source range.
    pub range: RangeDto,
}

/// An XML tag entry in an exported document index.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExportedXmlTagDto {
    /// Tag name.
    pub tag_name: String,
    /// Source range.
    pub range: RangeDto,
}

/// A wiki link entry in an exported document index.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExportedWikiLinkDto {
    /// Target page name.
    pub target: String,
    /// Optional heading anchor.
    pub heading: Option<String>,
    /// Source range.
    pub range: RangeDto,
}

/// A markdown link entry in an exported document index.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExportedMarkdownLinkDto {
    /// Link display text.
    pub text: String,
    /// Link URL.
    pub url: String,
    /// Source range.
    pub range: RangeDto,
}

/// Response payload for `export-index`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExportIndexResponse {
    /// Document URI.
    pub uri: String,
    /// Headings in document order.
    pub headings: Vec<ExportedHeadingDto>,
    /// XML tags in document order.
    pub xml_tags: Vec<ExportedXmlTagDto>,
    /// Wiki links in document order.
    pub wiki_links: Vec<ExportedWikiLinkDto>,
    /// Markdown links in document order.
    pub markdown_links: Vec<ExportedMarkdownLinkDto>,
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
