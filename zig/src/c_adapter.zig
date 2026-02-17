const std = @import("std");
const heading_scan = @import("kernels/heading_scan.zig");
const link_scan = @import("kernels/link_scan.zig");
const tag_scan = @import("kernels/tag_scan.zig");
const block_scan = @import("kernels/block_scan.zig");
const token_estimate = @import("kernels/token_estimate.zig");
const content_hash_mod = @import("kernels/content_hash.zig");
const fence_map = @import("kernels/fence_map.zig");
const similarity = @import("shared/similarity.zig");
const normalize = @import("shared/normalize.zig");
const entities = @import("shared/entities.zig");
const quantize_mod = @import("shared/quantize.zig");
const embeddings_mod = @import("shared/embeddings.zig");

// Pull in C ABI exports from dedicated export files.
// The _ = @import forces Zig to include these export fn declarations in the library.
comptime {
    _ = @import("exports_embed.zig");
}

/// Re-export types for C consumers
pub const HeadingScan = heading_scan.HeadingScan;
pub const LinkScan = link_scan.LinkScan;
pub const TagScan = tag_scan.TagScan;
pub const BlockIdScan = block_scan.BlockIdScan;
pub const FenceRange = fence_map.FenceRange;

/// Version constant for markymark kernels
/// Format: 0xMMmmpp (major, minor, patch)
const MARKY_VERSION: u32 = 0x000100; // 0.1.0

/// Returns the version of libmarky_kernels.
/// This is a no-op export to verify build system linkage.
/// `export fn` gets C calling convention by default in Zig 0.15.x.
export fn marky_version() u32 {
    return MARKY_VERSION;
}

/// SIMD-accelerated heading extraction.
///
/// Scans `text[0..len]` for ATX headings (# at line start followed by space).
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more headings than cap)
export fn marky_scan_headings(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]HeadingScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = heading_scan.scan_headings(t, len, o, cap);
    w.* = count;

    // If we filled the buffer exactly, there may be more headings
    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated link extraction.
///
/// Scans `text[0..len]` for markdown links [text](url) and wiki-links [[target]].
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more links than cap)
export fn marky_scan_links(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]LinkScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = link_scan.scan_links(t, len, o, cap);
    w.* = count;

    // If we filled the buffer exactly, there may be more links
    if (count >= cap) return -2;

    return 0;
}

/// Approximate BPE token count via SIMD word boundary detection.
///
/// Returns approximate token count for the given text.
/// Returns 0 for null text pointer or zero length.
export fn marky_estimate_tokens(
    text: ?[*]const u8,
    len: u32,
) u32 {
    const t = text orelse return 0;
    if (len == 0) return 0;
    return token_estimate.estimate_tokens(t, len);
}

/// FNV-1a 64-bit content fingerprint.
///
/// Returns a deterministic hash of the text content.
/// Returns 0 for null text pointer. Returns FNV offset basis for zero length.
export fn marky_content_hash(
    text: ?[*]const u8,
    len: u32,
) u64 {
    const t = text orelse return 0;
    return content_hash_mod.content_hash(t, len);
}

/// SIMD-accelerated tag extraction.
///
/// Scans `text[0..len]` for #tag patterns (whitespace-bounded).
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more tags than cap)
export fn marky_scan_tags(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]TagScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = tag_scan.scan_tags(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated block ID extraction.
///
/// Scans `text[0..len]` for ^block-id patterns at end of line.
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more block IDs than cap)
export fn marky_scan_block_ids(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]BlockIdScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = block_scan.scan_block_ids(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

/// SIMD-accelerated fence map builder.
///
/// Scans `text[0..len]` for fenced code blocks (triple+ backtick/tilde at
/// column 0). Writes byte ranges into `ranges_out[0..cap]`, sets `*written`
/// to the number of ranges found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more ranges than cap)
export fn marky_build_fence_map(
    text: ?[*]const u8,
    len: u32,
    ranges_out: ?[*]FenceRange,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = ranges_out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) {
        w.* = 0;
        return -2;
    }

    const count = fence_map.build_fence_map(t, len, o, cap);
    w.* = count;

    if (count >= cap) return -2;

    return 0;
}

// ============================================================================
// Shared kernel exports: similarity, normalize, entities, quantize
// ============================================================================

