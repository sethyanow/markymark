const std = @import("std");

/// Scalar L2 normalization of an f32 vector.
///
/// Divides each element by the L2 norm (Euclidean length) of the vector.
/// Result is a unit vector with ||output|| == 1.0.
///
/// Returns:
///   0  — success
///  -1  — invalid input (zero length)
///  -1  — zero vector (cannot normalize)
pub fn normalize_f32_l2(input: [*]const f32, output: [*]f32, n: u32) i32 {
    if (n == 0) return -1;

    const src = input[0..n];
    const dst = output[0..n];

    // Compute L2 norm in f64 for precision
    var sum_sq: f64 = 0.0;
    for (src) |v| {
        const fv: f64 = @floatCast(v);
        sum_sq += fv * fv;
    }

    if (sum_sq < 1e-30) return -1; // zero vector

    const norm: f64 = @sqrt(sum_sq);
    const inv_norm: f64 = 1.0 / norm;

    for (src, dst) |v, *d| {
        d.* = @floatCast(@as(f64, @floatCast(v)) * inv_norm);
    }

    return 0;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "ref_normalize_unit_length" {
    const input = [_]f32{ 3.0, 4.0 };
    var output: [2]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 2);
    try testing.expectEqual(@as(i32, 0), rc);
    // ||output|| should be 1.0
    const norm = @sqrt(output[0] * output[0] + output[1] * output[1]);
    try testing.expectApproxEqAbs(@as(f32, 1.0), norm, 1e-6);
    // 3/5 = 0.6, 4/5 = 0.8
    try testing.expectApproxEqAbs(@as(f32, 0.6), output[0], 1e-6);
    try testing.expectApproxEqAbs(@as(f32, 0.8), output[1], 1e-6);
}

test "ref_normalize_zero_vector" {
    const input = [_]f32{ 0.0, 0.0, 0.0 };
    var output: [3]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 3);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "ref_normalize_empty" {
    const input = [_]f32{1.0};
    var output: [1]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 0);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "ref_normalize_already_unit" {
    const input = [_]f32{ 1.0, 0.0, 0.0 };
    var output: [3]f32 = undefined;
    const rc = normalize_f32_l2(&input, &output, 3);
    try testing.expectEqual(@as(i32, 0), rc);
    try testing.expectApproxEqAbs(@as(f32, 1.0), output[0], 1e-6);
    try testing.expectApproxEqAbs(@as(f32, 0.0), output[1], 1e-6);
    try testing.expectApproxEqAbs(@as(f32, 0.0), output[2], 1e-6);
}
