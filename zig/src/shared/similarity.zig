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

test "test_jaccard_simd_scalar_parity" {
    const s1 = [_]u32{ 10, 20, 30, 40, 50 };
    const s2 = [_]u32{ 20, 40, 60, 80 };
    const simd_result = jaccard_similarity(&s1, 5, &s2, 4);
    const scalar_result = ref.jaccard_similarity(&s1, 5, &s2, 4);
    try testing.expectEqual(scalar_result, simd_result);
}
