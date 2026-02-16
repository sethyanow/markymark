const std = @import("std");

/// FNV-1a 64-bit offset basis
const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
/// FNV-1a 64-bit prime
const FNV_PRIME: u64 = 1099511628211;

/// FNV-1a 64-bit content hash.
///
/// Computes a deterministic fingerprint of the given text content.
/// Uses the FNV-1a algorithm which XORs each byte then multiplies by the prime,
/// providing good distribution and avalanche properties.
///
/// Returns FNV_OFFSET_BASIS for empty input (hash of empty string).
pub fn content_hash(text: [*]const u8, len: u32) u64 {
    if (len == 0) return FNV_OFFSET_BASIS;

    const buf = text[0..len];
    var hash: u64 = FNV_OFFSET_BASIS;

    for (buf) |byte| {
        hash ^= @intCast(byte);
        hash *%= FNV_PRIME;
    }

    return hash;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_content_hash_empty" {
    const text = "";
    const result = content_hash(text.ptr, 0);
    try testing.expectEqual(FNV_OFFSET_BASIS, result);
}

test "test_content_hash_deterministic" {
    const text = "Hello, world!";
    const hash1 = content_hash(text.ptr, text.len);
    const hash2 = content_hash(text.ptr, text.len);
    try testing.expectEqual(hash1, hash2);
}

test "test_content_hash_known_vectors" {
    // FNV-1a test vectors from the spec (fnvhash.com)
    // FNV-1a 64-bit of "" = 0xcbf29ce484222325 (offset basis)
    const empty_hash = content_hash("".ptr, 0);
    try testing.expectEqual(@as(u64, 0xcbf29ce484222325), empty_hash);

    // FNV-1a 64-bit of "a" = 0xaf63dc4c8601ec8c
    const a_hash = content_hash("a".ptr, 1);
    try testing.expectEqual(@as(u64, 0xaf63dc4c8601ec8c), a_hash);

    // FNV-1a 64-bit of "foobar" = 0x85944171f73967e8
    const foobar_hash = content_hash("foobar".ptr, 6);
    try testing.expectEqual(@as(u64, 0x85944171f73967e8), foobar_hash);
}

test "test_content_hash_distinct_inputs" {
    // Different inputs must produce different hashes
    const hash1 = content_hash("hello".ptr, 5);
    const hash2 = content_hash("world".ptr, 5);
    const hash3 = content_hash("Hello".ptr, 5); // case differs
    const hash4 = content_hash("hello ".ptr, 6); // trailing space

    try testing.expect(hash1 != hash2);
    try testing.expect(hash1 != hash3);
    try testing.expect(hash1 != hash4);
    try testing.expect(hash2 != hash3);
}

test "test_content_hash_avalanche" {
    // Different content should produce well-distributed hashes.
    // Test with several pairs and check that XOR has multiple bits set.
    const pairs = [_][2][]const u8{
        .{ "hello world", "hello worle" },
        .{ "abc123", "abc124" },
        .{ "the quick brown fox", "the quick brown fog" },
    };
    for (pairs) |pair| {
        const hash1 = content_hash(pair[0].ptr, @intCast(pair[0].len));
        const hash2 = content_hash(pair[1].ptr, @intCast(pair[1].len));
        const diff = hash1 ^ hash2;
        const bit_count = @popCount(diff);
        // FNV-1a: even a single byte change should differ in at least a few bits
        try testing.expect(bit_count >= 4);
    }
}

test "test_content_hash_large_input" {
    // ~10KB input should work without issues
    const line = "This is a regular line of text for testing content hash functions.\n";
    comptime var text: []const u8 = "";
    comptime {
        var i = 0;
        while (i < 150) : (i += 1) {
            text = text ++ line;
        }
    }
    const result = content_hash(text.ptr, @intCast(text.len));
    try testing.expect(result != FNV_OFFSET_BASIS); // Not empty hash
    try testing.expect(result != 0); // Not zero

    // Deterministic
    const result2 = content_hash(text.ptr, @intCast(text.len));
    try testing.expectEqual(result, result2);
}

test "test_content_hash_single_byte_difference" {
    // Two strings differing by one byte should have different hashes
    const text1 = "AAAAAAAAAAAAAAAA"; // 16 bytes
    const text2 = "AAAAAAAAAAAAAAAB"; // 16 bytes, last byte differs
    const hash1 = content_hash(text1.ptr, text1.len);
    const hash2 = content_hash(text2.ptr, text2.len);
    try testing.expect(hash1 != hash2);
}
