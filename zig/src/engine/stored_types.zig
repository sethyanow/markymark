// Stored types: engine-internal data structures for serialized document state.
//
// Pure data types with no dependencies. Used by document.zig, exports.zig,
// get_result.zig, ffi_types.zig, and document_test.zig.

/// Maximum number of fenced code block ranges tracked on the stack.
/// Limits stack allocation to ~2 KB (256 × 8 bytes). Documents with more
/// than 256 fenced blocks will have tags/block-ids inside excess fences
/// silently included — a benign false positive on extreme inputs.
pub const FENCE_MAP_MAX: u32 = 256;

// ── Stored types (engine-internal) ──────────────────────────────────

pub const Position = struct {
    line: u32,
    col: u32,
};

pub const StoredHeading = struct {
    text: []const u8, // owned
    slug: []const u8, // owned
    source_offset: u32,
    start: Position,
    end: Position,
    level: u8,
};

pub const StoredLink = struct {
    text: []const u8, // owned
    target: []const u8, // owned
    source_offset: u32,
    start: Position,
    end: Position,
    is_wiki: bool,
};

pub const StoredTag = struct {
    name: []const u8, // owned
    source_offset: u32,
    start: Position,
};

pub const StoredCodeSpan = struct {
    text: []const u8, // owned decoded text
    source_offset: u32, // byte offset of opening backtick
    end_offset: u32, // byte offset past closing backtick
    start: Position, // line:col of opening backtick
    end: Position, // line:col past closing backtick
};

pub const StoredBlockId = struct {
    id: []const u8, // owned
    source_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredTask = struct {
    state: u8,
    text: []const u8, // owned
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredEmbed = struct {
    target: []const u8, // owned
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredCallout = struct {
    callout_type: []const u8, // owned, lowercase alpha
    title: ?[]const u8, // owned, null if no title
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredBlockRef = struct {
    uuid: []const u8, // owned, 36-char UUID
    source_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredQueryBlock = struct {
    query: []const u8, // owned, the query text
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredLinkDefinition = struct {
    label: []const u8, // owned, the link label
    url: []const u8, // owned, the URL
    title: ?[]const u8, // owned, optional title
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
};

pub const StoredProperty = struct {
    key: []const u8, // owned, the property key
    value: []const u8, // owned, the raw value text
    value_type: u8, // 0=string, 1=list, 2=page_ref
};

pub const StoredXmlTag = struct {
    tag_name: []const u8, // owned
    raw_html: []const u8, // owned, opening tag HTML for attribute parsing
    source_offset: u32,
    end_offset: u32,
    start: Position,
    end: Position,
    is_self_closing: bool,
    is_unclosed: bool,
    is_inline: bool,
};
