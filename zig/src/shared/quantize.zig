const std = @import("std");
const ref = @import("../reference/quantize_ref.zig");

pub const Q4_BLOCK_SIZE = ref.Q4_BLOCK_SIZE;
pub const Q4_BLOCK_BYTES = ref.Q4_BLOCK_BYTES;

/// SIMD-accelerated Q4_0 quantization: f32 -> 4-bit packed format.
///
/// Uses @Vector(4, f32) for absmax finding and quantization arithmetic.
/// n must be divisible by Q4_BLOCK_SIZE (32).
///
/// Returns:
///   0  — success
///  -1  — invalid input (n not divisible by block size, or zero)
pub fn quantize_f32_to_q4_0(input: [*]const f32, output: [*]u8, n: u32) i32 {
    if (n == 0 or n % Q4_BLOCK_SIZE != 0) return -1;

    const src = input[0..n];
    const num_blocks = n / Q4_BLOCK_SIZE;

    var block_idx: u32 = 0;
    while (block_idx < num_blocks) : (block_idx += 1) {
        const block_start = block_idx * Q4_BLOCK_SIZE;
        const block = src[block_start..][0..Q4_BLOCK_SIZE];

        // SIMD absmax: process 4 elements at a time
        var max_vec: @Vector(4, f32) = @splat(0.0);
        var i: u32 = 0;
        while (i < Q4_BLOCK_SIZE) : (i += 4) {
            const chunk: @Vector(4, f32) = block[i..][0..4].*;
            const abs_chunk = @abs(chunk);
            max_vec = @max(max_vec, abs_chunk);
        }
        const absmax = @reduce(.Max, max_vec);

        const scale: f32 = if (absmax > 0.0) absmax / 7.5 else 0.0;
        const inv_scale: f32 = if (scale > 0.0) 1.0 / scale else 0.0;

        // Write scale as f16
        const out_offset = block_idx * Q4_BLOCK_BYTES;
        const scale_f16: u16 = @bitCast(@as(f16, @floatCast(scale)));
        output[out_offset] = @truncate(scale_f16);
        output[out_offset + 1] = @truncate(scale_f16 >> 8);

        // Quantize and pack pairs into nibbles
        // SIMD quantization: process 4 values, extract nibbles
        const offset_vec: @Vector(4, f32) = @splat(7.5);
        const inv_scale_vec: @Vector(4, f32) = @splat(inv_scale);
        const zero_vec: @Vector(4, f32) = @splat(0.0);
        const fifteen_vec: @Vector(4, f32) = @splat(15.0);

        var byte_idx: u32 = 0;
        i = 0;
        while (i < Q4_BLOCK_SIZE) : (i += 4) {
            const chunk: @Vector(4, f32) = block[i..][0..4].*;
            const q_float = @min(fifteen_vec, @max(zero_vec, chunk * inv_scale_vec + offset_vec));

            // Extract and round each lane
            const q0: u8 = @intFromFloat(@round(q_float[0]));
            const q1: u8 = @intFromFloat(@round(q_float[1]));
            const q2: u8 = @intFromFloat(@round(q_float[2]));
            const q3: u8 = @intFromFloat(@round(q_float[3]));

            // Pack pairs: (q1 << 4 | q0), (q3 << 4 | q2)
            output[out_offset + 2 + byte_idx] = (q1 << 4) | q0;
            output[out_offset + 2 + byte_idx + 1] = (q3 << 4) | q2;
            byte_idx += 2;
        }
    }

    return 0;
}

