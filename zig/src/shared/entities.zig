const std = @import("std");
const ref = @import("../reference/entities_ref.zig");

/// SIMD-accelerated entity hash extraction.
///
/// Scans text for word boundaries using @Vector(16, u8) to detect separators,
/// then hashes each word with FNV-1a (lower 32 bits).
///
/// Returns:
///   0  — success (zero length is a no-op returning 0)
///  -2  — buffer too small (writes as many as fit)
pub fn extract_entity_hashes(
    text: [*]const u8,
    text_len: u32,
    output_ids: [*]u32,
    capacity: u32,
    written: *u32,
) i32 {
    if (text_len == 0) {
        written.* = 0;
        return 0;
    }

    const buf = text[0..text_len];
    var count: u32 = 0;
    var word_start: u32 = 0;
    var in_word: bool = false;

    // SIMD scan: detect space/tab/newline/cr boundaries in 16-byte chunks
    // For punctuation-level splitting, we fall back to scalar within "word" regions
    const chunk_size: u32 = 16;
    const space_vec: @Vector(16, u8) = @splat(' ');
    const newline_vec: @Vector(16, u8) = @splat('\n');
    const tab_vec: @Vector(16, u8) = @splat('\t');
    const cr_vec: @Vector(16, u8) = @splat('\r');

    var pos: u32 = 0;
    const simd_end = if (text_len >= chunk_size) text_len - chunk_size + 1 else 0;

    while (pos < simd_end) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;

        // Check for whitespace boundaries (most common separators)
        const is_space = chunk == space_vec;
        const is_newline = chunk == newline_vec;
        const is_tab = chunk == tab_vec;
        const is_cr = chunk == cr_vec;

        const is_ws = @select(bool, is_space, is_space, @select(bool, is_newline, is_newline, @select(bool, is_tab, is_tab, is_cr)));

        // Process each lane
        inline for (0..chunk_size) |lane| {
            const i = pos + @as(u32, @intCast(lane));
            const c = buf[i];

            if (is_ws[lane] or is_punct(c)) {
                if (in_word) {
                    if (count < capacity) {
                        const word = buf[word_start..i];
                        output_ids[count] = @truncate(ref.fnv1a_hash(word));
                    }
                    count += 1;
                    in_word = false;
                }
            } else {
                if (!in_word) {
                    word_start = i;
                    in_word = true;
                }
            }
        }
    }

    // Scalar tail
    while (pos < text_len) : (pos += 1) {
        const c = buf[pos];
        if (is_separator(c)) {
            if (in_word) {
                if (count < capacity) {
                    const word = buf[word_start..pos];
                    output_ids[count] = @truncate(ref.fnv1a_hash(word));
                }
                count += 1;
                in_word = false;
            }
        } else {
            if (!in_word) {
                word_start = pos;
                in_word = true;
            }
        }
    }

    // Final word
    if (in_word) {
        if (count < capacity) {
            const word = buf[word_start..text_len];
            output_ids[count] = @truncate(ref.fnv1a_hash(word));
        }
        count += 1;
    }

    const actual_written = @min(count, capacity);
    written.* = actual_written;

    if (count > capacity) return -2;
    return 0;
}

fn is_punct(c: u8) bool {
    return switch (c) {
        ',', '.', ';', ':', '!', '?', '(', ')', '[', ']', '{', '}', '"', '\'', '/', '\\', '|', '-', '_', '+', '=', '<', '>', '~', '`', '@', '#', '$', '%', '^', '&', '*' => true,
        else => false,
    };
}

fn is_separator(c: u8) bool {
    return switch (c) {
        ' ', '\t', '\n', '\r' => true,
        else => is_punct(c),
    };
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_entity_hash_determinism" {
    const text = "hello world foo bar baz";
    var out1: [16]u32 = undefined;
    var out2: [16]u32 = undefined;
    var w1: u32 = undefined;
    var w2: u32 = undefined;
    _ = extract_entity_hashes(text.ptr, text.len, &out1, 16, &w1);
    _ = extract_entity_hashes(text.ptr, text.len, &out2, 16, &w2);
    try testing.expectEqual(w1, w2);
    for (0..w1) |i| {
        try testing.expectEqual(out1[i], out2[i]);
    }
}

test "test_entity_hash_known_values" {
    const text = "hello";
    var out: [1]u32 = undefined;
    var w: u32 = undefined;
    _ = extract_entity_hashes(text.ptr, text.len, &out, 1, &w);
    try testing.expectEqual(@as(u32, 1), w);
    const expected = ref.fnv1a_hash("hello");
    try testing.expectEqual(@as(u32, @truncate(expected)), out[0]);
}

test "test_entity_hash_empty" {
    const text = "";
    var out: [4]u32 = undefined;
    var w: u32 = undefined;
    const rc = extract_entity_hashes(text.ptr, 0, &out, 4, &w);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), w);
}

test "test_entity_hash_buffer_overflow" {
    const text = "one two three four five";
    var out: [2]u32 = undefined;
    var w: u32 = undefined;
    const rc = extract_entity_hashes(text.ptr, text.len, &out, 2, &w);
    try testing.expectEqual(@as(i32, -2), rc);
    try testing.expectEqual(@as(u32, 2), w);
}

test "test_entity_hash_punctuation_splits" {
    const text = "hello,world.foo";
    var out: [8]u32 = undefined;
    var w: u32 = undefined;
    const rc = extract_entity_hashes(text.ptr, text.len, &out, 8, &w);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 3), w);
}

test "test_entity_simd_scalar_parity" {
    const text = "The quick brown fox jumps over the lazy dog and then some more words for good measure";
    var simd_out: [32]u32 = undefined;
    var scalar_out: [32]u32 = undefined;
    var simd_w: u32 = undefined;
    var scalar_w: u32 = undefined;
    _ = extract_entity_hashes(text.ptr, text.len, &simd_out, 32, &simd_w);
    _ = ref.extract_entity_hashes(text.ptr, text.len, &scalar_out, 32, &scalar_w);
    try testing.expectEqual(scalar_w, simd_w);
    for (0..simd_w) |i| {
        try testing.expectEqual(scalar_out[i], simd_out[i]);
    }
}

test "test_entity_hash_large_text" {
    const line = "Entity extraction with SIMD acceleration handles large inputs efficiently. ";
    comptime var text: []const u8 = "";
    comptime {
        var i = 0;
        while (i < 50) : (i += 1) {
            text = text ++ line;
        }
    }
    var out: [1024]u32 = undefined;
    var w: u32 = undefined;
    const rc = extract_entity_hashes(text.ptr, @intCast(text.len), &out, 1024, &w);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expect(w > 0);
}
