const std = @import("std");
const math = std.math;
const ref = @import("../reference/similarity_ref.zig");

/// SIMD-accelerated cosine similarity between two f32 vectors.
///
/// Uses @Vector(4, f32) for dot product and norm accumulation.
/// Falls back to scalar reference for vectors shorter than 4 elements.
///
/// Returns the cosine of the angle between vectors a and b:
///   dot(a,b) / (||a|| * ||b||)
///
/// Returns -2.0 on error (zero length, zero-magnitude vector).
pub fn cosine_similarity(a: [*]const f32, b: [*]const f32, dims: u32) f32 {
    if (dims == 0) return -2.0;

    // For short vectors, use scalar path
    if (dims < 4) return ref.cosine_similarity(a, b, dims);

    const va = a[0..dims];
    const vb = b[0..dims];

    const chunk_size: u32 = 4;
    const simd_end = dims - (dims % chunk_size);

    var dot_acc: @Vector(4, f32) = @splat(0.0);
    var norm_a_acc: @Vector(4, f32) = @splat(0.0);
    var norm_b_acc: @Vector(4, f32) = @splat(0.0);

    var pos: u32 = 0;
    while (pos < simd_end) : (pos += chunk_size) {
        const chunk_a: @Vector(4, f32) = va[pos..][0..chunk_size].*;
        const chunk_b: @Vector(4, f32) = vb[pos..][0..chunk_size].*;

        dot_acc += chunk_a * chunk_b;
        norm_a_acc += chunk_a * chunk_a;
        norm_b_acc += chunk_b * chunk_b;
    }

    // Horizontal sum of SIMD accumulators
    var dot: f64 = @floatCast(@reduce(.Add, dot_acc));
    var norm_a: f64 = @floatCast(@reduce(.Add, norm_a_acc));
    var norm_b: f64 = @floatCast(@reduce(.Add, norm_b_acc));

    // Scalar tail
    while (pos < dims) : (pos += 1) {
        const fa: f64 = @floatCast(va[pos]);
        const fb: f64 = @floatCast(vb[pos]);
        dot += fa * fb;
        norm_a += fa * fa;
        norm_b += fb * fb;
    }

    const mag = @sqrt(norm_a) * @sqrt(norm_b);
    if (mag < 1e-30) return -2.0;

    return @floatCast(dot / mag);
}

/// SIMD-accelerated Jaccard similarity between two sorted u32 hash sets.
///
/// Both sets MUST be sorted in ascending order.
/// Returns |intersection| / |union| as f32.
///
/// Note: Jaccard on sorted sets is inherently a merge-join (sequential),
/// so SIMD provides limited benefit. This implementation uses the scalar
/// reference directly. The C ABI export is provided for API consistency.
///
/// Returns 0.0 for empty sets.
pub fn jaccard_similarity(set1: [*]const u32, set1_len: u32, set2: [*]const u32, set2_len: u32) f32 {
    // Sorted merge-join is inherently sequential — no SIMD benefit.
    // Delegate to scalar reference for correctness.
    return ref.jaccard_similarity(set1, set1_len, set2, set2_len);
}

/// Fuzzy match score between query and candidate.
///
/// Scoring model (integer, higher is better):
/// - Prefix (candidate starts with query): +200 bonus
/// - Character match: +10 each
/// - Consecutive match extension: +5 each after the first
/// - Gap penalty between matched chars: -1 per skipped byte
///
/// Matching is ASCII case-insensitive and subsequence-based.
/// Returns 0 when query is not a subsequence of candidate.
pub fn fuzzy_match_score(
    query_ptr: [*]const u8,
    query_len: u32,
    candidate_ptr: [*]const u8,
    candidate_len: u32,
) i32 {
    if (query_len == 0 or candidate_len == 0) return 0;

    const query = query_ptr[0..query_len];
    const candidate = candidate_ptr[0..candidate_len];

    var qi: usize = 0;
    var ci: usize = 0;

    var score: i32 = 0;
    var last_match_idx: ?usize = null;

    while (qi < query.len and ci < candidate.len) : (ci += 1) {
        const qch = std.ascii.toLower(query[qi]);
        const cch = std.ascii.toLower(candidate[ci]);

        if (qch != cch) continue;

        score += 10;

        if (last_match_idx) |prev| {
            if (ci == prev + 1) {
                score += 5;
            } else {
                const gap: i32 = @intCast(ci - prev - 1);
                score -= gap;
            }
        }

        last_match_idx = ci;
        qi += 1;
    }

    if (qi != query.len) return 0;

    if (candidate.len >= query.len) {
        var prefix = true;
        var i: usize = 0;
        while (i < query.len) : (i += 1) {
            if (std.ascii.toLower(query[i]) != std.ascii.toLower(candidate[i])) {
                prefix = false;
                break;
            }
        }
        if (prefix) score += 200;
    }

    if (score < 0) return 0;
    return score;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_cosine_identical" {
    const a = [_]f32{ 1.0, 2.0, 3.0, 4.0 };
    const b = [_]f32{ 1.0, 2.0, 3.0, 4.0 };
    const result = cosine_similarity(&a, &b, 4);
    try testing.expectApproxEqAbs(@as(f32, 1.0), result, 1e-5);
}

