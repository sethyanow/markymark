// FFI integration tests for md4c extraction exports.
// Tests the C ABI contract: marky_md4c_extract + marky_md4c_free.

const std = @import("std");
const testing = std.testing;

const exports = @import("exports.zig");
const CMd4cResult = exports.CMd4cResult;
const marky_md4c_extract = exports.marky_md4c_extract;
const marky_md4c_free = exports.marky_md4c_free;

test "md4c_extract: simple heading" {
    const input = "# Hello\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.headings_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const h = result.headings.?[0];
    try testing.expectEqualStrings("Hello", blob[h.text_offset..h.text_offset + h.text_length]);
    try testing.expectEqual(@as(u8, 1), h.level);
}

test "md4c_extract: inline link with text and target" {
    const input = "[click](https://example.com)\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const l = result.links.?[0];
    try testing.expectEqualStrings("click", blob[l.text_offset..l.text_offset + l.text_length]);
    try testing.expectEqualStrings("https://example.com", blob[l.target_offset..l.target_offset + l.target_length]);
    try testing.expectEqual(@as(u8, 0), l.is_wiki);
}

test "md4c_extract: null text pointer returns -1" {
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(null, 10, &result);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "md4c_extract: null out pointer returns -1" {
    const input = "# Hello\n";
    const rc = marky_md4c_extract(input.ptr, input.len, null);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "md4c_extract: empty input returns zero results" {
    const input = "";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, 0, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.headings_count);
    try testing.expectEqual(@as(u32, 0), result.links_count);
}

test "md4c_extract: wiki link" {
    const input = "[[Target]]\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    try testing.expectEqual(@as(u8, 1), result.links.?[0].is_wiki);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const l = result.links.?[0];
    try testing.expectEqualStrings("Target", blob[l.target_offset..l.target_offset + l.target_length]);
}

test "md4c_extract: double free is no-op" {
    const input = "# Test\n";
    var result: CMd4cResult = undefined;
    _ = marky_md4c_extract(input.ptr, input.len, &result);
    marky_md4c_free(&result);
    // Second free should be no-op (result zeroed by first free)
    marky_md4c_free(&result);
}

test "md4c_extract: entity text decoded in heading" {
    // Entity references are decoded to UTF-8 by ExtractionRenderer (marky-yfh7)
    const input = "# Hello &amp; World\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const h = result.headings.?[0];
    try testing.expectEqualStrings("Hello & World", blob[h.text_offset..h.text_offset + h.text_length]);
}

test "md4c_extract: mixed document headings and links" {
    const input = "# Title\n\nSome [link](url) text.\n\n## Section\n\nSee [[Wiki]] for details.\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 2), result.headings_count);
    try testing.expectEqual(@as(u32, 2), result.links_count);

    const blob = result.text_blob.?[0..result.text_blob_len];
    const h0 = result.headings.?[0];
    const h1 = result.headings.?[1];
    try testing.expectEqualStrings("Title", blob[h0.text_offset..h0.text_offset + h0.text_length]);
    try testing.expectEqualStrings("Section", blob[h1.text_offset..h1.text_offset + h1.text_length]);

    // Second link should be wiki
    try testing.expectEqual(@as(u8, 1), result.links.?[1].is_wiki);
}

test "md4c_extract: null text with zero len returns -1" {
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(null, 0, &result);
    try testing.expectEqual(@as(i32, -1), rc);
}

// --- Code span FFI tests (marky-pdyo) ---

test "md4c_extract: code span text via blob" {
    const input = "here is `hello` world\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.code_spans_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const cs = result.code_spans.?[0];
    try testing.expectEqualStrings("hello", blob[cs.text_offset..cs.text_offset + cs.text_length]);
    try testing.expectEqual(@as(u32, 8), cs.source_offset);
    try testing.expectEqual(@as(u32, 15), cs.end_offset);
}

test "md4c_extract: mixed document with code spans" {
    const input = "# Title `code` [link](url)\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.headings_count);
    try testing.expectEqual(@as(u32, 1), result.links_count);
    try testing.expectEqual(@as(u32, 1), result.code_spans_count);
    // All blob offsets should be valid
    const blob = result.text_blob.?[0..result.text_blob_len];
    const cs = result.code_spans.?[0];
    try testing.expectEqualStrings("code", blob[cs.text_offset..cs.text_offset + cs.text_length]);
}

test "md4c_extract: no code spans" {
    const input = "Just plain text with no backticks.\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.code_spans_count);
}

// --- Task/Embed FFI tests (marky-rd7r) ---

test "md4c_extract: task via blob" {
    const input = "- [x] Done\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.tasks_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const tk = result.tasks.?[0];
    try testing.expectEqualStrings("Done", blob[tk.text_offset..tk.text_offset + tk.text_length]);
    try testing.expectEqual(@as(u8, 'x'), tk.state);
}

test "md4c_extract: embed via blob" {
    const input = "![[target]]\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.embeds_count);
    const blob = result.text_blob.?[0..result.text_blob_len];
    const e = result.embeds.?[0];
    try testing.expectEqualStrings("target", blob[e.target_offset..e.target_offset + e.target_length]);
    // Also has a wikilink
    try testing.expectEqual(@as(u32, 1), result.links_count);
}

test "md4c_extract: no tasks or embeds" {
    const input = "Just plain text.\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.tasks_count);
    try testing.expectEqual(@as(u32, 0), result.embeds_count);
}

// --- Callout/BlockRef FFI tests (marky-1r0t) ---

test "md4c_extract: callout via blob" {
    const input = "> [!note]\n> Some content\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.callouts_count);
    const blob_slice = result.text_blob.?[0..result.text_blob_len];
    const cl = result.callouts.?[0];
    try testing.expectEqualStrings("note", blob_slice[cl.type_offset..cl.type_offset + cl.type_length]);
    try testing.expectEqual(@as(u32, 0), cl.title_length); // no title
}

test "md4c_extract: callout with title via blob" {
    const input = "> [!tip] My Title\n> Content\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.callouts_count);
    const blob_slice = result.text_blob.?[0..result.text_blob_len];
    const cl = result.callouts.?[0];
    try testing.expectEqualStrings("tip", blob_slice[cl.type_offset..cl.type_offset + cl.type_length]);
    try testing.expectEqualStrings("My Title", blob_slice[cl.title_offset..cl.title_offset + cl.title_length]);
}

test "md4c_extract: block ref via blob" {
    const input = "Text ((a1b2c3d4-e5f6-7890-abcd-ef1234567890)) more\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 1), result.block_refs_count);
    const blob_slice = result.text_blob.?[0..result.text_blob_len];
    const br = result.block_refs.?[0];
    try testing.expectEqualStrings("a1b2c3d4-e5f6-7890-abcd-ef1234567890", blob_slice[br.uuid_offset..br.uuid_offset + br.uuid_length]);
}

test "md4c_extract: no callouts or block refs" {
    const input = "Just plain text.\n";
    var result: CMd4cResult = undefined;
    const rc = marky_md4c_extract(input.ptr, input.len, &result);
    defer marky_md4c_free(&result);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), result.callouts_count);
    try testing.expectEqual(@as(u32, 0), result.block_refs_count);
}
