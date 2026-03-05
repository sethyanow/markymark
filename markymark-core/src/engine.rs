//! Core engine: transport-agnostic operation types and trait.

use async_trait::async_trait;

use crate::structured::DocumentKind;
use crate::{CoreError, DocumentUri, Range};

/// A single result from a workspace-wide search.
#[derive(Debug, Clone)]
pub struct WorkspaceSearchResult {
    /// Document URI.
    pub uri: DocumentUri,
    /// First H1 heading text, or filename derived from URI (stripped extension, underscores to spaces).
    pub title: String,
    /// Relevance score: 1.0 = title match, 0.8 = heading match, 0.6 = frontmatter/property match, 1.0 = filter-only (no query).
    pub score: f32,
    /// Which fields matched the query, e.g. ["title", "frontmatter:status", "property:type", "heading"].
    pub matched_fields: Vec<String>,
    /// First 3 frontmatter key-value pairs (value stringified for display).
    pub frontmatter_preview: Vec<(String, String)>,
    /// First 3 Logseq inline property key-value pairs.
    pub property_preview: Vec<(String, String)>,
    /// All tag names on this document (without `#` prefix).
    pub tags: Vec<String>,
    /// Whether this document was detected as a Logseq journal page.
    pub is_journal: bool,
    /// Logseq journal date `(year, month, day)` if detected.
    pub journal_date: Option<(u16, u8, u8)>,
}

/// Semantic search match payload shared across transports.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticSearchMatch {
    /// Matched document URI.
    pub doc_uri: DocumentUri,
    /// Matched heading text.
    pub heading: String,
    /// Matched heading level.
    pub heading_level: u8,
    /// Similarity score.
    pub score: f32,
    /// Heading/section source range.
    pub section_range: Range,
    /// Short preview snippet for the matched section.
    pub section_preview: String,
}

/// Severity level for a diagnostic produced by document analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    /// An error (e.g., broken link). LSP value 1.
    Error,
    /// A warning (e.g., duplicate heading slug). LSP value 2.
    Warning,
}

/// A diagnostic produced by document analysis.
#[derive(Debug, Clone)]
pub struct CoreDiagnostic {
    /// Source range of the problem (0-based lines, 0-based characters).
    pub range: crate::Range,
    /// Severity level.
    pub severity: DiagnosticSeverity,
    /// Human-readable description of the problem.
    pub message: String,
}

