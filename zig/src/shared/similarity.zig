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

fn is_worse(score_a: i32, index_a: u32, score_b: i32, index_b: u32) bool {
    if (score_a != score_b) return score_a < score_b;
    return index_a > index_b;
}

fn heap_sift_up(scores: []i32, indices: []u32, start_idx: u32) void {
    var idx = start_idx;
    while (idx > 0) {
        const parent = (idx - 1) / 2;
        if (!is_worse(scores[idx], indices[idx], scores[parent], indices[parent])) break;

        const score_tmp = scores[idx];
        scores[idx] = scores[parent];
        scores[parent] = score_tmp;

        const index_tmp = indices[idx];
        indices[idx] = indices[parent];
        indices[parent] = index_tmp;

        idx = parent;
    }
}

fn heap_sift_down(scores: []i32, indices: []u32, heap_len: u32, start_idx: u32) void {
    var idx = start_idx;
    while (true) {
        const left = idx * 2 + 1;
        if (left >= heap_len) break;

        var worst = left;
        const right = left + 1;
        if (right < heap_len and is_worse(scores[right], indices[right], scores[left], indices[left])) {
            worst = right;
        }

        if (!is_worse(scores[worst], indices[worst], scores[idx], indices[idx])) break;

        const score_tmp = scores[idx];
        scores[idx] = scores[worst];
        scores[worst] = score_tmp;

        const index_tmp = indices[idx];
        indices[idx] = indices[worst];
        indices[worst] = index_tmp;

        idx = worst;
    }
}

fn heap_sort_descending(scores: []i32, indices: []u32, count: u32) void {
    if (count <= 1) return;

    var end = count;
    while (end > 1) {
        end -= 1;

        const score_tmp = scores[0];
        scores[0] = scores[end];
        scores[end] = score_tmp;

        const index_tmp = indices[0];
        indices[0] = indices[end];
        indices[end] = index_tmp;

        heap_sift_down(scores, indices, end, 0);
    }
}

/// Batched top-k fuzzy matching across candidate strings.
///
/// Candidate selection is deterministic:
/// - score descending
/// - candidate index ascending on ties
///
/// Returns:
///   0  — success
///  -1  — invalid input (null candidate pointer with non-zero length)
///  -2  — invalid output capacity (`top_k > output_cap` or `output_cap == 0` when `top_k > 0`)
pub fn fuzzy_match_top_k(
    query_ptr: [*]const u8,
    query_len: u32,
    candidate_ptrs: [*]const ?[*]const u8,
    candidate_lens: [*]const u32,
    candidate_count: u32,
    scores_out: [*]i32,
    indices_out: [*]u32,
    output_cap: u32,
    top_k: u32,
    written: *u32,
) i32 {
    written.* = 0;

    if (query_len == 0 or candidate_count == 0 or top_k == 0) return 0;
    if (output_cap == 0 or top_k > output_cap) return -2;

    const effective_k = @min(top_k, candidate_count);
    const scores = scores_out[0..output_cap];
    const indices = indices_out[0..output_cap];

    var selected: u32 = 0;
    var i: u32 = 0;
    while (i < candidate_count) : (i += 1) {
        const candidate_len = candidate_lens[i];
        const candidate_ptr = candidate_ptrs[i] orelse {
            if (candidate_len == 0) continue;
            written.* = 0;
            return -1;
        };

        const score = fuzzy_match_score(query_ptr, query_len, candidate_ptr, candidate_len);
        if (score <= 0) continue;

        if (selected < effective_k) {
            scores[selected] = score;
            indices[selected] = i;
            heap_sift_up(scores, indices, selected);
            selected += 1;
            continue;
        }

        if (selected > 0 and is_worse(scores[0], indices[0], score, i)) {
            scores[0] = score;
            indices[0] = i;
            heap_sift_down(scores, indices, selected, 0);
        }
    }

    heap_sort_descending(scores, indices, selected);
    written.* = selected;
    return 0;
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

test "test_fuzzy_match_top_k_stable_ties" {
    const query = "ab";
    const candidates = [_]?[*]const u8{
        "acb".ptr,
        "adb".ptr,
        "aeb".ptr,
    };
    const lengths = [_]u32{ 3, 3, 3 };
    var scores: [3]i32 = undefined;
    var indices: [3]u32 = undefined;
    var written: u32 = 0;

    const rc = fuzzy_match_top_k(
        query.ptr,
        query.len,
        &candidates,
        &lengths,
        candidates.len,
        &scores,
        &indices,
        scores.len,
        2,
        &written,
    );

    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 2), written);
    try testing.expectEqual(@as(u32, 0), indices[0]);
    try testing.expectEqual(@as(u32, 1), indices[1]);
    try testing.expect(scores[0] >= scores[1]);
}

