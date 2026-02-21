// Tests for DocumentEngine and related helpers.
//
// Pulled out of document.zig to keep that file under 1000 lines.
// Imported via `test { _ = @import("document_test.zig"); }` in document.zig.

const std = @import("std");
const testing = std.testing;

const doc = @import("document.zig");
const DocumentEngine = doc.DocumentEngine;
const StoredHeading = doc.StoredHeading;
const StoredLink = doc.StoredLink;
const StoredTag = doc.StoredTag;
const StoredCodeSpan = doc.StoredCodeSpan;
const StoredBlockId = doc.StoredBlockId;
const computeLineStarts = doc.computeLineStarts;
const byteOffsetToPosition = doc.byteOffsetToPosition;
const slugifyText = doc.slugifyText;
const parseAll = doc.parseAll;
const freeHeadings = doc.freeHeadings;
const freeLinks = doc.freeLinks;
const freeCodeSpans = doc.freeCodeSpans;
const freeTags = doc.freeTags;
const freeBlockIds = doc.freeBlockIds;
const freeStoredHeadingsList = doc.freeStoredHeadingsList;
const freeStoredLinksList = doc.freeStoredLinksList;
const blob = @import("blob.zig");

// Import blob tests
test {
    _ = @import("blob.zig");
}

// --- Extraction correctness ---

test "test_create_simple_markdown" {
    const input = "# Hello\n\nSome [link](url.md) text with #tag and ^blockid\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    try testing.expectEqual(@as(usize, 1), engine.links.len);
    try testing.expectEqual(@as(usize, 1), engine.tags.len);
    try testing.expectEqual(@as(usize, 1), engine.block_ids.len);

    try testing.expectEqualStrings("Hello", engine.headings[0].text);
    try testing.expectEqual(@as(u8, 1), engine.headings[0].level);
    try testing.expectEqualStrings("hello", engine.headings[0].slug);
}

test "test_create_multiple_headings" {
    const input = "# H1\n\n## H2\n\n### H3\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 3), engine.headings.len);
    try testing.expectEqual(@as(u8, 1), engine.headings[0].level);
    try testing.expectEqual(@as(u8, 2), engine.headings[1].level);
    try testing.expectEqual(@as(u8, 3), engine.headings[2].level);
}

test "test_entity_decoding" {
    const input = "# Hello &amp; World\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    try testing.expectEqualStrings("Hello & World", engine.headings[0].text);
}

test "test_wiki_links" {
    const input = "See [[Other Page]] and [normal](link.md)\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 2), engine.links.len);
    // Order depends on md4c parse order (wiki link and normal link)
    var wiki_count: usize = 0;
    var normal_count: usize = 0;
    for (engine.links) |l| {
        if (l.is_wiki) wiki_count += 1 else normal_count += 1;
    }
    try testing.expectEqual(@as(usize, 1), wiki_count);
    try testing.expectEqual(@as(usize, 1), normal_count);
}

// --- Slug dedup ---

test "test_slug_dedup" {
    const input = "# Title\n\n# Title\n\n# Title\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 3), engine.headings.len);
    try testing.expectEqualStrings("title", engine.headings[0].slug);
    try testing.expectEqualStrings("title-1", engine.headings[1].slug);
    try testing.expectEqualStrings("title-2", engine.headings[2].slug);
}

// --- Line starts and positions ---

test "test_line_starts" {
    const input = "first\nsecond\nthird\n";
    const starts = try computeLineStarts(testing.allocator, input);
    defer testing.allocator.free(starts);

    // "first\n" = 6 bytes, "second\n" = 7 bytes, "third\n" = 6 bytes
    try testing.expectEqual(@as(usize, 4), starts.len);
    try testing.expectEqual(@as(u32, 0), starts[0]);
    try testing.expectEqual(@as(u32, 6), starts[1]);
    try testing.expectEqual(@as(u32, 13), starts[2]);
    try testing.expectEqual(@as(u32, 19), starts[3]);
}

test "test_byte_offset_to_position" {
    const starts = &[_]u32{ 0, 6, 13, 19 };
    // Offset 0 → line 0, col 0
    const p0 = byteOffsetToPosition(starts, 0);
    try testing.expectEqual(@as(u32, 0), p0.line);
    try testing.expectEqual(@as(u32, 0), p0.col);

    // Offset 6 → line 1, col 0
    const p1 = byteOffsetToPosition(starts, 6);
    try testing.expectEqual(@as(u32, 1), p1.line);
    try testing.expectEqual(@as(u32, 0), p1.col);

    // Offset 8 → line 1, col 2
    const p2 = byteOffsetToPosition(starts, 8);
    try testing.expectEqual(@as(u32, 1), p2.line);
    try testing.expectEqual(@as(u32, 2), p2.col);
}