/// An operation that can be executed by the core engine.
#[derive(Debug)]
pub enum CoreOperation {
    /// Get the document outline (heading tree).
    GetOutline {
        /// Target document.
        uri: DocumentUri,
        /// Realm to query. Defaults to "default" when `None`.
        realm: Option<String>,
        /// Output format: "flat" (default) or "tree" (hierarchical).
        format: String,
        /// When true (and format="tree"), inline section text in each node.
        include_text: bool,
    },
    /// Find all references to the symbol at a position.
    FindReferences {
        /// Target document.
        uri: DocumentUri,
        /// Position of the symbol.
        position: Range,
        /// Realm to query. Defaults to "default" when `None`.
        realm: Option<String>,
    },
    /// Rename the symbol at a position.
    Rename {
        /// Target document.
        uri: DocumentUri,
        /// Position of the symbol.
        position: Range,
        /// New name for the symbol.
        new_name: String,
        /// Realm to query. Defaults to "default" when `None`.
        realm: Option<String>,
    },
    /// Search for symbols matching a query.
    SearchSymbols {
        /// Search query string.
        query: String,
        /// Realm to query. Defaults to "default" when `None`.
        realm: Option<String>,
    },
    /// Run semantic search across indexed document sections.
    SemanticSearch {
        /// Search query string.
        query: String,
        /// Optional realm name. Defaults to "default" when omitted.
        realm: Option<String>,
        /// Max number of results to return.
        top_k: u32,
        /// Score floor in `[0.0, 1.0]`.
        min_score: f32,
    },
    /// Create a new named realm.
    CreateRealm {
        /// Realm name (must be unique).
        name: String,
    },
    /// Destroy a named realm and all its indexed documents.
    DestroyRealm {
        /// Realm name.
        name: String,
    },
    /// Add a workspace root to a realm and index its markdown files.
    AddRoot {
        /// Realm name.
        realm: String,
        /// Filesystem path to add.
        root: std::path::PathBuf,
    },
    /// Remove a workspace root from a realm, unindexing its documents.
    RemoveRoot {
        /// Realm name.
        realm: String,
        /// Filesystem path to remove.
        root: std::path::PathBuf,
    },
    /// Get aggregate statistics for a named realm.
    RealmStats {
        /// Realm name (e.g. "default").
        realm: String,
        /// Include semantic duplicate counts.
        check_duplicates: bool,
        /// Include aggregate token estimation.
        include_token_counts: bool,
    },
    /// Export the full document index for a single document.
    ExportIndex {
        /// Target document.
        uri: DocumentUri,
        /// Realm to query. Defaults to "default" when `None`.
        realm: Option<String>,
    },
    /// Get a dependency graph showing inter-document links.
    DependencyGraph {
        /// Realm name (e.g. "default").
        realm: String,
        /// Output format: "json" or "dot".
        format: String,
    },
    /// Search workspace files by regex pattern with optional glob file filter.
    SearchForPattern {
        /// Regex pattern to search for. Must not be empty or whitespace-only.
        pattern: String,
        /// Optional glob filter (e.g. `"*.md"`, `"**/*.rs"`). When the glob has no `/`,
        /// it is matched against the filename only; otherwise against the full path.
        include_glob: Option<String>,
        /// Lines of context around each match. Clamped to `[0, 20]`.
        context_lines: u32,
        /// Maximum total matches to return. Clamped to `[1, 500]`.
        limit: u32,
        /// Case-insensitive regex matching.
        case_insensitive: bool,
        /// Realm to search. Defaults to `"default"` when `None`.
        realm: Option<String>,
    },
    /// Search workspace documents by text, frontmatter, and property queries.
    SearchWorkspace {
        /// Free-text search query. Case-insensitive substring match against title, heading
        /// text, frontmatter values, and property values. `None` means no text filter.
        query: Option<String>,
        /// Filter: only include docs where `frontmatter[key]` value contains the given string.
        /// Key match is case-insensitive and exact. Value match is case-insensitive substring.
        /// For list frontmatter values (e.g. `aliases`), any element matching is sufficient.
        frontmatter_filter: Option<(String, String)>,
        /// Filter: only include docs where Logseq property `key:: value` matches.
        /// Key is case-insensitive exact match; value is case-insensitive substring.
        property_filter: Option<(String, String)>,
        /// Filter: only include docs that have this tag (case-insensitive, exact name after `#`).
        tag_filter: Option<String>,
        /// Realm to search. Defaults to `"default"` when `None`.
        realm: Option<String>,
        /// Max results to return. `0` returns empty (not an error). Clamped to 100 silently.
        limit: u32,
    },
    /// Analyse the link graph of a realm: orphans, hubs, broken links, clusters, stats.
    GraphAnalysis {
        /// Realm to analyse. Defaults to `"default"` when `None`.
        realm: Option<String>,
        /// Number of top hub documents to return (by incoming link count). Default 10.
        top_n_hubs: u32,
        /// Whether to compute weakly-connected clusters (can be expensive for large workspaces).
        include_clusters: bool,
    },
    /// Compute diagnostics (broken links, duplicate headings, unclosed XML tags) for a file or
    /// all files in a realm.
    GetDiagnostics {
        /// Optional specific document URI to check. When `None`, all documents in the realm are
        /// checked.
        uri: Option<crate::DocumentUri>,
        /// Realm to query. Defaults to `"default"` when `None`.
        realm: Option<String>,
    },
    /// Export a pipe-delimited docs_index block from realm state.
    ExportDocsIndex {
        /// Realm to export. Defaults to `"default"` when `None`.
        realm: Option<String>,
        /// Override the `[name]` prefix for each root entry.
        name_override: Option<String>,
    },
    /// Enrich a document's outline with LLM-generated summaries.
    EnrichDocument {
        /// Target document.
        uri: DocumentUri,
        /// Realm to query. Defaults to `"default"` when `None`.
        realm: Option<String>,
        /// Directory for sidecar files. When `None`, uses `.markymark/` under the workspace root.
        sidecar_dir: Option<std::path::PathBuf>,
        /// Force re-enrichment even if sidecar is fresh.
        force: bool,
    },

