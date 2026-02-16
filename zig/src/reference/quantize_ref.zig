const std = @import("std");
const math = std.math;

/// Q4_0 block size: 32 f32 values quantized into 16 bytes + 2 bytes scale = 18 bytes per block.
/// For simplicity, we use a basic scheme: each f32 maps to a 4-bit value (0-15),
/// and we store a per-block scale factor and zero point.
pub const Q4_BLOCK_SIZE: u32 = 32;
/// Bytes per Q4 block: 2 (scale as f16) + 16 (32 nibbles packed into 16 bytes)
pub const Q4_BLOCK_BYTES: u32 = 18;

/// Scalar Q4_0 quantization: f32 -> 4-bit packed format.
///
/// Input: n f32 values. n must be divisible by Q4_BLOCK_SIZE (32).
/// Output buffer must be at least (n / Q4_BLOCK_SIZE) * Q4_BLOCK_BYTES bytes.
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

        // Find absmax in block for scale
        var absmax: f32 = 0.0;
        for (block) |v| {
            const abs_v = @abs(v);
            if (abs_v > absmax) absmax = abs_v;
        }

        // Scale: maps [-absmax, absmax] to [0, 15]
        const scale: f32 = if (absmax > 0.0) absmax / 7.5 else 0.0;
        const inv_scale: f32 = if (scale > 0.0) 1.0 / scale else 0.0;

        // Write scale as 2 bytes (f16)
        const out_offset = block_idx * Q4_BLOCK_BYTES;
        const scale_f16: u16 = @bitCast(@as(f16, @floatCast(scale)));
        output[out_offset] = @truncate(scale_f16);
        output[out_offset + 1] = @truncate(scale_f16 >> 8);

        // Pack pairs of values into nibbles
        var byte_idx: u32 = 0;
        while (byte_idx < Q4_BLOCK_SIZE / 2) : (byte_idx += 1) {
            const v0 = block[byte_idx * 2];
            const v1 = block[byte_idx * 2 + 1];

            // Quantize: round((v / scale) + 7.5) clamped to [0, 15]
            const q0 = quantize_value(v0, inv_scale);
            const q1 = quantize_value(v1, inv_scale);

            output[out_offset + 2 + byte_idx] = (q1 << 4) | q0;
        }
    }

    return 0;
}

/// Scalar Q4_0 dequantization: 4-bit packed format -> f32.
///
/// Input: packed Q4 data. Output: n f32 values.
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

        // Read scale (f16 stored as 2 bytes little-endian)
        const scale_u16: u16 = @as(u16, input[in_offset]) | (@as(u16, input[in_offset + 1]) << 8);
        const scale: f32 = @floatCast(@as(f16, @bitCast(scale_u16)));

        // Unpack nibbles
        var byte_idx: u32 = 0;
        while (byte_idx < Q4_BLOCK_SIZE / 2) : (byte_idx += 1) {
            const nibbles = input[in_offset + 2 + byte_idx];
            const q0: u8 = nibbles & 0x0F;
            const q1: u8 = nibbles >> 4;

            output[out_start + byte_idx * 2] = (@as(f32, @floatFromInt(q0)) - 7.5) * scale;
            output[out_start + byte_idx * 2 + 1] = (@as(f32, @floatFromInt(q1)) - 7.5) * scale;
        }
    }

    return 0;
}

fn quantize_value(v: f32, inv_scale: f32) u8 {
    const q_f32 = v * inv_scale + 7.5;
    const clamped = @max(@as(f32, 0.0), @min(@as(f32, 15.0), q_f32));
    return @intFromFloat(@round(clamped));
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "ref_quantize_round_trip" {
    // 32 values — one block
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = (@as(f32, @floatFromInt(i)) - 16.0) / 16.0; // range [-1.0, ~0.9375]
    }

    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc_q = quantize_f32_to_q4_0(&input, &q4_buf, 32);
    try testing.expectEqual(@as(i32, 0), rc_q);

    var output: [32]f32 = undefined;
    const rc_d = dequantize_q4_0_to_f32(&q4_buf, &output, 32);
    try testing.expectEqual(@as(i32, 0), rc_d);

    // Round-trip error should be small for values in [-1, 1]
    for (0..32) |i| {
        const err = @abs(input[i] - output[i]);
        try testing.expect(err < 0.15); // Q4 has ~4-bit precision
    }
}

test "ref_quantize_invalid_n" {
    var input: [31]f32 = undefined;
    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc = quantize_f32_to_q4_0(&input, &q4_buf, 31); // not divisible by 32
    try testing.expectEqual(@as(i32, -1), rc);
}

test "ref_quantize_zero_n" {
    var input: [32]f32 = undefined;
    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc = quantize_f32_to_q4_0(&input, &q4_buf, 0);
    try testing.expectEqual(@as(i32, -1), rc);
}

test "ref_quantize_clamp_range" {
    // Values outside [-1,1] should still work (clamped by scale)
    var input: [32]f32 = undefined;
    for (0..32) |i| {
        input[i] = @as(f32, @floatFromInt(i)) - 16.0; // range [-16, 15]
    }

    var q4_buf: [Q4_BLOCK_BYTES]u8 = undefined;
    const rc = quantize_f32_to_q4_0(&input, &q4_buf, 32);
    try testing.expectEqual(@as(i32, 0), rc);

    var output: [32]f32 = undefined;
    _ = dequantize_q4_0_to_f32(&q4_buf, &output, 32);

    // Verify the round-trip preserves approximate values
    for (0..32) |i| {
        const err = @abs(input[i] - output[i]);
        // Wider error tolerance for larger range
        try testing.expect(err < 3.0);
    }
}

test "ref_quantize_all_zeros" {
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