// --- Blob serialization ---

test "test_blob_header" {
    const input = "# Hello\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = blob.readHeader(blob_data);

    try testing.expectEqual(blob.BLOB_MAGIC, header.magic);
    try testing.expectEqual(blob.BLOB_VERSION, header.version);
    try testing.expectEqual(@as(u32, 1), header.heading_count);
    try testing.expectEqual(@as(u32, 0), header.link_count);
}

test "test_blob_text_pool" {
    const input = "# Hello\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = blob.readHeader(blob_data);
    const offsets = blob.computeSectionOffsets(header).?;

    // Read the heading from the blob
    const bh = try blob.readStruct(blob.BlobHeading, blob_data, offsets.headings);
    try testing.expectEqual(@as(u8, 1), bh.level);

    // Verify text pool contains "Hello"
    const text_start = offsets.text_pool + bh.text_off;
    const text_end = text_start + bh.text_len;
    try testing.expectEqualStrings("Hello", blob_data[text_start..text_end]);

    // Verify text pool contains slug "hello"
    const slug_start = offsets.text_pool + bh.slug_off;
    const slug_end = slug_start + bh.slug_len;
    try testing.expectEqualStrings("hello", blob_data[slug_start..slug_end]);
}

test "test_blob_empty_document" {
    const input = "";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    // Empty document blob is header only (64 bytes)
    try testing.expectEqual(@as(usize, 64), blob_data.len);

    const header = blob.readHeader(blob_data);
    try testing.expectEqual(@as(u32, 0), header.heading_count);
    try testing.expectEqual(@as(u32, 0), header.link_count);
    try testing.expectEqual(@as(u32, 0), header.tag_count);
    try testing.expectEqual(@as(u32, 0), header.block_id_count);
}

test "test_blob_validate_rejects_bad_magic" {
    var buf: [64]u8 = .{0} ** 64;
    std.mem.writeInt(u32, buf[0..4], 0xDEADBEEF, .little);
    try testing.expectError(error.InvalidMagic, blob.validateBlob(&buf));
}

test "test_blob_validates_after_serialize" {
    const input = "# Title\n\n[link](url.md) #tag ^block\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = try blob.validateBlob(blob_data);
    try testing.expectEqual(blob.BLOB_MAGIC, header.magic);
    try testing.expectEqual(blob.BLOB_VERSION, header.version);
}

// --- Update ---

test "test_update_replaces_state" {
    var engine = try DocumentEngine.create("# A\n", testing.allocator);
    defer engine.destroy();

    try testing.expectEqualStrings("A", engine.headings[0].text);
    try engine.update("# B\n");
    try testing.expectEqualStrings("B", engine.headings[0].text);
}

test "test_update_invalidates_blob" {
    var engine = try DocumentEngine.create("# A\n", testing.allocator);
    defer engine.destroy();

    const blob1 = try engine.getBlob();
    const blob1_len = blob1.len;

    try engine.update("# B\n## C\n");
    // Blob should be invalidated
    try testing.expectEqual(@as(?[]u8, null), engine.cached_blob);

    const blob2 = try engine.getBlob();
    // New blob should be different (more headings = larger)
    try testing.expect(blob2.len != blob1_len or blob2.ptr != blob1.ptr);
}

test "test_update_changes_counts" {
    var engine = try DocumentEngine.create("# One\n", testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);

    try engine.update("# One\n## Two\n### Three\n");
    try testing.expectEqual(@as(usize, 3), engine.headings.len);
}

// --- Memory safety ---

test "test_create_destroy_no_leaks" {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer {
        const check = gpa.deinit();
        if (check == .leak) @panic("Memory leak detected in create/destroy");
    }
    const allocator = gpa.allocator();

    var engine = try DocumentEngine.create("# Hello\n\n[link](url) #tag ^id\n", allocator);
    engine.destroy();
}