    /// Recommend documents matching an intent query using combined text + graph ranking.
    RecommendDocs {
        /// Intent or search query.
        query: String,
        /// Realm to query. Defaults to `"default"` when `None`.
        realm: Option<String>,
        /// Maximum recommendations to return.
        top_k: u32,
        /// Whether to include per-section summaries from sidecars.
        include_sections: bool,
    },

    /// Run curation diagnostics composing graph-analysis + diagnostics for actionable suggestions.
    CurationDiagnostics {
        /// Realm to diagnose. Defaults to `"default"` when `None`.
        realm: Option<String>,
        /// Whether to generate cross-link suggestions (default true).
        include_suggestions: bool,
        /// Maximum suggestions to return (default 20).
        max_suggestions: u32,
        /// Maximum items per diagnostic category (default 50).
        max_items_per_category: u32,
    },
}

/// A single match result from a regex pattern search.
#[derive(Debug, Clone)]
pub struct PatternMatch {
    /// Document URI where the match was found.
    pub uri: DocumentUri,
    /// 0-based line number of the match.
    pub line: u32,
    /// 0-based character offset of the match start within the line.
    pub column: u32,
    /// The text that the regex matched.
    pub match_text: String,
    /// The full line containing the match (trailing `\r` stripped).
    pub line_text: String,
    /// Lines before the match line (empty if `context_lines` is 0 or match is at file start).
    pub context_before: Vec<String>,
    /// Lines after the match line (empty if `context_lines` is 0 or match is at file end).
    pub context_after: Vec<String>,
    /// 0-based line number of `context_before[0]`.
    pub context_start_line: u32,
}

/// A section summary within a recommended document.
#[derive(Debug, Clone)]
pub struct RecommendedSection {
    /// Heading path (e.g. "Overview > Getting Started").
    pub heading_path: String,
    /// Heading level (1-6).
    pub level: u8,
    /// LLM-generated summary text.
    pub summary: String,
}

/// A single document recommendation from the recommend-docs operation.
#[derive(Debug, Clone)]
pub struct DocRecommendation {
    /// Document URI.
    pub uri: DocumentUri,
    /// Document title.
    pub title: String,
    /// Combined relevance score (0.0-1.0).
    pub relevance_score: f32,
    /// Text search score (0.0-1.0).
    pub search_score: f32,
    /// Normalized graph hub score (0.0-1.0).
    pub hub_score: f32,
    /// Fields that matched the query.
    pub matched_fields: Vec<String>,
    /// Document tags.
    pub tags: Vec<String>,
    /// Document-level summary from sidecar.
    pub document_summary: Option<String>,
    /// Per-section summaries from sidecar.
    pub sections: Option<Vec<RecommendedSection>>,
}

/// A suggestion for improving documentation quality (cross-link, index page, etc.).
#[derive(Debug, Clone)]
pub struct CurationSuggestion {
    /// Source document URI (the document that should add a link).
    pub source_doc: DocumentUri,
    /// Target document URI (the document being linked to).
    pub target_doc: DocumentUri,
    /// Human-readable reason for the suggestion.
    pub reason: String,
    /// Type of suggestion.
    pub suggestion_type: CurationSuggestionType,
}

/// The type of curation suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurationSuggestionType {
    /// Add a cross-link between two documents.
    CrossLink,
    /// Reduce orphan status by linking to a hub.
    ReduceOrphan,
}

/// A document with its connectivity score.
#[derive(Debug, Clone)]
pub struct ConnectivityDoc {
    /// Document URI.
    pub uri: DocumentUri,
    /// Total link count (in-degree + out-degree).
    pub connectivity: u32,
    /// Incoming link count.
    pub in_degree: u32,
    /// Outgoing link count.
    pub out_degree: u32,
}

