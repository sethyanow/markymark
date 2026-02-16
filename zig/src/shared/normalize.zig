const std = @import("std");
const ref = @import("../reference/normalize_ref.zig");

/// SIMD-accelerated L2 normalization of an f32 vector.
///
/// Divides each element by the L2 norm (Euclidean length).
/// Uses @Vector(4, f32) for sum-of-squares and scaling.
///
/// Returns:
///   0  — success
///  -1  — invalid input (zero length, zero vector)
pub fn normalize_f32_l2(input: [*]const f32, output: [*]f32, n: u32) i32 {
    if (n == 0) return -1;

    // Short vectors: use scalar
    if (n < 4) return ref.normalize_f32_l2(input, output, n);

    const src = input[0..n];
    const dst = output[0..n];

    const chunk_size: u32 = 4;
    const simd_end = n - (n % chunk_size);

    // Phase 1: compute sum of squares via SIMD
    var sum_sq_vec: @Vector(4, f32) = @splat(0.0);
    var pos: u32 = 0;

    while (pos < simd_end) : (pos += chunk_size) {
        const chunk: @Vector(4, f32) = src[pos..][0..chunk_size].*;
        sum_sq_vec += chunk * chunk;
    }

    var sum_sq: f64 = @floatCast(@reduce(.Add, sum_sq_vec));

    // Scalar tail for sum
    while (pos < n) : (pos += 1) {
        const fv: f64 = @floatCast(src[pos]);
        sum_sq += fv * fv;
    }

    if (sum_sq < 1e-30) return -1; // zero vector

    const inv_norm: f32 = @floatCast(1.0 / @sqrt(sum_sq));
    const inv_norm_vec: @Vector(4, f32) = @splat(inv_norm);

    // Phase 2: scale via SIMD
    pos = 0;
    while (pos < simd_end) : (pos += chunk_size) {
        const chunk: @Vector(4, f32) = src[pos..][0..chunk_size].*;
        const scaled = chunk * inv_norm_vec;
        dst[pos..][0..chunk_size].* = scaled;
    }

    // Scalar tail for scale
    while (pos < n) : (pos += 1) {
        dst[pos] = src[pos] * inv_norm;
    }

    return 0;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_normalize_unit_length" {
    const input = [_]f32{ 3.0, 4.0, 0.0, 0.0 };
    var output: [4]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 4);
    try testing.expectEqual(@as(i32, 0), rc);
    // Verify unit length
    var norm_sq: f32 = 0.0;
    for (output) |v| norm_sq += v * v;
    try testing.expectApproxEqAbs(@as(f32, 1.0), @sqrt(norm_sq), 1e-5);
}

test "test_normalize_zero_vector" {
    const input = [_]f32{ 0.0, 0.0, 0.0, 0.0 };
    var output: [4]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 4);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "test_normalize_empty" {
    const input = [_]f32{1.0};
    var output: [1]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 0);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "test_normalize_already_unit" {
    const input = [_]f32{ 1.0, 0.0, 0.0, 0.0 };
    var output: [4]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 4);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectApproxEqAbs(@as(f32, 1.0), output[0], 1e-6);
    try testing.expectApproxEqAbs(@as(f32, 0.0), output[1], 1e-6);
}

test "test_normalize_large_vector" {
    var input: [128]f32 = undefined;
    var output: [128]f32 = undefined;
    for (0..128) |i| {
        input[i] = @floatFromInt(i + 1);
    }
    const rc = normalize_f32_l2(&input, &output, 128);
    try testing.expectEqual(@as(i32, 0), rc);
    // Verify unit length
    var norm_sq: f64 = 0.0;
    for (output) |v| {
        const fv: f64 = @floatCast(v);
        norm_sq += fv * fv;
    }
    try testing.expectApproxEqAbs(@as(f64, 1.0), @sqrt(norm_sq), 1e-4);
}

test "test_normalize_simd_scalar_parity" {
    const sizes = [_]u32{ 3, 4, 5, 7, 8, 16, 17, 32, 33 };
    for (sizes) |n| {
        var input: [33]f32 = undefined;
        var simd_out: [33]f32 = undefined;
        var scalar_out: [33]f32 = undefined;
        for (0..n) |i| {
            input[i] = @as(f32, @floatFromInt(i + 1)) * 0.5;
        }
        const rc_simd = normalize_f32_l2(&input, &simd_out, n);
        const rc_scalar = ref.normalize_f32_l2(&input, &scalar_out, n);
        try testing.expectEqual(rc_scalar, rc_simd);
        if (rc_simd == 0) {
            for (0..n) |i| {
                try testing.expectApproxEqAbs(scalar_out[i], simd_out[i], 1e-5);
            }
        }
    }
}

test "test_normalize_negative_values" {
    const input = [_]f32{ -3.0, 4.0, -5.0, 0.0 };
    var output: [4]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 4);
    try testing.expectEqual(@as(i32, 0), rc);
    // Verify unit length
    var norm_sq: f32 = 0.0;
    for (output) |v| norm_sq += v * v;
    try testing.expectApproxEqAbs(@as(f32, 1.0), @sqrt(norm_sq), 1e-5);
    // Direction preserved: output[0] should be negative
    try testing.expect(output[0] < 0.0);
}
