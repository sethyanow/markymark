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

/// Request payload for `semantic-search`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SemanticSearchRequest {
    /// Query text to embed and search against indexed sections.
    pub query: String,
    /// Optional realm name. Defaults to "default".
    pub realm: Option<String>,
    /// Maximum number of matches to return. Defaults to 10.
    pub top_k: Option<u32>,
    /// Similarity floor in `[0.0, 1.0]`. Defaults to 0.5.
    pub min_score: Option<f32>,
}

/// Single semantic-search result entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SemanticSearchResultDto {
    /// Matched document URI.
    pub doc_uri: String,
    /// Matched heading text.
    pub heading: String,
    /// Matched heading level.
    pub heading_level: u8,
    /// Similarity score, rounded to 4 decimals.
    pub score: f32,
    /// Short preview from the matched section.
    pub section_preview: String,
}

/// Response payload for `semantic-search`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SemanticSearchResponse {
    /// Query text used for search.
    pub query: String,
    /// Realm searched.
    pub realm: String,
    /// Ranked semantic matches.
    pub results: Vec<SemanticSearchResultDto>,
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
    /// Include duplicate document pair count in the response.
    #[serde(default)]
    pub check_duplicates: bool,
    /// Include aggregate token estimation in the response.
    #[serde(default)]
    pub include_token_counts: bool,
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
    /// Optional duplicate pair count (when requested).
    pub duplicate_pairs: Option<usize>,
    /// Optional aggregate token count (when requested).
    pub total_tokens: Option<u64>,
}

/// Request payload for `export-index`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportIndexRequest {
    /// Document URI (`file://...`) to export.
    pub uri: String,
    /// Realm to query. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
    /// When true, include content blocks (paragraphs, list items, code blocks, etc.)
    /// in the response. Defaults to false.
    #[serde(default)]
    pub include_blocks: bool,
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

/// A frontmatter entry in an exported document index.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExportedFrontmatterEntryDto {
    /// Key.
    pub key: String,
    /// Value: single-element for string values, multi-element for list values.
    pub value: Vec<String>,
}

/// A Logseq property entry in an exported document index.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ExportedPropertyEntryDto {
    /// Key.
    pub key: String,
    /// Value: single-element for string/page-ref values, multi-element for list values.
    pub value: Vec<String>,
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
    /// YAML frontmatter entries.
    pub frontmatter: Vec<ExportedFrontmatterEntryDto>,
    /// Logseq inline property entries.
    pub properties: Vec<ExportedPropertyEntryDto>,
    /// Content blocks (paragraphs, list items, code blocks, etc.).
    /// Present only when `include_blocks` was true in the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<ContentBlockDto>>,
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

// --- search-workspace ---

/// Request payload for `search-workspace`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchWorkspaceRequest {
    /// Free-text query (case-insensitive substring). Omit to match all documents.
    #[serde(default)]
    pub query: Option<String>,
    /// Frontmatter filter key. Must be paired with `frontmatter_filter_value`.
    #[serde(default)]
    pub frontmatter_filter_key: Option<String>,
    /// Frontmatter filter value (case-insensitive substring match).
    #[serde(default)]
    pub frontmatter_filter_value: Option<String>,
    /// Logseq property filter key. Must be paired with `property_filter_value`.
    #[serde(default)]
    pub property_filter_key: Option<String>,
    /// Logseq property filter value (case-insensitive substring match).
    #[serde(default)]
    pub property_filter_value: Option<String>,
    /// Tag filter: only include documents that have this tag (case-insensitive, without `#`).
    #[serde(default)]
    pub tag_filter: Option<String>,
    /// Realm to search. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
    /// Max results to return (0–100, default 20). Values above 100 are clamped silently.
    #[serde(default = "default_search_workspace_limit")]
    pub limit: u32,
}

fn default_search_workspace_limit() -> u32 {
    20
}

/// A single search-workspace result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceSearchResultDto {
    /// Document URI.
    pub uri: String,
    /// Document title (first H1 heading, or filename without extension).
    pub title: String,
    /// Relevance score (0.0–1.0).
    pub score: f32,
    /// Fields that matched the query (e.g. `["title", "frontmatter:status"]`).
    pub matched_fields: Vec<String>,
    /// First 3 frontmatter key-value pairs.
    pub frontmatter_preview: Vec<(String, String)>,
    /// First 3 Logseq property key-value pairs.
    pub property_preview: Vec<(String, String)>,
    /// All tag names on this document (without `#` prefix).
    pub tags: Vec<String>,
    /// Whether this document is a Logseq journal page.
    pub is_journal: bool,
    /// Journal date `[year, month, day]` if detected.
    pub journal_date: Option<[u16; 3]>,
}