/// Aggregate statistics for the curation report.
#[derive(Debug, Clone)]
pub struct CurationStats {
    /// Total documents in realm.
    pub total_docs: u32,
    /// Number of orphan documents.
    pub orphan_count: u32,
    /// Percentage of orphan documents (0.0-100.0).
    pub orphan_percentage: f32,
    /// Average connectivity across all documents.
    pub avg_connectivity: f32,
    /// Median connectivity across all documents.
    pub median_connectivity: f32,
    /// Total broken links from diagnostics.
    pub broken_link_count: u32,
}

/// Full curation diagnostics report.
#[derive(Debug, Clone)]
pub struct CurationReportData {
    /// Documents with no resolved links in or out.
    pub orphan_docs: Vec<DocumentUri>,
    /// Documents with connectivity below the median and threshold.
    pub low_connectivity_docs: Vec<ConnectivityDoc>,
    /// Actionable cross-link suggestions.
    pub suggestions: Vec<CurationSuggestion>,
    /// Aggregate statistics.
    pub stats: CurationStats,
}

/// Transport-agnostic interface for executing core operations.
///
/// Both LSP and MCP transports call into this trait so indexing and
/// resolution logic stays shared in one place.
#[async_trait]
pub trait CoreEngine: Send + Sync {
    /// Execute a core operation and return the transport-neutral result.
    async fn execute(&self, operation: CoreOperation) -> CoreOperationResult;
}

/// An owned node in the hierarchical outline tree.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineTreeNode {
    /// Heading title (empty string for root node).
    pub title: String,
    /// Heading level (0 for root, 1-6 for headings).
    pub level: u8,
    /// Source range of the heading line.
    pub range: Range,
    /// Section text content (when include_text requested).
    pub text: Option<String>,
    /// LLM-generated summary (from sidecar enrichment).
    pub summary: Option<String>,
    /// Child nodes.
    pub children: Vec<OutlineTreeNode>,
}