/// SIMD-accelerated cosine similarity between two f32 vectors.
///
/// Returns cosine similarity in [-1.0, 1.0].
/// Returns -2.0 on error (null pointers, zero dims, zero-magnitude vector).
export fn zig_cosine_similarity(
    a: ?[*]const f32,
    b: ?[*]const f32,
    dims: u32,
) f32 {
    const va = a orelse return -2.0;
    const vb = b orelse return -2.0;
    if (dims == 0) return -2.0;
    return similarity.cosine_similarity(va, vb, dims);
}

/// Jaccard similarity between two sorted u32 hash sets.
///
/// Both sets MUST be sorted in ascending order.
/// Returns |intersection| / |union| in [0.0, 1.0].
/// Returns -1.0 on error (null pointers).
export fn zig_jaccard_similarity(
    set1: ?[*]const u32,
    set1_len: u32,
    set2: ?[*]const u32,
    set2_len: u32,
) f32 {
    const s1 = set1 orelse return -1.0;
    const s2 = set2 orelse return -1.0;
    return similarity.jaccard_similarity(s1, set1_len, s2, set2_len);
}

/// SIMD-accelerated entity hash extraction.
///
/// Scans text for words, produces FNV-1a u32 hash for each.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (writes as many as fit)
export fn zig_extract_entity_hashes(
    text_ptr: ?[*]const u8,
    text_len: u32,
    output_ids: ?[*]u32,
    capacity: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;

    // Zero-length text is a no-op regardless of other params
    if (text_len == 0) {
        w.* = 0;
        return 0;
    }

    const t = text_ptr orelse return -1;

    if (capacity == 0) {
        w.* = 0;
        return -2;
    }

    const o = output_ids orelse return -1;

    return entities.extract_entity_hashes(t, text_len, o, capacity, w);
}

/// SIMD-accelerated L2 normalization of f32 vector.
///
/// Produces a unit vector (||output|| == 1.0).
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer, zero length, zero vector)
export fn zig_normalize_f32_l2(
    input: ?[*]const f32,
    output: ?[*]f32,
    n: u32,
) i32 {
    const i = input orelse return -1;
    const o = output orelse return -1;
    if (n == 0) return -1;
    return normalize.normalize_f32_l2(i, o, n);
}

/// SIMD-accelerated Q4_0 quantization: f32 -> 4-bit packed format.
///
/// n must be divisible by 32 (Q4 block size).
///
/// Returns:
///   0  — success
///  -1  — invalid input (n not divisible by 32, zero, null pointer)
export fn zig_quantize_f32_to_q4_0(
    input: ?[*]const f32,
    output: ?[*]u8,
    n: u32,
) i32 {
    const i = input orelse return -1;
    const o = output orelse return -1;
    if (n == 0) return -1;
    return quantize_mod.quantize_f32_to_q4_0(i, o, n);
}

/// SIMD-accelerated Q4_0 dequantization: 4-bit packed format -> f32.
///
/// n must be divisible by 32 (Q4 block size).
///
/// Returns:
///   0  — success
///  -1  — invalid input
export fn zig_dequantize_q4_0_to_f32(
    input: ?[*]const u8,
    output: ?[*]f32,
    n: u32,
) i32 {
    const i = input orelse return -1;
    const o = output orelse return -1;
    if (n == 0) return -1;
    return quantize_mod.dequantize_q4_0_to_f32(i, o, n);
}

// ============================================================================
// Tests
// ============================================================================

// Pull in kernel tests so they run as part of `zig build test`
test {
    _ = @import("kernels/heading_scan.zig");
    _ = @import("reference/heading_scan_ref.zig");
    _ = @import("kernels/link_scan.zig");
    _ = @import("reference/link_scan_ref.zig");
    _ = @import("kernels/tag_scan.zig");
    _ = @import("reference/tag_scan_ref.zig");
    _ = @import("kernels/block_scan.zig");
    _ = @import("reference/block_scan_ref.zig");
    _ = @import("kernels/token_estimate.zig");
    _ = @import("kernels/content_hash.zig");
    _ = @import("kernels/fence_map.zig");
    _ = @import("reference/fence_map_ref.zig");
    // Multi-scan automaton (Aho-Corasick)
    _ = @import("reference/multi_scan_ref.zig");
    // Shared kernels (forked from forge BRZA)
    _ = @import("shared/similarity.zig");
    _ = @import("reference/similarity_ref.zig");
    _ = @import("shared/normalize.zig");
    _ = @import("reference/normalize_ref.zig");
    _ = @import("shared/entities.zig");
    _ = @import("reference/entities_ref.zig");
    _ = @import("shared/quantize.zig");
    _ = @import("reference/quantize_ref.zig");
    // Embedding index (persistent data structure with lifecycle)
    _ = @import("shared/embeddings.zig");
    // Embedding C ABI exports + tests
    _ = @import("exports_embed.zig");
}