test "test_cosine_orthogonal" {
    const a = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    const b = [_]f32{ 0.0, 1.0, 0.0, 0.0 };
    const result = cosine_similarity(&a, &b, 4);
    try testing.expectApproxEqAbs(@as(f32, 0.0), result, 1e-5);
}

test "test_cosine_opposite" {
    const a = [_]f32{ 1.0, 2.0, 3.0, 4.0 };
    const b = [_]f32{ -1.0, -2.0, -3.0, -4.0 };
    const result = cosine_similarity(&a, &b, 4);
    try testing.expectApproxEqAbs(@as(f32, -1.0), result, 1e-5);
}

test "test_cosine_zero_dims" {
    const a = [_]f32{1.0};
    const result = cosine_similarity(&a, &a, 0);
    try testing.expectEqual(@as(f32, -2.0), result);
}

test "test_cosine_zero_magnitude" {
    const a = [_]f32{ 0.0, 0.0, 0.0, 0.0 };
    const b = [_]f32{ 1.0, 2.0, 3.0, 4.0 };
    const result = cosine_similarity(&a, &b, 4);
    try testing.expectEqual(@as(f32, -2.0), result);
}

test "test_cosine_large_vector" {
    // 128-dim vector to exercise multiple SIMD iterations
    var a: [128]f32 = undefined;
    var b: [128]f32 = undefined;
    for (0..128) |i| {
        a[i] = @floatFromInt(i);
        b[i] = @floatFromInt(128 - i);
    }
    const result = cosine_similarity(&a, &b, 128);
    // Both non-zero, non-orthogonal — should be a valid cosine value
    try testing.expect(result > -1.0 and result < 1.0);
}

test "test_cosine_simd_scalar_parity" {
    // Compare SIMD vs scalar reference for various sizes
    const sizes = [_]u32{ 3, 4, 5, 7, 8, 15, 16, 17, 31, 32, 33, 64, 100 };
    for (sizes) |n| {
        var a: [100]f32 = undefined;
        var b_arr: [100]f32 = undefined;
        for (0..n) |i| {
            a[i] = @as(f32, @floatFromInt(i + 1)) * 0.1;
            b_arr[i] = @as(f32, @floatFromInt(n - i)) * 0.1;
        }
        const simd_result = cosine_similarity(&a, &b_arr, n);
        const scalar_result = ref.cosine_similarity(&a, &b_arr, n);
        try testing.expectApproxEqAbs(scalar_result, simd_result, 1e-4);
    }
}

test "test_jaccard_identical" {
    const s = [_]u32{ 1, 2, 3, 4, 5 };
    const result = jaccard_similarity(&s, 5, &s, 5);
    try testing.expectApproxEqAbs(@as(f32, 1.0), result, 1e-6);
}

test "test_jaccard_disjoint" {
    const s1 = [_]u32{ 1, 2, 3 };
    const s2 = [_]u32{ 4, 5, 6 };
    const result = jaccard_similarity(&s1, 3, &s2, 3);
    try testing.expectApproxEqAbs(@as(f32, 0.0), result, 1e-6);
}

test "test_jaccard_partial_overlap" {
    const s1 = [_]u32{ 1, 2, 3, 4 };
    const s2 = [_]u32{ 3, 4, 5, 6 };
    const result = jaccard_similarity(&s1, 4, &s2, 4);
    try testing.expectApproxEqAbs(@as(f32, 2.0 / 6.0), result, 1e-6);
}

test "test_jaccard_empty" {
    const s = [_]u32{1};
    const result = jaccard_similarity(&s, 0, &s, 0);
    try testing.expectApproxEqAbs(@as(f32, 0.0), result, 1e-6);
}

test "test_fuzzy_match_prefix_scores_higher_than_substring" {
    const prefix = fuzzy_match_score("st".ptr, 2, "stage".ptr, 5);
    const substring = fuzzy_match_score("st".ptr, 2, "setup".ptr, 5);

    try testing.expect(prefix > 0);
    try testing.expect(substring > 0);
    try testing.expect(prefix > substring);
}

test "test_fuzzy_match_case_insensitive" {
    const score = fuzzy_match_score("ST".ptr, 2, "Setup".ptr, 5);
    try testing.expect(score > 0);
}

test "test_fuzzy_match_subsequence" {
    const score = fuzzy_match_score("stp".ptr, 3, "setup".ptr, 5);
    try testing.expect(score > 0);
}

test "test_fuzzy_match_no_match_returns_zero" {
    const score = fuzzy_match_score("zzz".ptr, 3, "setup".ptr, 5);
    try testing.expectEqual(@as(i32, 0), score);
}

test "test_jaccard_simd_scalar_parity" {
    const s1 = [_]u32{ 10, 20, 30, 40, 50 };
    const s2 = [_]u32{ 20, 40, 60, 80 };
    const simd_result = jaccard_similarity(&s1, 5, &s2, 4);
    const scalar_result = ref.jaccard_similarity(&s1, 5, &s2, 4);
    try testing.expectEqual(scalar_result, simd_result);
}