/// SIMD-accelerated Q4_0 dequantization: 4-bit packed format -> f32.
///
/// Uses @Vector(4, f32) for scaling.
/// n must be divisible by Q4_BLOCK_SIZE (32).
///
/// Returns:
///   0  — success
///  -1  — invalid input
pub fn dequantize_q4_0_to_f32(input: [*]const u8, output: [*]f32, n: u32) i32 {
    if (n == 0 or n % Q4_BLOCK_SIZE != 0) return -1;

    const num_blocks = n / Q4_BLOCK_SIZE;

    var block_idx: u32 = 0;
    while (block_idx < num_blocks) : (block_idx += 1) {
        const in_offset = block_idx * Q4_BLOCK_BYTES;
        const out_start = block_idx * Q4_BLOCK_SIZE;

        // Read scale (f16 little-endian)
        const scale_u16: u16 = @as(u16, input[in_offset]) | (@as(u16, input[in_offset + 1]) << 8);
        const scale: f32 = @floatCast(@as(f16, @bitCast(scale_u16)));

        const scale_vec: @Vector(4, f32) = @splat(scale);
        const offset_vec: @Vector(4, f32) = @splat(7.5);

        // Unpack nibbles and dequantize 4 at a time
        var byte_idx: u32 = 0;
        var out_idx: u32 = 0;
        while (out_idx < Q4_BLOCK_SIZE) : (out_idx += 4) {
            const packed0 = input[in_offset + 2 + byte_idx];
            const packed1 = input[in_offset + 2 + byte_idx + 1];

            const q: @Vector(4, f32) = .{
                @floatFromInt(packed0 & 0x0F),
                @floatFromInt(packed0 >> 4),
                @floatFromInt(packed1 & 0x0F),
                @floatFromInt(packed1 >> 4),
            };

            const dequant = (q - offset_vec) * scale_vec;
            output[out_start + out_idx ..][0..4].* = dequant;
            byte_idx += 2;
        }
    }

    return 0;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_quantize_round_trip" {
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = (@as(f32, @floatFromInt(i)) - 16.0) / 16.0;
    }

    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc_q = quantize_f32_to_q4_0(&input, &q4_buf, 32);
    try testing.expectEqual(@as(i32, 0), rc_q);

    var output: [32]f32 = undefined;
    const rc_d = dequantize_q4_0_to_f32(&q4_buf, &output, 32);
    try testing.expectEqual(@as(i32, 0), rc_d);

    for (0..32) |i| {
        const err = @abs(input[i] - output[i]);
        try testing.expect(err < 0.15);
    }
}

test "test_quantize_invalid_n" {
    var input: [31]f32 = undefined;
    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc = quantize_f32_to_q4_0(&input, &q4_buf, 31);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "test_quantize_zero_n" {
    var input: [32]f32 = undefined;
    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc = quantize_f32_to_q4_0(&input, &q4_buf, 0);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "test_quantize_clamp_range" {
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = @as(f32, @floatFromInt(i)) - 16.0;
    }

    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc = quantize_f32_to_q4_0(&input, &q4_buf, 32);
    try testing.expectEqual(@as(i32, 0), rc);

    var output: [32]f32 = undefined;
    _ = dequantize_q4_0_to_f32(&q4_buf, &output, 32);

    for (0..32) |i| {
        const err = @abs(input[i] - output[i]);
        try testing.expect(err < 3.0);
    }
}

test "test_quantize_all_zeros" {
    var input: [32]f32 = [_]f32{0.0} ** 32;
    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc = quantize_f32_to_q4_0(&input, &q4_buf, 32);
    try testing.expectEqual(@as(i32, 0), rc);

    var output: [32]f32 = undefined;
    _ = dequantize_q4_0_to_f32(&q4_buf, &output, 32);

    for (output) |v| {
        try testing.expectApproxEqAbs(@as(f32, 0.0), v, 1e-6);
    }
}

test "test_quantize_multiple_blocks" {
    var input: [64]f32 = undefined;
    for (0..64) |i| {
        input[i] = @sin(@as(f32, @floatFromInt(i)) * 0.1);
    }

    var q4_buf: [Q4_BLOCK_BYTES * 2]u8 = undefined;
    const rc_q = quantize_f32_to_q4_0(&input, &q4_buf, 64);
    try testing.expectEqual(@as(i32, 0), rc_q);

    var output: [64]f32 = undefined;
    const rc_d = dequantize_q4_0_to_f32(&q4_buf, &output, 64);
    try testing.expectEqual(@as(i32, 0), rc_d);

    for (0..64) |i| {
        const err = @abs(input[i] - output[i]);
        try testing.expect(err < 0.15);
    }
}

test "test_quantize_simd_scalar_parity" {
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = (@as(f32, @floatFromInt(i)) - 16.0) / 16.0;
    }

    var simd_packed: [Q4_BLOCK_BYTES]u8 = undefined;
    var scalar_packed: [Q4_BLOCK_BYTES]u8 = undefined;

    _ = quantize_f32_to_q4_0(&input, &simd_packed, 32);
    _ = ref.quantize_f32_to_q4_0(&input, &scalar_packed, 32);

    // Both should produce identical packed bytes
    for (0..Q4_BLOCK_BYTES) |i| {
        try testing.expectEqual(scalar_packed[i], simd_packed[i]);
    }
}