test "test_update_100_times_no_leaks" {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer {
        const check = gpa.deinit();
        if (check == .leak) @panic("Memory leak detected in update×100");
    }
    const allocator = gpa.allocator();

    var engine = try DocumentEngine.create("# Initial\n", allocator);
    defer engine.destroy();

    var i: u32 = 0;
    while (i < 100) : (i += 1) {
        var buf: [128]u8 = undefined;
        const text = std.fmt.bufPrint(&buf, "# Heading {d}\n\nSome [link](url{d}.md) text #tag{d}\n", .{ i, i, i }) catch continue;
        try engine.update(text);
    }
}

// --- Additional edge cases ---

test "empty heading produces empty slug" {
    const input = "# \n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    // Empty heading text → empty slug
    try testing.expectEqualStrings("", engine.headings[0].slug);
}

test "line_starts for empty input" {
    const starts = try computeLineStarts(testing.allocator, "");
    // Empty text has no line starts
    try testing.expectEqual(@as(usize, 0), starts.len);
}

test "token_estimate is nonzero for nonempty input" {
    var engine = try DocumentEngine.create("hello world foo bar\n", testing.allocator);
    defer engine.destroy();

    try testing.expect(engine.token_estimate > 0);
}

test "content_hash is deterministic" {
    var engine1 = try DocumentEngine.create("# Same\n", testing.allocator);
    defer engine1.destroy();

    var engine2 = try DocumentEngine.create("# Same\n", testing.allocator);
    defer engine2.destroy();

    try testing.expectEqual(engine1.content_hash, engine2.content_hash);
}

test "content_hash differs for different input" {
    var engine1 = try DocumentEngine.create("# A\n", testing.allocator);
    defer engine1.destroy();

    var engine2 = try DocumentEngine.create("# B\n", testing.allocator);
    defer engine2.destroy();

    try testing.expect(engine1.content_hash != engine2.content_hash);
}

test "tags inside code blocks are filtered" {
    const input = "text #visible\n```\n#hidden\n```\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.tags.len);
    try testing.expectEqualStrings("visible", engine.tags[0].name);
}

test "getBlob caches result" {
    var engine = try DocumentEngine.create("# Test\n", testing.allocator);
    defer engine.destroy();

    const blob1 = try engine.getBlob();
    const blob2 = try engine.getBlob();
    // Same pointer — cached
    try testing.expectEqual(blob1.ptr, blob2.ptr);
}

test "blob line_starts roundtrip" {
    const input = "# Line1\nLine2\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = blob.readHeader(blob_data);
    const offsets = blob.computeSectionOffsets(header).?;

    // Read line_starts from blob
    for (0..header.line_count) |i| {
        const offset = offsets.line_starts + @as(u32, @intCast(i)) * @sizeOf(u32);
        const ls = std.mem.readInt(u32, blob_data[offset..][0..4], .little);
        try testing.expectEqual(engine.line_starts[i], ls);
    }
}

test "slugifyText truncated slug returns content not empty string" {
    // When heading text produces >512 slug bytes, slugify() returns -2 (truncated).
    // The output buffer holds 512 valid bytes. Fix: return out[0..512], not "".
    // This test verifies the fix: a 513-char heading gets a 512-byte slug, not "".
    var out: [512]u8 = undefined;
    const long_text = "a" ** 513; // 513 'a' chars → slugify returns -2, buffer has 512 'a' chars
    const slug = slugifyText(long_text, &out);
    try testing.expectEqual(@as(usize, 512), slug.len);
    try testing.expectEqualStrings("a" ** 512, slug);
}

test "freeStoredHeadingsList with free_texts=true frees text strings" {
    // Verify that freeStoredHeadingsList with free_texts=true frees both text and slug.
    // This simulates the errdefer cleanup path after texts_transferred (Bug 1 fix):
    // before the fix, errdefer only freed slugs, leaking h.text owned by stored lists.
    const alloc = testing.allocator;
    var list = std.ArrayListUnmanaged(StoredHeading){};
    const text = try alloc.dupe(u8, "Hello World");
    const slug = try alloc.dupe(u8, "hello-world");
    try list.append(alloc, .{
        .text = text,
        .slug = slug,
        .source_offset = 0,
        .start = .{ .line = 0, .col = 0 },
        .end = .{ .line = 0, .col = 10 },
        .level = 1,
    });
    freeStoredHeadingsList(alloc, &list, true);
    // testing.allocator (GPA) detects leaks: if text or slug aren't freed, test fails
}