test "marky_version returns expected version" {
    const version = marky_version();
    try std.testing.expectEqual(@as(u32, 0x000100), version);
}

test "version format is correct" {
    const version = marky_version();
    const major: u8 = @truncate(version >> 16);
    const minor: u8 = @truncate(version >> 8);
    const patch: u8 = @truncate(version);

    try std.testing.expectEqual(@as(u8, 0), major);
    try std.testing.expectEqual(@as(u8, 1), minor);
    try std.testing.expectEqual(@as(u8, 0), patch);
}

test "marky_scan_headings basic" {
    const text = "# Hello\n## World\n";
    var out: [8]HeadingScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "marky_scan_headings null text with zero len" {
    var w: u32 = undefined;
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_headings null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_headings null written" {
    const text = "# Hello\n";
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_headings zero cap" {
    const text = "# Hello\n";
    var out: [4]HeadingScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

// -- marky_scan_links tests --

test "marky_scan_links basic" {
    const text = "[hello](url) and [[wiki]]";
    var out: [8]LinkScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_links(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u8, 0), out[0].link_type); // markdown
    try std.testing.expectEqual(@as(u8, 1), out[1].link_type); // wiki
}

test "marky_scan_links null text with zero len" {
    var w: u32 = undefined;
    var out: [4]LinkScan = undefined;
    const rc = marky_scan_links(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_links null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]LinkScan = undefined;
    const rc = marky_scan_links(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_links null written" {
    const text = "[hello](url)";
    var out: [4]LinkScan = undefined;
    const rc = marky_scan_links(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_links zero cap" {
    const text = "[hello](url)";
    var out: [4]LinkScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_links(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_links buffer overflow returns -2" {
    const text = "[a](b) [c](d) [e](f)";
    var out: [1]LinkScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_links(text.ptr, text.len, &out, 1, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
}

// -- marky_estimate_tokens tests --

test "marky_estimate_tokens basic" {
    const text = "hello world foo bar";
    const result = marky_estimate_tokens(text.ptr, text.len);
    // 4 words * 1.3 = 5.2 -> (4*13+5)/10 = 57/10 = 5
    try std.testing.expectEqual(@as(u32, 5), result);
}

test "marky_estimate_tokens null text" {
    const result = marky_estimate_tokens(null, 10);
    try std.testing.expectEqual(@as(u32, 0), result);
}

test "marky_estimate_tokens zero length" {
    const text = "hello";
    const result = marky_estimate_tokens(text.ptr, 0);
    try std.testing.expectEqual(@as(u32, 0), result);
}

// -- marky_content_hash tests --

test "marky_content_hash basic" {
    const text = "hello";
    const hash = marky_content_hash(text.ptr, text.len);
    try std.testing.expect(hash != 0);
    // Deterministic
    const hash2 = marky_content_hash(text.ptr, text.len);
    try std.testing.expectEqual(hash, hash2);
}

test "marky_content_hash null text" {
    const result = marky_content_hash(null, 10);
    try std.testing.expectEqual(@as(u64, 0), result);
}

test "marky_content_hash zero length" {
    const text = "hello";
    const result = marky_content_hash(text.ptr, 0);
    // FNV offset basis for empty
    try std.testing.expectEqual(@as(u64, 0xcbf29ce484222325), result);
}

test "marky_content_hash distinct" {
    const hash1 = marky_content_hash("abc".ptr, 3);
    const hash2 = marky_content_hash("def".ptr, 3);
    try std.testing.expect(hash1 != hash2);
}

// -- marky_scan_tags tests --

test "marky_scan_tags basic" {
    const text = "text #tag1 #tag2";
    var out: [8]TagScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_tags(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
    try std.testing.expectEqual(@as(u32, 11), out[1].offset);
}

test "marky_scan_tags null text with zero len" {
    var w: u32 = undefined;
    var out: [4]TagScan = undefined;
    const rc = marky_scan_tags(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_tags null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]TagScan = undefined;
    const rc = marky_scan_tags(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_tags null written" {
    const text = "#tag";
    var out: [4]TagScan = undefined;
    const rc = marky_scan_tags(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_tags zero cap" {
    const text = "#tag";
    var out: [4]TagScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_tags(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_tags buffer overflow returns -2" {
    const text = "#a #b #c";
    var out: [1]TagScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_tags(text.ptr, text.len, &out, 1, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
}

// -- marky_scan_block_ids tests --

test "marky_scan_block_ids basic" {
    const text = "text ^block-id\n";
    var out: [8]BlockIdScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_block_ids(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 5), out[0].offset);
    try std.testing.expectEqual(@as(u16, 8), out[0].length);
}

test "marky_scan_block_ids null text with zero len" {
    var w: u32 = undefined;
    var out: [4]BlockIdScan = undefined;
    const rc = marky_scan_block_ids(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_block_ids null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]BlockIdScan = undefined;
    const rc = marky_scan_block_ids(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_block_ids null written" {
    const text = "text ^id\n";
    var out: [4]BlockIdScan = undefined;
    const rc = marky_scan_block_ids(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_block_ids zero cap" {
    const text = "text ^id\n";
    var out: [4]BlockIdScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_block_ids(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_block_ids not at EOL" {
    const text = "^id more text\n";
    var out: [4]BlockIdScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_block_ids(text.ptr, text.len, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

// -- zig_cosine_similarity tests --

test "zig_cosine_similarity basic" {
    const a = [_]f32{ 1.0, 2.0, 3.0, 4.0 };
    const b = [_]f32{ 1.0, 2.0, 3.0, 4.0 };
    const result = zig_cosine_similarity(&a, &b, 4);
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), result, 1e-5);
}

test "zig_cosine_similarity null a" {
    const b = [_]f32{ 1.0, 2.0 };
    const result = zig_cosine_similarity(null, &b, 2);
    try std.testing.expectEqual(@as(f32, -2.0), result);
}

test "zig_cosine_similarity null b" {
    const a = [_]f32{ 1.0, 2.0 };
    const result = zig_cosine_similarity(&a, null, 2);
    try std.testing.expectEqual(@as(f32, -2.0), result);
}

test "zig_cosine_similarity zero dims" {
    const a = [_]f32{1.0};
    const result = zig_cosine_similarity(&a, &a, 0);
    try std.testing.expectEqual(@as(f32, -2.0), result);
}

// -- zig_jaccard_similarity tests --

test "zig_jaccard_similarity basic" {
    const s1 = [_]u32{ 1, 2, 3, 4, 5 };
    const s2 = [_]u32{ 3, 4, 5, 6, 7 };
    const result = zig_jaccard_similarity(&s1, 5, &s2, 5);
    // intersection = {3,4,5} = 3, union = 5+5-3 = 7
    try std.testing.expectApproxEqAbs(@as(f32, 3.0 / 7.0), result, 1e-6);
}

test "zig_jaccard_similarity null set1" {
    const s2 = [_]u32{1};
    const result = zig_jaccard_similarity(null, 1, &s2, 1);
    try std.testing.expectEqual(@as(f32, -1.0), result);
}

test "zig_jaccard_similarity null set2" {
    const s1 = [_]u32{1};
    const result = zig_jaccard_similarity(&s1, 1, null, 1);
    try std.testing.expectEqual(@as(f32, -1.0), result);
}

// -- zig_extract_entity_hashes tests --

test "zig_extract_entity_hashes basic" {
    const text = "hello world";
    var out: [8]u32 = undefined;
    var w: u32 = undefined;
    const rc = zig_extract_entity_hashes(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "zig_extract_entity_hashes null text with zero len" {
    var out: [4]u32 = undefined;
    var w: u32 = undefined;
    const rc = zig_extract_entity_hashes(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "zig_extract_entity_hashes null text with nonzero len" {
    var out: [4]u32 = undefined;
    var w: u32 = undefined;
    const rc = zig_extract_entity_hashes(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_extract_entity_hashes null written" {
    const text = "hello";
    var out: [4]u32 = undefined;
    const rc = zig_extract_entity_hashes(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_extract_entity_hashes buffer overflow" {
    const text = "one two three four five";
    var out: [2]u32 = undefined;
    var w: u32 = undefined;
    const rc = zig_extract_entity_hashes(text.ptr, text.len, &out, 2, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "zig_extract_entity_hashes capacity zero sets written" {
    const text = "hello world";
    var w: u32 = 99;
    const rc = zig_extract_entity_hashes(text.ptr, text.len, null, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "zig_extract_entity_hashes text_len zero ignores null output_ids" {
    var w: u32 = 99;
    const rc = zig_extract_entity_hashes(null, 0, null, 0, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

// -- zig_normalize_f32_l2 tests --

test "zig_normalize_f32_l2 basic" {
    const input = [_]f32{ 3.0, 4.0, 0.0, 0.0 };
    var output: [4]f32 = undefined;
    const rc = zig_normalize_f32_l2(&input, &output, 4);
    try std.testing.expectEqual(@as(i32, 0), rc);
    var norm_sq: f32 = 0.0;
    for (output) |v| norm_sq += v * v;
    try std.testing.expectApproxEqAbs(@as(f32, 1.0), @sqrt(norm_sq), 1e-5);
}

test "zig_normalize_f32_l2 null input" {
    var output: [4]f32 = undefined;
    const rc = zig_normalize_f32_l2(null, &output, 4);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_normalize_f32_l2 null output" {
    const input = [_]f32{ 1.0, 0.0 };
    const rc = zig_normalize_f32_l2(&input, null, 2);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_normalize_f32_l2 zero n" {
    const input = [_]f32{1.0};
    var output: [1]f32 = undefined;
    const rc = zig_normalize_f32_l2(&input, &output, 0);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

// -- asm_quantize/dequantize tests --

test "zig_quantize_f32_to_q4_0 basic" {
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = (@as(f32, @floatFromInt(i)) - 16.0) / 16.0;
    }
    var q4_buf: [quantize_mod.Q4_BLOCK_BYTES]u8 = undefined;
    const rc = zig_quantize_f32_to_q4_0(&input, &q4_buf, 32);
    try std.testing.expectEqual(@as(i32, 0), rc);
}

test "zig_quantize_f32_to_q4_0 null input" {
    var q4_buf: [quantize_mod.Q4_BLOCK_BYTES]u8 = undefined;
    const rc = zig_quantize_f32_to_q4_0(null, &q4_buf, 32);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "zig_dequantize_q4_0_to_f32 round trip" {
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = (@as(f32, @floatFromInt(i)) - 16.0) / 16.0;
    }
    var q4_buf: [quantize_mod.Q4_BLOCK_BYTES]u8 = undefined;
    _ = zig_quantize_f32_to_q4_0(&input, &q4_buf, 32);

    var output: [32]f32 = undefined;
    const rc = zig_dequantize_q4_0_to_f32(&q4_buf, &output, 32);
    try std.testing.expectEqual(@as(i32, 0), rc);

    for (0..32) |i| {
        const err = @abs(input[i] - output[i]);
        try std.testing.expect(err < 0.15);
    }
}

test "zig_dequantize_q4_0_to_f32 null input" {
    var output: [32]f32 = undefined;
    const rc = zig_dequantize_q4_0_to_f32(null, &output, 32);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

// -- marky_build_fence_map tests --

test "marky_build_fence_map basic" {
    const text = "```\ncode here\n```\n";
    var out: [8]FenceRange = undefined;
    var w: u32 = undefined;
    const rc = marky_build_fence_map(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u32, 0), out[0].start);
    try std.testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "marky_build_fence_map null text with zero len" {
    var w: u32 = undefined;
    var out: [4]FenceRange = undefined;
    const rc = marky_build_fence_map(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_build_fence_map null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]FenceRange = undefined;
    const rc = marky_build_fence_map(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_build_fence_map null written" {
    const text = "```\ncode\n```\n";
    var out: [4]FenceRange = undefined;
    const rc = marky_build_fence_map(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_build_fence_map zero cap" {
    const text = "```\ncode\n```\n";
    var out: [4]FenceRange = undefined;
    var w: u32 = undefined;
    const rc = marky_build_fence_map(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_build_fence_map buffer overflow returns -2" {
    const text = "```\na\n```\n```\nb\n```\n```\nc\n```\n";
    var out: [1]FenceRange = undefined;
    var w: u32 = undefined;
    const rc = marky_build_fence_map(text.ptr, text.len, &out, 1, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqual(@as(u32, 1), w);
}
