// Extracted result types for ExtractionRenderer.
// Split from extraction_renderer.zig for marky-t5iv.

const std = @import("std");
const Allocator = std.mem.Allocator;

pub const ExtractedHeading = struct {
    text: []const u8, // owned
    offset: u32,
    level: u8,
};

pub const ExtractedLink = struct {
    text: []const u8, // owned
    target: []const u8, // owned
    offset: u32,
    end_offset: u32, // byte offset past the link's closing character
    is_wiki: bool,
};

pub const ExtractedCodeSpan = struct {
    text: []const u8, // owned decoded code span text
    offset: u32, // byte offset of opening backtick in source
    end_offset: u32, // byte offset past closing backtick in source
};

pub const ExtractedTask = struct {
    state: u8, // task mark char: ' ', 'x', 'X'
    text: []const u8, // owned by allocator
    offset: u32, // byte offset of '[' in [x]
    end_offset: u32, // byte offset past task text
};

pub const ExtractedEmbed = struct {
    target: []const u8, // owned by allocator
    offset: u32, // byte offset of '!' before '![['
    end_offset: u32, // byte offset past ']]'
};

pub const ExtractedCallout = struct {
    callout_type: []const u8, // owned, lowercase alpha (e.g. "note", "warning")
    title: ?[]const u8, // owned, null if no title text after [!type]
    offset: u32, // byte offset of '>' in source
    end_offset: u32, // byte offset past callout content
};

pub const ExtractedBlockRef = struct {
    uuid: []const u8, // owned, 36-char UUID preserved as-is from source
    offset: u32, // byte offset of first '(' of '((' in source
};

pub const ExtractedQueryBlock = struct {
    query: []const u8, // owned, the query text (trimmed)
    offset: u32, // byte offset of first '{' of '{{' in source
    end_offset: u32, // byte offset past closing '}}'
};

pub const ExtractedLinkDefinition = struct {
    label: []const u8, // owned, the link label
    url: []const u8, // owned, the URL
    title: ?[]const u8, // owned, optional title (null if absent)
    offset: u32, // byte offset of '[' in source
    end_offset: u32, // byte offset past end of line
};

pub const ExtractedProperty = struct {
    key: []const u8, // owned, the property key (trimmed)
    value: []const u8, // owned, the raw value text (trimmed)
    value_type: u8, // 0=string, 1=list, 2=page_ref
};

pub const ExtractedXmlTag = struct {
    tag_name: []const u8, // owned (duped from src_text)
    raw_html: []const u8, // owned (opening tag HTML for attribute parsing)
    offset: u32, // start byte in source
    end_offset: u32, // end byte (includes closing tag if matched)
    is_self_closing: bool,
    is_unclosed: bool,
};

pub const ExtractionResult = struct {
    headings: []ExtractedHeading,
    links: []ExtractedLink,
    code_spans: []ExtractedCodeSpan,
    tasks: []ExtractedTask,
    embeds: []ExtractedEmbed,
    callouts: []ExtractedCallout,
    block_refs: []ExtractedBlockRef,
    query_blocks: []ExtractedQueryBlock,
    link_definitions: []ExtractedLinkDefinition,
    properties: []ExtractedProperty,
    xml_tags: []ExtractedXmlTag,
    allocator: Allocator,

    pub fn deinit(self: *ExtractionResult) void {
        for (self.headings) |h| {
            self.allocator.free(h.text);
        }
        self.allocator.free(self.headings);
        for (self.links) |l| {
            self.allocator.free(l.text);
            self.allocator.free(l.target);
        }
        self.allocator.free(self.links);
        for (self.code_spans) |cs| {
            self.allocator.free(cs.text);
        }
        self.allocator.free(self.code_spans);
        for (self.tasks) |t| {
            self.allocator.free(t.text);
        }
        self.allocator.free(self.tasks);
        for (self.embeds) |e| {
            self.allocator.free(e.target);
        }
        self.allocator.free(self.embeds);
        for (self.callouts) |c| {
            self.allocator.free(c.callout_type);
            if (c.title) |t| self.allocator.free(t);
        }
        self.allocator.free(self.callouts);
        for (self.block_refs) |br| {
            self.allocator.free(br.uuid);
        }
        self.allocator.free(self.block_refs);
        for (self.query_blocks) |qb| {
            self.allocator.free(qb.query);
        }
        self.allocator.free(self.query_blocks);
        for (self.link_definitions) |ld| {
            self.allocator.free(ld.label);
            self.allocator.free(ld.url);
            if (ld.title) |t| self.allocator.free(t);
        }
        self.allocator.free(self.link_definitions);
        for (self.properties) |p| {
            self.allocator.free(p.key);
            self.allocator.free(p.value);
        }
        self.allocator.free(self.properties);
        for (self.xml_tags) |xt| {
            self.allocator.free(xt.tag_name);
            self.allocator.free(xt.raw_html);
        }
        self.allocator.free(self.xml_tags);
    }
};