test "freeStoredLinksList with free_texts=true frees text and target strings" {
    // Verify that freeStoredLinksList with free_texts=true frees text and target.
    // Before the fix, freeStoredLinksList freed nothing (link texts were always owned
    // by extraction until line 289-290, but errdefer fires after that with no way to
    // distinguish which allocations to free).
    const alloc = testing.allocator;
    var list = std.ArrayListUnmanaged(StoredLink){};
    const text = try alloc.dupe(u8, "Click here");
    const target = try alloc.dupe(u8, "https://example.com");
    try list.append(alloc, .{
        .text = text,
        .target = target,
        .source_offset = 0,
        .start = .{ .line = 0, .col = 0 },
        .end = .{ .line = 0, .col = 10 },
        .is_wiki = false,
    });
    freeStoredLinksList(alloc, &list, true);
    // testing.allocator (GPA) detects leaks: if text or target aren't freed, test fails
}

test "slugifyText truncated heading via DocumentEngine is non-empty" {
    // Integration: DocumentEngine.create with a >512-char heading text should
    // produce a non-empty slug (not "" from silently discarding truncated output).
    const prefix = "# ";
    const heading_text = "b" ** 513;
    const input = prefix ++ heading_text ++ "\n";
    var engine = try DocumentEngine.create(input, testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    // Slug should be 512 'b' chars, not empty
    try testing.expectEqual(@as(usize, 512), engine.headings[0].slug.len);
    try testing.expectEqualStrings("b" ** 512, engine.headings[0].slug);
}

// --- marky-wdnc: defense-in-depth guards ---

test "H2: serializeState returns OutOfMemory for oversized element counts" {
    // Construct a DocumentEngine with a fake oversized headings slice.
    // Uses the many-pointer trick (MEMORY.md): [*] slicing has no bounds check,
    // and the guard fires before any data access so the fake pointer is never
    // dereferenced.
    var engine = try DocumentEngine.create("", testing.allocator);
    defer engine.destroy();

    // Save original empty headings and replace with fake oversized slice
    const original_headings = engine.headings;
    var sentinel: StoredHeading = undefined;
    const p: [*]StoredHeading = @ptrCast(&sentinel);
    const huge_len = @as(usize, std.math.maxInt(u32)) + 1;
    engine.headings = p[0..huge_len];

    // getBlob → serializeState should return OutOfMemory from the guard
    try testing.expectError(error.OutOfMemory, engine.getBlob());

    // Restore original before destroy to avoid freeing fake pointer
    engine.headings = original_headings;
}

test "H4: FENCE_MAP_MAX constant has expected value" {
    try testing.expectEqual(@as(u32, 256), doc.FENCE_MAP_MAX);
}

// --- marky-8nzt: toOwnedSlice cascade leak regression ---

test "marky-8nzt: parseAll toOwnedSlice cascade OOM — no leak" {
    // Exercises every OOM failure point in parseAll by iterating fail_index
    // from 0..N. At each index, exactly one allocation fails.
    // GPA detects leaks (.leak status) — verifies that scoped errdefers after
    // each toOwnedSlice call correctly free transferred data when a later
    // toOwnedSlice fails.
    //
    // Input has headings, links, tags, and block IDs so all four toOwnedSlice
    // paths (lines 359-362) are exercised. The critical path: headings
    // toOwnedSlice succeeds → links toOwnedSlice fails → errdefer must free
    // out_headings (top-level errdefer runs on empty stored_headings_list,
    // which is a no-op after toOwnedSlice consumed it).
    const input = "# Heading One\n\n[Link Text](https://example.com)\n\n#tag1\n\nA paragraph ^block-one\n";

    var fail_index: usize = 0;
    var consecutive_successes: usize = 0;
    while (consecutive_successes < 5) : (fail_index += 1) {
        // Safety valve: prevent infinite loop if something is very wrong
        if (fail_index > 300) break;

        var gpa = std.heap.GeneralPurposeAllocator(.{}){};
        var failing = std.testing.FailingAllocator.init(gpa.allocator(), .{ .fail_index = fail_index });

        var out_headings: []StoredHeading = &.{};
        var out_links: []StoredLink = &.{};
        var out_code_spans: []StoredCodeSpan = &.{};
        var out_tags: []StoredTag = &.{};
        var out_block_ids: []StoredBlockId = &.{};
        var out_line_starts: []u32 = &.{};
        var out_token_estimate: u32 = 0;
        var out_content_hash: u64 = 0;

        const result = parseAll(
            failing.allocator(),
            input,
            &out_headings,
            &out_links,
            &out_code_spans,
            &out_tags,
            &out_block_ids,
            &out_line_starts,
            &out_token_estimate,
            &out_content_hash,
        );

        if (result) |_| {
            // Success: free output slices manually (simulates caller cleanup)
            freeHeadings(failing.allocator(), out_headings);
            freeLinks(failing.allocator(), out_links);
            freeCodeSpans(failing.allocator(), out_code_spans);
            freeTags(failing.allocator(), out_tags);
            freeBlockIds(failing.allocator(), out_block_ids);
            if (out_line_starts.len > 0) failing.allocator().free(out_line_starts);
            consecutive_successes += 1;
        } else |_| {
            consecutive_successes = 0;
        }

        const check = gpa.deinit();
        try testing.expect(check == .ok);
    }

    // Verify we actually tested multiple failure points (not just index 0)
    try testing.expect(fail_index > 5);
}

// ── Code span tests ───────────────────────────────────────────────

test "engine extracts code spans from backtick text" {
    const engine = try DocumentEngine.create("Hello `world` end", testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.code_spans.len);
    try testing.expectEqualStrings("world", engine.code_spans[0].text);
    try testing.expectEqual(@as(u32, 6), engine.code_spans[0].source_offset);
    // end_offset past closing backtick: 6 + 1 + 5 + 1 = 13
    try testing.expectEqual(@as(u32, 13), engine.code_spans[0].end_offset);
}

test "engine extracts multiple code spans" {
    const engine = try DocumentEngine.create("`a` and `b`", testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 2), engine.code_spans.len);
    try testing.expectEqualStrings("a", engine.code_spans[0].text);
    try testing.expectEqualStrings("b", engine.code_spans[1].text);
    // Second code span offset must be greater than first
    try testing.expect(engine.code_spans[1].source_offset > engine.code_spans[0].source_offset);
}

