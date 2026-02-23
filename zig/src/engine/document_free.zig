// Free helpers: memory deallocation for stored document types.
//
// Pure utility functions — take (allocator, slice/list) and free memory.
// No dependencies on DocumentEngine state.

const std = @import("std");
const Allocator = std.mem.Allocator;
const stored_types = @import("stored_types.zig");

const StoredHeading = stored_types.StoredHeading;
const StoredLink = stored_types.StoredLink;
const StoredCodeSpan = stored_types.StoredCodeSpan;
const StoredTag = stored_types.StoredTag;
const StoredBlockId = stored_types.StoredBlockId;
const StoredTask = stored_types.StoredTask;
const StoredEmbed = stored_types.StoredEmbed;
const StoredCallout = stored_types.StoredCallout;
const StoredBlockRef = stored_types.StoredBlockRef;
const StoredQueryBlock = stored_types.StoredQueryBlock;
const StoredLinkDefinition = stored_types.StoredLinkDefinition;
const StoredProperty = stored_types.StoredProperty;

// ── Slice free helpers ──────────────────────────────────────────────

pub fn freeHeadings(allocator: Allocator, headings: []StoredHeading) void {
    for (headings) |h| {
        allocator.free(h.text);
        allocator.free(h.slug);
    }
    if (headings.len > 0) allocator.free(headings);
}

pub fn freeLinks(allocator: Allocator, links: []StoredLink) void {
    for (links) |l| {
        allocator.free(l.text);
        allocator.free(l.target);
    }
    if (links.len > 0) allocator.free(links);
}

pub fn freeCodeSpans(allocator: Allocator, code_spans: []StoredCodeSpan) void {
    for (code_spans) |cs| {
        allocator.free(cs.text);
    }
    if (code_spans.len > 0) allocator.free(code_spans);
}

pub fn freeTags(allocator: Allocator, tags: []StoredTag) void {
    for (tags) |t| {
        allocator.free(t.name);
    }
    if (tags.len > 0) allocator.free(tags);
}

pub fn freeBlockIds(allocator: Allocator, block_ids: []StoredBlockId) void {
    for (block_ids) |b| {
        allocator.free(b.id);
    }
    if (block_ids.len > 0) allocator.free(block_ids);
}

pub fn freeTasks(allocator: Allocator, tasks: []StoredTask) void {
    for (tasks) |t| {
        allocator.free(t.text);
    }
    if (tasks.len > 0) allocator.free(tasks);
}

pub fn freeEmbeds(allocator: Allocator, embeds: []StoredEmbed) void {
    for (embeds) |e| {
        allocator.free(e.target);
    }
    if (embeds.len > 0) allocator.free(embeds);
}

pub fn freeCallouts(allocator: Allocator, callouts: []StoredCallout) void {
    for (callouts) |c| {
        allocator.free(c.callout_type);
        if (c.title) |t| allocator.free(t);
    }
    if (callouts.len > 0) allocator.free(callouts);
}

pub fn freeBlockRefs(allocator: Allocator, block_refs: []StoredBlockRef) void {
    for (block_refs) |br| {
        allocator.free(br.uuid);
    }
    if (block_refs.len > 0) allocator.free(block_refs);
}

pub fn freeQueryBlocks(allocator: Allocator, query_blocks: []StoredQueryBlock) void {
    for (query_blocks) |qb| {
        allocator.free(qb.query);
    }
    if (query_blocks.len > 0) allocator.free(query_blocks);
}

pub fn freeLinkDefinitions(allocator: Allocator, link_defs: []StoredLinkDefinition) void {
    for (link_defs) |ld| {
        allocator.free(ld.label);
        allocator.free(ld.url);
        if (ld.title) |t| allocator.free(t);
    }
    if (link_defs.len > 0) allocator.free(link_defs);
}

pub fn freeProperties(allocator: Allocator, props: []StoredProperty) void {
    for (props) |p| {
        allocator.free(p.key);
        allocator.free(p.value);
    }
    if (props.len > 0) allocator.free(props);
}

// ── ArrayList free helpers ──────────────────────────────────────────

pub fn freeStoredHeadingsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredHeading), free_texts: bool) void {
    for (list.items) |h| {
        // h.text was transferred from extraction; only free it when texts_transferred=true
        // (i.e., after extraction.headings/links slice containers were freed at line 289-290).
        if (free_texts) allocator.free(h.text);
        allocator.free(h.slug);
    }
    list.deinit(allocator);
}

pub fn freeStoredLinksList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredLink), free_texts: bool) void {
    // l.text and l.target were transferred from extraction; free them only when
    // texts_transferred=true (after extraction slice containers freed at line 289-290).
    if (free_texts) {
        for (list.items) |l| {
            allocator.free(l.text);
            allocator.free(l.target);
        }
    }
    list.deinit(allocator);
}

pub fn freeStoredCodeSpansList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredCodeSpan), free_texts: bool) void {
    // cs.text was transferred from extraction; free only when texts_transferred=true.
    if (free_texts) {
        for (list.items) |cs| {
            allocator.free(cs.text);
        }
    }
    list.deinit(allocator);
}

pub fn freeStoredTagsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredTag)) void {
    for (list.items) |t| {
        allocator.free(t.name);
    }
    list.deinit(allocator);
}

pub fn freeStoredBlockIdsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredBlockId)) void {
    for (list.items) |b| {
        allocator.free(b.id);
    }
    list.deinit(allocator);
}

pub fn freeStoredTasksList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredTask), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |t| {
            allocator.free(t.text);
        }
    }
    list.deinit(allocator);
}

pub fn freeStoredEmbedsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredEmbed), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |e| {
            allocator.free(e.target);
        }
    }
    list.deinit(allocator);
}

pub fn freeStoredCalloutsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredCallout), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |c| {
            allocator.free(c.callout_type);
            if (c.title) |t| allocator.free(t);
        }
    }
    list.deinit(allocator);
}

pub fn freeStoredBlockRefsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredBlockRef), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |br| {
            allocator.free(br.uuid);
        }
    }
    list.deinit(allocator);
}

pub fn freeStoredQueryBlocksList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredQueryBlock), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |qb| {
            allocator.free(qb.query);
        }
    }
    list.deinit(allocator);
}

pub fn freeStoredLinkDefsList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredLinkDefinition), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |ld| {
            allocator.free(ld.label);
            allocator.free(ld.url);
            if (ld.title) |t| allocator.free(t);
        }
    }
    list.deinit(allocator);
}

pub fn freeStoredPropertiesList(allocator: Allocator, list: *std.ArrayListUnmanaged(StoredProperty), free_texts: bool) void {
    if (free_texts) {
        for (list.items) |p| {
            allocator.free(p.key);
            allocator.free(p.value);
        }
    }
    list.deinit(allocator);
}