test "test_fuzzy_match_top_k_empty_query_returns_zero" {
    const candidates = [_]?[*]const u8{"stage".ptr};
    const lengths = [_]u32{5};
    var scores: [1]i32 = undefined;
    var indices: [1]u32 = undefined;
    var written: u32 = 123;

    const rc = fuzzy_match_top_k(
        "".ptr,
        0,
        &candidates,
        &lengths,
        candidates.len,
        &scores,
        &indices,
        scores.len,
        1,
        &written,
    );

    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectEqual(@as(u32, 0), written);
}

test "test_fuzzy_match_top_k_capacity_guard" {
    const query = "st";
    const candidates = [_]?[*]const u8{"stage".ptr};
    const lengths = [_]u32{5};
    var scores: [1]i32 = undefined;
    var indices: [1]u32 = undefined;
    var written: u32 = 0;

    const rc = fuzzy_match_top_k(
        query.ptr,
        query.len,
        &candidates,
        &lengths,
        candidates.len,
        &scores,
        &indices,
        scores.len,
        2,
        &written,
    );

    try testing.expectEqual(@as(i32, -2), rc);
    try testing.expectEqual(@as(u32, 0), written);
}

test "test_jaccard_simd_scalar_parity" {
    const s1 = [_]u32{ 10, 20, 30, 40, 50 };
    const s2 = [_]u32{ 20, 40, 60, 80 };
    const simd_result = jaccard_similarity(&s1, 5, &s2, 4);
    const scalar_result = ref.jaccard_similarity(&s1, 5, &s2, 4);
    try testing.expectEqual(scalar_result, simd_result);
}

// ============================================================================
// PR29 Copilot triage: prefix bonus false-positive investigation (marky-8s3.10)
//
// Copilot claimed the prefix bonus could be applied incorrectly because it
// "happens AFTER the main matching loop" and "the matches might not be
// consecutive from position 0".
//
// Finding: FALSE POSITIVE. The prefix check is an intentional, independent
// text comparison (candidate[0..N] == query). The greedy left-to-right
// subsequence scan guarantees that when a candidate starts with the query
// text, the matches ARE found at positions 0..N-1. The two checks are
// deliberately decoupled: one scores the match, the other awards a relevance
// bonus for text-prefix candidates.
// ============================================================================

test "test_fuzzy_prefix_bonus_applied_when_candidate_starts_with_query" {
    // "stage" starts with "st": s(0) t(1) consecutive, prefix bonus applied.
    // Expected: 10 (s) + 10 (t) + 5 (consecutive) + 200 (prefix) = 225
    const score = fuzzy_match_score("st".ptr, 2, "stage".ptr, 5);
    try testing.expectEqual(@as(i32, 225), score);
}

test "test_fuzzy_prefix_bonus_not_applied_for_non_prefix_subsequence" {
    // "restart" contains "st" as consecutive subsequence at positions 2,3,
    // but "restart" does NOT start with "st". No prefix bonus.
    // Expected: 10 (s) + 10 (t) + 5 (consecutive) = 25
    const score = fuzzy_match_score("st".ptr, 2, "restart".ptr, 7);
    try testing.expectEqual(@as(i32, 25), score);
}

test "test_fuzzy_prefix_bonus_greedy_guarantees_prefix_positions" {
    // Copilot concern: subsequence matches might not be at prefix positions
    // even when candidate starts with query.  Counter-evidence: greedy
    // left-to-right scan always takes the earliest match, so for "acid"
    // and query "ac", positions 0 and 1 are taken (consecutive = +5) and
    // the prefix check sees "ac" == "ac" → correct.
    // Expected: 10 + 10 + 5 + 200 = 225
    const score = fuzzy_match_score("ac".ptr, 2, "acid".ptr, 4);
    try testing.expectEqual(@as(i32, 225), score);
}

test "test_fuzzy_prefix_bonus_not_applied_for_non_prefix_gap_match" {
    // "abcd": query "ac" matches a(0) c(2) with gap=1.
    // "abcd" does NOT start with "ac" (starts with "ab"). No prefix bonus.
    // Expected: 10 (a) + 10 (c) - 1 (gap of 1) = 19
    const score = fuzzy_match_score("ac".ptr, 2, "abcd".ptr, 4);
    try testing.expectEqual(@as(i32, 19), score);
}