/// Response payload for `search-workspace`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchWorkspaceResponse {
    /// Realm that was searched.
    pub realm: String,
    /// Original query, if provided.
    pub query: Option<String>,
    /// Ranked search results.
    pub results: Vec<WorkspaceSearchResultDto>,
}

// --- search-for-pattern ---

/// Request payload for `search-for-pattern`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchForPatternRequest {
    /// Regex pattern to search for. Must not be empty or whitespace-only.
    pub pattern: String,
    /// Optional glob filter, e.g. `"*.md"` or `"**/*.rs"`.
    /// Patterns without `/` are matched against the filename only;
    /// patterns with `/` are matched against the full file path.
    #[serde(default)]
    pub include_glob: Option<String>,
    /// Lines of context to include around each match (clamped to 0–20, default 2).
    #[serde(default = "default_context_lines")]
    pub context_lines: u32,
    /// Maximum total matches to return (clamped to 1–500, default 100).
    #[serde(default = "default_pattern_limit")]
    pub limit: u32,
    /// Case-insensitive regex matching (default `false`).
    #[serde(default)]
    pub case_insensitive: bool,
    /// Realm to search. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
}

fn default_context_lines() -> u32 {
    2
}

fn default_pattern_limit() -> u32 {
    100
}

/// A single match from `search-for-pattern`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PatternMatchDto {
    /// Document URI where the match was found.
    pub uri: String,
    /// 0-based line number of the match.
    pub line: u32,
    /// 0-based character offset of the match start within the line.
    pub column: u32,
    /// The text matched by the regex.
    pub match_text: String,
    /// The full line containing the match (trailing `\r` stripped).
    pub line_text: String,
    /// Lines before the match (may be empty when `context_lines` is 0 or match is at file start).
    pub context_before: Vec<String>,
    /// Lines after the match (may be empty when `context_lines` is 0 or match is at file end).
    pub context_after: Vec<String>,
    /// 0-based line number of `context_before[0]`.
    pub context_start_line: u32,
}

/// Response payload for `search-for-pattern`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchForPatternResponse {
    /// The pattern that was searched for.
    pub pattern: String,
    /// Realm that was searched.
    pub realm: String,
    /// Number of files that were read and searched.
    pub files_searched: u32,
    /// Number of files skipped (unreadable, too large, or missing path).
    pub files_skipped: u32,
    /// Matches found (up to `limit`).
    pub matches: Vec<PatternMatchDto>,
    /// `true` when results were truncated at `limit`.
    pub truncated: bool,
}

/// Request to analyse the link graph of a realm.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphAnalysisRequest {
    /// Realm to analyse. Defaults to `"default"`.
    pub realm: Option<String>,
    /// Number of top hub documents to return (sorted by incoming link count).
    #[serde(default = "default_top_n_hubs")]
    pub top_n_hubs: u32,
    /// Whether to compute weakly-connected clusters. Can be expensive on large workspaces.
    #[serde(default)]
    pub include_clusters: bool,
}

fn default_top_n_hubs() -> u32 {
    10
}

/// A document with no incoming and no outgoing resolved internal links.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OrphanDto {
    /// Document URI.
    pub uri: String,
}

/// A hub document with the most incoming links.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HubDto {
    /// Document URI.
    pub uri: String,
    /// Number of other documents that link to this one.
    pub incoming_count: u32,
}

/// A link that could not be resolved to any indexed document.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrokenLinkDto {
    /// URI of the document that contains the link.
    pub source_uri: String,
    /// The unresolved link target string.
    pub target: String,
    /// Link kind: `"wiki"` for `[[…]]`, `"markdown"` for `[…](…)`.
    pub kind: String,
}

/// Summary statistics for a realm's link graph.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphStatsDto {
    /// Total number of indexed markdown documents.
    pub total_docs: u32,
    /// Total resolved internal links (wiki + local markdown).
    pub total_internal_links: u32,
    /// Number of orphan documents.
    pub orphan_count: u32,
    /// Number of broken links.
    pub broken_link_count: u32,
    /// Number of clusters, when `include_clusters` was `true`.
    pub cluster_count: Option<u32>,
}

/// A weakly-connected cluster of documents.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClusterDto {
    /// Sequential cluster identifier (0-based, largest cluster first).
    pub id: usize,
    /// URIs of member documents.
    pub members: Vec<String>,
    /// Number of members.
    pub size: usize,
}

/// Request payload for `get-diagnostics`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetDiagnosticsRequest {
    /// Document URI (`file://...`) to check. When omitted, all documents in the realm are
    /// checked.
    #[serde(default)]
    pub uri: Option<String>,
    /// Realm to query. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
}

