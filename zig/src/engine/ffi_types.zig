// C ABI types for DocumentEngine result export.
// Field ordering matches Rust #[repr(C)] mirrors exactly.

const std = @import("std");

pub const CEngineHeading = extern struct {
    text_offset: u32,
    text_length: u32,
    slug_offset: u32,
    slug_length: u32,
    source_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    level: u8,
    _pad: [3]u8 = .{ 0, 0, 0 },
};
comptime {
    std.debug.assert(@sizeOf(CEngineHeading) == 40);
}

pub const CEngineLink = extern struct {
    text_offset: u32,
    text_length: u32,
    target_offset: u32,
    target_length: u32,
    source_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    is_wiki: u8,
    _pad: [3]u8 = .{ 0, 0, 0 },
};
comptime {
    std.debug.assert(@sizeOf(CEngineLink) == 40);
}

pub const CEngineCodeSpan = extern struct {
    text_offset: u32,
    text_length: u32,
    source_offset: u32,
    end_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
};
comptime {
    std.debug.assert(@sizeOf(CEngineCodeSpan) == 32);
}

pub const CEngineTag = extern struct {
    name_offset: u32,
    name_length: u32,
    source_offset: u32,
    start_line: u32,
    start_col: u32,
};
comptime {
    std.debug.assert(@sizeOf(CEngineTag) == 20);
}

pub const CEngineBlockId = extern struct {
    id_offset: u32,
    id_length: u32,
    source_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
};
comptime {
    std.debug.assert(@sizeOf(CEngineBlockId) == 28);
}

pub const CEngineTask = extern struct {
    text_offset: u32,
    text_length: u32,
    source_offset: u32,
    end_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    state: u8,
    _pad: [3]u8 = .{ 0, 0, 0 },
};
comptime {
    std.debug.assert(@sizeOf(CEngineTask) == 36);
}

pub const CEngineEmbed = extern struct {
    target_offset: u32,
    target_length: u32,
    source_offset: u32,
    end_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
};
comptime {
    std.debug.assert(@sizeOf(CEngineEmbed) == 32);
}

pub const CEngineCallout = extern struct {
    type_offset: u32,
    type_length: u32,
    title_offset: u32,
    title_length: u32,
    source_offset: u32,
    end_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
};
comptime {
    std.debug.assert(@sizeOf(CEngineCallout) == 40);
}

pub const CEngineBlockRef = extern struct {
    uuid_offset: u32,
    uuid_length: u32,
    source_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
};
comptime {
    std.debug.assert(@sizeOf(CEngineBlockRef) == 28);
}

pub const CEngineQueryBlock = extern struct {
    query_offset: u32,
    query_length: u32,
    source_offset: u32,
    end_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
};
comptime {
    std.debug.assert(@sizeOf(CEngineQueryBlock) == 32);
}

pub const CEngineLinkDefinition = extern struct {
    label_offset: u32,
    label_length: u32,
    url_offset: u32,
    url_length: u32,
    title_offset: u32,
    title_length: u32,
    source_offset: u32,
    end_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
};
comptime {
    std.debug.assert(@sizeOf(CEngineLinkDefinition) == 48);
}

pub const CEngineProperty = extern struct {
    key_offset: u32,
    key_length: u32,
    value_offset: u32,
    value_length: u32,
    value_type: u8,
    _pad: [3]u8 = .{ 0, 0, 0 },
};
comptime {
    std.debug.assert(@sizeOf(CEngineProperty) == 20);
}

pub const CEngineXmlTag = extern struct {
    tag_name_offset: u32,
    tag_name_length: u32,
    raw_html_offset: u32,
    raw_html_length: u32,
    source_offset: u32,
    end_offset: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    is_self_closing: u8,
    is_unclosed: u8,
    is_inline: u8,
    _pad: [1]u8 = .{0},
};
comptime {
    std.debug.assert(@sizeOf(CEngineXmlTag) == 44);
}

pub const CEngineResult = extern struct {
    headings: ?[*]CEngineHeading,
    links: ?[*]CEngineLink,
    code_spans: ?[*]CEngineCodeSpan,
    tags: ?[*]CEngineTag,
    block_ids: ?[*]CEngineBlockId,
    tasks: ?[*]CEngineTask,
    embeds: ?[*]CEngineEmbed,
    callouts: ?[*]CEngineCallout,
    block_refs: ?[*]CEngineBlockRef,
    query_blocks: ?[*]CEngineQueryBlock,
    link_definitions: ?[*]CEngineLinkDefinition,
    properties: ?[*]CEngineProperty,
    xml_tags: ?[*]CEngineXmlTag,
    line_starts: ?[*]u32,
    text_blob: ?[*]const u8,

    content_hash: u64,
    generation: u64,

    headings_count: u32,
    links_count: u32,
    code_spans_count: u32,
    tags_count: u32,
    block_ids_count: u32,
    tasks_count: u32,
    embeds_count: u32,
    callouts_count: u32,
    block_refs_count: u32,
    query_blocks_count: u32,
    link_definitions_count: u32,
    properties_count: u32,
    xml_tags_count: u32,
    line_starts_count: u32,
    text_blob_len: u32,
    token_estimate: u32,

    _reserved: [32]u8 = [_]u8{0} ** 32,
};
comptime {
    // 15 pointers (120) + 2 u64 (16) + 16 u32 (64) + 32 reserved = 232 bytes.
    std.debug.assert(@sizeOf(CEngineResult) == 232);
}