/// The result of a core engine operation.
#[derive(Debug)]
pub enum CoreOperationResult {
    /// An outline (list of heading descriptions, flat format).
    Outline(Vec<String>),
    /// A hierarchical outline tree.
    OutlineTree(OutlineTreeNode),
    /// A list of locations (uri, range).
    Locations(Vec<(DocumentUri, Range)>),
    /// A workspace edit (uri, list of (range, replacement text)).
    WorkspaceEdit(Vec<(DocumentUri, Vec<(Range, String)>)>),
    /// A list of symbols (name, uri, range).
    Symbols(Vec<(String, DocumentUri, Range)>),
    /// Semantic search results.
    SemanticMatches(Vec<SemanticSearchMatch>),
    /// Realm info: name, root count, document count.
    RealmInfo {
        /// Realm name.
        name: String,
        /// Number of tracked workspace roots.
        root_count: usize,
        /// Number of indexed documents.
        document_count: usize,
    },
    /// Realm statistics: aggregate counts across all indexed documents.
    RealmStats {
        /// Realm name.
        name: String,
        /// Number of tracked workspace roots.
        root_count: usize,
        /// Number of indexed documents.
        document_count: usize,
        /// Total headings across all documents.
        heading_count: usize,
        /// Total XML tags across all documents.
        xml_tag_count: usize,
        /// Total wiki links across all documents.
        wiki_link_count: usize,
        /// Total markdown links across all documents.
        markdown_link_count: usize,
        /// Number of structured (non-markdown) documents indexed.
        structured_doc_count: usize,
        /// Total key paths across all structured documents.
        key_path_count: usize,
        /// Optional count of near-duplicate document pairs.
        duplicate_pairs: Option<usize>,
        /// Optional aggregate token count across realm documents.
        total_tokens: Option<u64>,
    },
    /// Exported document index: full structured data for a single document.
    DocumentExport {
        /// Document URI.
        uri: DocumentUri,
        /// The kind of document (Markdown, JSON, YAML, etc.).
        document_kind: Option<DocumentKind>,
        /// Heading texts and levels.
        headings: Vec<(String, u8, Range)>,
        /// XML tag names with ranges.
        xml_tags: Vec<(String, Range)>,
        /// Wiki link targets with ranges.
        wiki_links: Vec<(String, Option<String>, Range)>,
        /// Markdown link URLs with ranges.
        markdown_links: Vec<(String, String, Range)>,
        /// Frontmatter key-value pairs. String values are wrapped as single-element vecs.
        frontmatter: Vec<(String, Vec<String>)>,
        /// Logseq inline properties. String values are wrapped as single-element vecs.
        properties: Vec<(String, Vec<String>)>,
    },
    /// A dependency graph in the requested format (json or dot).
    DependencyGraph {
        /// Realm name.
        realm: String,
        /// Output format used.
        format: String,
        /// Serialized graph content.
        content: String,
    },
    /// Results from a workspace-wide search.
    WorkspaceSearchResults {
        /// Realm that was searched.
        realm: String,
        /// The original query, if any.
        query: Option<String>,
        /// Ranked search results.
        results: Vec<WorkspaceSearchResult>,
    },
    /// Results from a regex pattern search across workspace files.
    PatternSearchResults {
        /// Realm that was searched.
        realm: String,
        /// The original pattern.
        pattern: String,
        /// Number of files that were actually read and searched.
        files_searched: u32,
        /// Number of files skipped (read error, size limit, or missing path).
        files_skipped: u32,
        /// Matches found (up to `limit`).
        matches: Vec<PatternMatch>,
        /// `true` when the result was truncated at `limit`.
        truncated: bool,
    },
    /// Results from a link graph analysis of the workspace.
    GraphAnalysis {
        /// Realm that was analysed.
        realm: String,
        /// Total markdown documents in the realm.
        total_docs: u32,
        /// Total resolved internal links (wiki + local markdown).
        total_internal_links: u32,
        /// Documents with zero incoming AND zero outgoing internal links.
        orphans: Vec<DocumentUri>,
        /// Top documents by incoming link count: `(uri, incoming_count)`, sorted descending.
        hubs: Vec<(DocumentUri, u32)>,
        /// Outgoing links that could not be resolved to any indexed document.
        /// Each entry is `(source_uri, target_string, kind)` where kind is `"wiki"` or `"markdown"`.
        broken_links: Vec<(DocumentUri, String, String)>,
        /// Weakly-connected clusters. `None` when `include_clusters` was `false`.
        clusters: Option<Vec<Vec<DocumentUri>>>,
    },
    /// Diagnostics for one or more files.
    Diagnostics {
        /// Realm that was checked.
        realm: String,
        /// Per-file diagnostics: `(document_uri, diagnostics)`.
        /// Only files that have at least one diagnostic are included.
        items: Vec<(crate::DocumentUri, Vec<CoreDiagnostic>)>,
    },
    /// Exported docs_index entries (one per root).
    DocsIndexExport {
        /// Realm that was exported.
        realm: String,
        /// Pipe-delimited docs_index entry strings.
        entries: Vec<String>,
        /// Number of markdown documents included.
        doc_count: usize,
        /// Number of roots that produced entries.
        root_count: usize,
        /// Number of documents skipped (URI didn't match any root).
        skipped_count: usize,
    },
    /// Result of enriching a document with LLM summaries.
    EnrichmentResult {
        /// Document URI that was enriched.
        uri: DocumentUri,
        /// Number of sections that were summarized.
        sections_enriched: usize,
        /// Whether the sidecar was fresh (skipped) or regenerated.
        was_stale: bool,
        /// Model used for enrichment.
        model_id: String,
    },
    /// Result of recommend-docs: ranked document recommendations.
    Recommendations {
        /// Realm that was searched.
        realm: String,
        /// The original query.
        query: String,
        /// Ranked recommendations: (uri, title, relevance_score, search_score, hub_score,
        /// matched_fields, tags, document_summary, sections).
        results: Vec<DocRecommendation>,
    },
    /// Result of curation diagnostics: orphans, low-connectivity docs, suggestions.
    CurationReport {
        /// Realm that was analyzed.
        realm: String,
        /// Full curation report.
        report: CurationReportData,
    },
    /// Success with no payload.
    Ok,
    /// An error occurred.
    Error(CoreError),
}