/// A single diagnostic item in a `get-diagnostics` response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosticItemDto {
    /// Location of the problem in the source file.
    pub range: RangeDto,
    /// Severity: `"error"` or `"warning"`.
    pub severity: String,
    /// Human-readable description of the problem.
    pub message: String,
}

/// Diagnostics for one file in a `get-diagnostics` response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileDiagnosticsDto {
    /// Document URI.
    pub uri: String,
    /// Diagnostics found in this file.
    pub diagnostics: Vec<DiagnosticItemDto>,
}

/// Response from the `get-diagnostics` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetDiagnosticsResponse {
    /// Realm that was checked.
    pub realm: String,
    /// Number of files with at least one diagnostic.
    pub files_with_issues: usize,
    /// Per-file diagnostic lists (only files that have issues are included).
    pub diagnostics: Vec<FileDiagnosticsDto>,
}

/// Response from the graph-analysis tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GraphAnalysisResponse {
    /// Realm that was analysed.
    pub realm: String,
    /// Summary statistics.
    pub stats: GraphStatsDto,
    /// Documents with zero resolved incoming and outgoing links.
    pub orphans: Vec<OrphanDto>,
    /// Top documents by incoming link count, sorted descending.
    pub hubs: Vec<HubDto>,
    /// Links that could not be resolved to any indexed document.
    pub broken_links: Vec<BrokenLinkDto>,
    /// Weakly-connected clusters. `null` when `include_clusters` was `false`.
    pub clusters: Option<Vec<ClusterDto>>,
}

// ---- Content Blocks ----

/// Request payload for `get-content-blocks`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GetContentBlocksRequest {
    /// Document URI (`file://...`) to get content blocks from.
    pub uri: String,
    /// Realm to query. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
    /// Filter by block kind (e.g. `"paragraph"`, `"list_item"`, `"code_block"`,
    /// `"blockquote"`, `"thematic_break"`, `"table"`).
    #[serde(default)]
    pub kind: Option<String>,
    /// Filter by parent heading slug (only blocks under this heading).
    #[serde(default)]
    pub heading: Option<String>,
    /// Look up a specific block by its block reference ID (e.g. `"my-ref"`).
    #[serde(default)]
    pub block_id: Option<String>,
    /// Whether to include block text content in the response. Defaults to `false`.
    #[serde(default)]
    pub include_text: bool,
}

/// A single content block in a `get-content-blocks` response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ContentBlockDto {
    /// Block kind (e.g. `"paragraph"`, `"list_item"`, `"code_block"`).
    pub kind: String,
    /// Source range of the block in the document.
    pub range: RangeDto,
    /// Slug of the parent heading, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_heading_slug: Option<String>,
    /// Block reference ID (e.g. `"my-ref"`), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
    /// The text content of the block (only present when `include_text` is `true`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Response payload for `get-content-blocks`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetContentBlocksResponse {
    /// The document URI that was queried.
    pub uri: String,
    /// The content blocks matching the request filters.
    pub content_blocks: Vec<ContentBlockDto>,
}

// ---- Search Block Text ----

/// Request payload for `search-block-text`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SearchBlockTextRequest {
    /// The substring to search for (case-insensitive). Must not be empty or whitespace-only.
    pub query: String,
    /// Realm to search. Defaults to `"default"` when omitted.
    #[serde(default)]
    pub realm: Option<String>,
    /// Filter by block kind (e.g. `"paragraph"`, `"list_item"`, `"code_block"`).
    #[serde(default)]
    pub kind: Option<String>,
    /// Maximum number of block-level matches to return (0–500, default 100).
    /// Values above 500 are clamped silently.
    #[serde(default = "default_block_text_limit")]
    pub limit: u32,
    /// Whether to include block text content in results. Defaults to `false`.
    #[serde(default)]
    pub include_text: bool,
}

fn default_block_text_limit() -> u32 {
    100
}

/// A single block-level match from `search-block-text`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BlockTextMatchDto {
    /// Document URI where the match was found.
    pub uri: String,
    /// Block kind (e.g. `"paragraph"`, `"list_item"`, `"code_block"`).
    pub kind: String,
    /// Source range of the block in the document.
    pub range: RangeDto,
    /// Slug of the parent heading, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_heading_slug: Option<String>,
    /// Block reference ID, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,
    /// The text content of the block (only present when `include_text` is `true`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Response payload for `search-block-text`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchBlockTextResponse {
    /// Realm that was searched.
    pub realm: String,
    /// The original query string.
    pub query: String,
    /// Total number of matches found (before limit applied).
    pub total_matches: u32,
    /// Block-level matches (up to `limit`).
    pub matches: Vec<BlockTextMatchDto>,
    /// `true` when results were truncated at `limit`.
    pub truncated: bool,
}
