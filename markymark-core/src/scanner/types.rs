//! Result types returned by scan backend operations.

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

use std::fmt;

/// Errors returned by scan backend operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    /// Invalid or malformed input text.
    InvalidInput(String),
    /// Internal scanner failure.
    InternalError(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "scan: invalid input: {msg}"),
            Self::InternalError(msg) => write!(f, "scan: internal error: {msg}"),
        }
    }
}

impl std::error::Error for ScanError {}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// A heading found by a scan backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingResult {
    /// Heading text content.
    pub text: String,
    /// Byte offset of the heading in the source text.
    pub offset: u32,
    /// Heading level (1–6).
    pub level: u8,
}

/// Link type discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanLinkType {
    /// Standard markdown link `[text](url)`.
    Markdown,
    /// Wiki-style link `[[target]]` or `[[target|display]]`.
    Wiki,
}

/// A link found by a scan backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkResult {
    /// Byte offset of the link in the source text.
    pub offset: u32,
    /// Display text of the link.
    pub text: String,
    /// Link target / URL.
    pub target: String,
    /// Whether this is a markdown or wiki link.
    pub link_type: ScanLinkType,
}

/// A tag found by a scan backend (e.g. `#topic`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagResult {
    /// Tag name without the leading `#`.
    pub name: String,
    /// Byte offset of the `#` in the source text.
    pub offset: u32,
}

/// A block ID found by a scan backend (e.g. `^block-id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockIdResult {
    /// Block ID without the leading `^`.
    pub id: String,
    /// Byte offset of the `^` in the source text.
    pub offset: u32,
}

/// An inline code span found by a scan backend (e.g. `` `hello` ``).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSpanResult {
    /// The backtick-delimited text content (decoded).
    pub text: String,
    /// Byte offset of the opening backtick in the source text.
    pub offset: u32,
    /// Byte offset one past the closing backtick in the source text.
    pub end_offset: u32,
}

/// A task list item found by a scan backend (e.g. `- [x] Done`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskResult {
    /// Checkbox state: "checked" or "unchecked".
    pub state: String,
    /// Task description text.
    pub text: String,
    /// Byte offset of the `[` in the source text.
    pub offset: u32,
    /// Byte offset past the task text.
    pub end_offset: u32,
}

/// An embed reference found by a scan backend (e.g. `![[target]]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedResult {
    /// The embedded resource path.
    pub target: String,
    /// Byte offset of `!` in the source text.
    pub offset: u32,
    /// Byte offset past `]]`.
    pub end_offset: u32,
}

/// A callout found by a scan backend (e.g. `> [!note] Title`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalloutResult {
    /// Callout type (e.g. "note", "warning", "tip").
    pub callout_type: String,
    /// Optional callout title.
    pub title: Option<String>,
    /// Byte offset of `>` in the source text.
    pub offset: u32,
    /// Byte offset past the callout block.
    pub end_offset: u32,
}

/// A block reference found by a scan backend (e.g. `((uuid))`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRefResult {
    /// The UUID string.
    pub uuid: String,
    /// Byte offset of first `(` in `((uuid))` in the source text.
    pub offset: u32,
}

/// Combined result from a single-pass scan of headings, links, code spans, tasks, embeds,
/// callouts, and block refs.
#[derive(Debug, Default)]
pub struct ScanAllResult {
    /// Headings extracted from the document.
    pub headings: Vec<HeadingResult>,
    /// Links extracted from the document.
    pub links: Vec<LinkResult>,
    /// Inline code spans extracted from the document.
    pub code_spans: Vec<CodeSpanResult>,
    /// Task list items extracted from the document.
    pub tasks: Vec<TaskResult>,
    /// Embed references extracted from the document.
    pub embeds: Vec<EmbedResult>,
    /// Callout blockquotes extracted from the document.
    pub callouts: Vec<CalloutResult>,
    /// Block references extracted from the document.
    pub block_refs: Vec<BlockRefResult>,
}