test "engine no code spans in plain text" {
    const engine = try DocumentEngine.create("No code here", testing.allocator);
    defer engine.destroy();
    try testing.expectEqual(@as(usize, 0), engine.code_spans.len);
}

test "engine code span inside heading" {
    const engine = try DocumentEngine.create("# Title `code` end", testing.allocator);
    defer engine.destroy();

    // Both heading and code span should be present
    try testing.expectEqual(@as(usize, 1), engine.headings.len);
    try testing.expectEqual(@as(usize, 1), engine.code_spans.len);
    try testing.expectEqualStrings("code", engine.code_spans[0].text);
}

test "engine code span positions are correct" {
    const engine = try DocumentEngine.create("line1\n`code`\nline3", testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.code_spans.len);
    // Code span is on line 1 (0-indexed), col 0
    try testing.expectEqual(@as(u32, 1), engine.code_spans[0].start.line);
    try testing.expectEqual(@as(u32, 0), engine.code_spans[0].start.col);
}

test "engine code span blob roundtrip" {
    const engine = try DocumentEngine.create("Hello `world` end", testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    // Validate the blob has code_span_count in header
    const header = blob.readHeader(blob_data);
    try testing.expectEqual(@as(u32, 1), header.code_span_count);
    try testing.expectEqual(@as(u32, 0), header.heading_count);

    // Verify blob validates successfully
    const validated = try blob.validateBlob(blob_data);
    try testing.expectEqual(@as(u32, 1), validated.code_span_count);
}

test "engine code span blob roundtrip empty" {
    const engine = try DocumentEngine.create("No code here", testing.allocator);
    defer engine.destroy();

    const blob_data = try engine.getBlob();
    const header = blob.readHeader(blob_data);
    try testing.expectEqual(@as(u32, 0), header.code_span_count);
}

test "engine update preserves code spans" {
    const engine = try DocumentEngine.create("`a`", testing.allocator);
    defer engine.destroy();

    try testing.expectEqual(@as(usize, 1), engine.code_spans.len);
    try testing.expectEqualStrings("a", engine.code_spans[0].text);

    // Update with new content
    try engine.update("`b` and `c`");
    try testing.expectEqual(@as(usize, 2), engine.code_spans.len);
    try testing.expectEqualStrings("b", engine.code_spans[0].text);
    try testing.expectEqualStrings("c", engine.code_spans[1].text);
}
