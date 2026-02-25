// C ABI types shared between md4c FFI exports and Rust bindings.
// Fields ordered by alignment to avoid implicit padding holes.
// Both Zig extern struct and Rust #[repr(C)] MUST use identical field order.

const std = @import("std");

pub const CMd4cHeading = extern struct {
    source_offset: u32, // byte offset of '#' (ATX) or text start (setext) in source
    text_offset: u32, // offset into text_blob for decoded heading text
    text_length: u32, // length in text_blob
    level: u8, // 1-6
    _padding: [3]u8, // explicit padding to 16-byte struct size
};
comptime {
    std.debug.assert(@sizeOf(CMd4cHeading) == 16);
}

pub const CMd4cLink = extern struct {
    source_offset: u32, // byte offset of '[' or '[[' in source
    text_offset: u32, // offset into text_blob for display text
    target_offset: u32, // offset into text_blob for href/target
    text_length: u32, // length in text_blob
    target_length: u32, // length in text_blob
    is_wiki: u8, // 1 for [[wiki]] links, 0 otherwise
    _padding: [3]u8, // explicit padding to 24-byte struct size
};
comptime {
    std.debug.assert(@sizeOf(CMd4cLink) == 24);
}

pub const CMd4cCodeSpan = extern struct {
    source_offset: u32, // byte offset of opening backtick in source
    end_offset: u32, // byte offset past closing backtick in source
    text_offset: u32, // offset into text_blob for decoded text
    text_length: u32, // length in text_blob
};
comptime {
    std.debug.assert(@sizeOf(CMd4cCodeSpan) == 16);
}

pub const CMd4cTask = extern struct {
    source_offset: u32,
    end_offset: u32,
    text_offset: u32,
    text_length: u32,
    state: u8,
    _pad: [3]u8 = .{ 0, 0, 0 },
};
comptime {
    std.debug.assert(@sizeOf(CMd4cTask) == 20);
}

pub const CMd4cEmbed = extern struct {
    source_offset: u32,
    end_offset: u32,
    target_offset: u32,
    target_length: u32,
};
comptime {
    std.debug.assert(@sizeOf(CMd4cEmbed) == 16);
}

pub const CMd4cCallout = extern struct {
    source_offset: u32, // byte offset of '>' in source
    end_offset: u32,
    type_offset: u32, // offset into text_blob for callout type string
    type_length: u32,
    title_offset: u32, // offset into text_blob for title (0 if no title)
    title_length: u32, // 0 means no title
};
comptime {
    std.debug.assert(@sizeOf(CMd4cCallout) == 24);
}

pub const CMd4cBlockRef = extern struct {
    source_offset: u32, // byte offset of first '(' of '((' in source
    uuid_offset: u32, // offset into text_blob for UUID string
    uuid_length: u32, // should be 36 for valid UUIDs
};
comptime {
    std.debug.assert(@sizeOf(CMd4cBlockRef) == 12);
}

pub const CMd4cQueryBlock = extern struct {
    source_offset: u32, // byte offset of first '{' of '{{' in source
    end_offset: u32, // byte offset past closing '}}'
    query_offset: u32, // offset into text_blob for query text
    query_length: u32,
};
comptime {
    std.debug.assert(@sizeOf(CMd4cQueryBlock) == 16);
}

pub const CMd4cLinkDefinition = extern struct {
    source_offset: u32, // byte offset of '[' in source
    end_offset: u32, // byte offset past end of definition
    label_offset: u32, // offset into text_blob for label
    label_length: u32,
    url_offset: u32, // offset into text_blob for URL
    url_length: u32,
    title_offset: u32, // offset into text_blob for title (0 if no title)
    title_length: u32, // 0 if no title
};
comptime {
    std.debug.assert(@sizeOf(CMd4cLinkDefinition) == 32);
}

pub const CMd4cProperty = extern struct {
    key_offset: u32, // offset into text_blob for key
    key_length: u32,
    value_offset: u32, // offset into text_blob for raw value
    value_length: u32,
    value_type: u8, // 0=string, 1=list, 2=page_ref
    _pad: [3]u8 = .{ 0, 0, 0 },
};
comptime {
    std.debug.assert(@sizeOf(CMd4cProperty) == 20);
}

pub const CMd4cXmlTag = extern struct {
    source_offset: u32,
    end_offset: u32,
    tag_name_offset: u32, // offset into text_blob
    tag_name_length: u32,
    raw_html_offset: u32, // offset into text_blob
    raw_html_length: u32,
    is_self_closing: u8,
    is_unclosed: u8,
    is_inline: u8,
    _pad: [1]u8 = .{0},
};
comptime {
    std.debug.assert(@sizeOf(CMd4cXmlTag) == 28);
}

// Pointers grouped first, then u32 counts — avoids internal padding on 64-bit.
pub const CMd4cResult = extern struct {
    headings: ?[*]CMd4cHeading, // Zig-allocated array, freed by marky_md4c_free
    links: ?[*]CMd4cLink, // Zig-allocated array, freed by marky_md4c_free
    code_spans: ?[*]CMd4cCodeSpan, // Zig-allocated array, freed by marky_md4c_free
    tasks: ?[*]CMd4cTask, // Zig-allocated array, freed by marky_md4c_free
    embeds: ?[*]CMd4cEmbed, // Zig-allocated array, freed by marky_md4c_free
    callouts: ?[*]CMd4cCallout, // Zig-allocated array, freed by marky_md4c_free
    block_refs: ?[*]CMd4cBlockRef, // Zig-allocated array, freed by marky_md4c_free
    query_blocks: ?[*]CMd4cQueryBlock, // Zig-allocated array, freed by marky_md4c_free
    link_definitions: ?[*]CMd4cLinkDefinition, // Zig-allocated array, freed by marky_md4c_free
    properties: ?[*]CMd4cProperty, // Zig-allocated array, freed by marky_md4c_free
    xml_tags: ?[*]CMd4cXmlTag, // Zig-allocated array, freed by marky_md4c_free
    text_blob: ?[*]const u8, // concatenated decoded texts, freed by marky_md4c_free
    headings_count: u32,
    links_count: u32,
    code_spans_count: u32,
    tasks_count: u32,
    embeds_count: u32,
    callouts_count: u32,
    block_refs_count: u32,
    query_blocks_count: u32,
    link_definitions_count: u32,
    properties_count: u32,
    xml_tags_count: u32,
    text_blob_len: u32,
};
comptime {
    // 12 pointers (96) + 12 u32 (48) = 144
    std.debug.assert(@sizeOf(CMd4cResult) == 144);
}
