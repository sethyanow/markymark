const std = @import("std");

/// FNV-1a 64-bit offset basis
pub const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
/// FNV-1a 64-bit prime
pub const FNV_PRIME: u64 = 1099511628211;

/// FNV-1a 64-bit hash of a byte slice.
pub fn fnv1a_hash(data: []const u8) u64 {
    var hash: u64 = FNV_OFFSET_BASIS;
    for (data) |byte| {
        hash ^= @intCast(byte);
        hash *%= FNV_PRIME;
    }
    return hash;
}

/// Scalar entity hash extraction.
///
/// Scans text for word boundaries and produces a FNV-1a hash for each word.
/// Words are separated by whitespace or punctuation.
/// Output hashes are written as u32 (lower 32 bits of FNV-1a 64-bit).
///
/// Returns:
///   0  — success (zero length is a no-op returning 0)
///  -2  — buffer too small (more entities than capacity; writes as many as fit)
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

    for (buf, 0..) |c, idx| {
        const i: u32 = @intCast(idx);
        if (is_separator(c)) {
            if (in_word) {
                if (count < capacity) {
                    const word = buf[word_start..i];
                    const hash = fnv1a_hash(word);
                    output_ids[count] = @truncate(hash);
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

    // Final word
    if (in_word) {
        if (count < capacity) {
            const word = buf[word_start..text_len];
            const hash = fnv1a_hash(word);
            output_ids[count] = @truncate(hash);
        }
        count += 1;
    }

    const actual_written = @min(count, capacity);
    written.* = actual_written;

    if (count > capacity) return -2;
    return 0;
}

fn is_separator(c: u8) bool {
    return switch (c) {
        ' ', '\t', '\n', '\r', ',', '.', ';', ':', '!', '?', '(', ')', '[', ']', '{', '}', '"', '\'', '/', '\\', '|', '-', '_', '+', '=', '<', '>', '~', '`', '@', '#', '$', '%', '^', '&', '*' => true,
        else => false,
    };
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "ref_entity_hash_determinism" {
    const text = "hello world";
    var out1: [8]u32 = undefined;
    var out2: [8]u32 = undefined;
    var w1: u32 = undefined;
    var w2: u32 = undefined;
    _ = extract_entity_hashes(text.ptr, text.len, &out1, 8, &w1);
    _ = extract_entity_hashes(text.ptr, text.len, &out2, 8, &w2);
    try testing.expectEqual(w1, w2);
    for (0..w1) |i| {
        try testing.expectEqual(out1[i], out2[i]);
    }
}

test "ref_entity_hash_known_values" {
    // "hello" -> FNV-1a 64-bit, truncated to u32
    const text = "hello";
    var out: [1]u32 = undefined;
    var w: u32 = undefined;
    _ = extract_entity_hashes(text.ptr, text.len, &out, 1, &w);
    try testing.expectEqual(@as(u32, 1), w);
    // Verify against direct FNV-1a computation
    const expected = fnv1a_hash("hello");
    try testing.expectEqual(@as(u32, @truncate(expected)), out[0]);
}

test "ref_entity_hash_empty" {
    const text = "";
    var out: [4]u32 = undefined;
    var w: u32 = undefined;
    const rc = extract_entity_hashes(text.ptr, 0, &out, 4, &w);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), w);
}

test "ref_entity_hash_buffer_overflow" {
    const text = "one two three four five";
    var out: [2]u32 = undefined;
    var w: u32 = undefined;
    const rc = extract_entity_hashes(text.ptr, text.len, &out, 2, &w);
    try testing.expectEqual(@as(i32, -2), rc);
    try testing.expectEqual(@as(u32, 2), w); // writes as many as fit
}

test "ref_entity_hash_punctuation_splits" {
    const text = "hello,world.foo";
    var out: [8]u32 = undefined;
    var w: u32 = undefined;
    const rc = extract_entity_hashes(text.ptr, text.len, &out, 8, &w);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 3), w); // hello, world, foo
}
