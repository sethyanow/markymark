const std = @import("std");
const math = std.math;

/// Scalar cosine similarity between two f32 vectors.
///
/// Returns the cosine of the angle between vectors a and b:
///   dot(a,b) / (||a|| * ||b||)
///
/// Returns -2.0 on error (null pointers, zero length, zero-magnitude vector).
pub fn cosine_similarity(a: [*]const f32, b: [*]const f32, dims: u32) f32 {
    if (dims == 0) return -2.0;

    const va = a[0..dims];
    const vb = b[0..dims];

    var dot: f64 = 0.0;
    var norm_a: f64 = 0.0;
    var norm_b: f64 = 0.0;

    for (va, vb) |ai, bi| {
        const fa: f64 = @floatCast(ai);
        const fb: f64 = @floatCast(bi);
        dot += fa * fb;
        norm_a += fa * fa;
        norm_b += fb * fb;
    }

    const mag = @sqrt(norm_a) * @sqrt(norm_b);
    if (mag < 1e-30) return -2.0; // zero-magnitude vector

    return @floatCast(dot / mag);
}

/// Scalar Jaccard similarity between two sorted u32 hash sets.
///
/// Both sets MUST be sorted in ascending order.
/// Returns |intersection| / |union| as f32.
///
/// Returns -1.0 on error (null pointers).
/// Returns 0.0 for empty sets (|union| == 0).
/// Returns 1.0 for identical sets.
pub fn jaccard_similarity(set1: [*]const u32, set1_len: u32, set2: [*]const u32, set2_len: u32) f32 {
    if (set1_len == 0 and set2_len == 0) return 0.0;
    if (set1_len == 0 or set2_len == 0) return 0.0;

    const s1 = set1[0..set1_len];
    const s2 = set2[0..set2_len];

    var i: u32 = 0;
    var j: u32 = 0;
    var intersection: u32 = 0;

    while (i < set1_len and j < set2_len) {
        if (s1[i] == s2[j]) {
            intersection += 1;
            i += 1;
            j += 1;
        } else if (s1[i] < s2[j]) {
            i += 1;
        } else {
            j += 1;
        }
    }

    const union_size = set1_len + set2_len - intersection;
    if (union_size == 0) return 0.0;

    return @as(f32, @floatFromInt(intersection)) / @as(f32, @floatFromInt(union_size));
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "ref_cosine_identical_vectors" {
    const a = [_]f32{ 1.0, 2.0, 3.0 };
    const b = [_]f32{ 1.0, 2.0, 3.0 };
    const result = cosine_similarity(&a, &b, 3);
    try testing.expectApproxEqAbs(@as(f32, 1.0), result, 1e-6);
}

test "ref_cosine_orthogonal_vectors" {
    const a = [_]f32{ 1.0, 0.0, 0.0 };
    const b = [_]f32{ 0.0, 1.0, 0.0 };
    const result = cosine_similarity(&a, &b, 3);
    try testing.expectApproxEqAbs(@as(f32, 0.0), result, 1e-6);
}

test "ref_cosine_opposite_vectors" {
    const a = [_]f32{ 1.0, 2.0, 3.0 };
    const b = [_]f32{ -1.0, -2.0, -3.0 };
    const result = cosine_similarity(&a, &b, 3);
    try testing.expectApproxEqAbs(@as(f32, -1.0), result, 1e-6);
}

test "ref_cosine_zero_length" {
    const a = [_]f32{1.0};
    const result = cosine_similarity(&a, &a, 0);
    try testing.expectEqual(@as(f32, -2.0), result);
}

test "ref_cosine_zero_magnitude" {
    const a = [_]f32{ 0.0, 0.0, 0.0 };
    const b = [_]f32{ 1.0, 2.0, 3.0 };
    const result = cosine_similarity(&a, &b, 3);
    try testing.expectEqual(@as(f32, -2.0), result);
}

test "ref_jaccard_identical_sets" {
    const s = [_]u32{ 1, 2, 3, 4, 5 };
    const result = jaccard_similarity(&s, 5, &s, 5);
    try testing.expectApproxEqAbs(@as(f32, 1.0), result, 1e-6);
}

test "ref_jaccard_disjoint_sets" {
    const s1 = [_]u32{ 1, 2, 3 };
    const s2 = [_]u32{ 4, 5, 6 };
    const result = jaccard_similarity(&s1, 3, &s2, 3);
    try testing.expectApproxEqAbs(@as(f32, 0.0), result, 1e-6);
}

test "ref_jaccard_partial_overlap" {
    const s1 = [_]u32{ 1, 2, 3, 4 };
    const s2 = [_]u32{ 3, 4, 5, 6 };
    // intersection = {3,4} = 2, union = 4+4-2 = 6
    const result = jaccard_similarity(&s1, 4, &s2, 4);
    try testing.expectApproxEqAbs(@as(f32, 2.0 / 6.0), result, 1e-6);
}

test "ref_jaccard_empty_sets" {
    const s = [_]u32{1};
    const result = jaccard_similarity(&s, 0, &s, 0);
    try testing.expectApproxEqAbs(@as(f32, 0.0), result, 1e-6);
}

test "ref_jaccard_one_empty" {
    const s1 = [_]u32{ 1, 2, 3 };
    const s2 = [_]u32{1};
    const result = jaccard_similarity(&s1, 3, &s2, 0);
    try testing.expectApproxEqAbs(@as(f32, 0.0), result, 1e-6);
}
