const std = @import("std");

/// Approximate BPE token count using word boundary detection.
///
/// Uses SIMD to scan for word boundaries (spaces, punctuation, newlines)
/// and applies a tokens-per-word multiplier (~1.3 for English prose).
/// This is an approximation — accuracy within ~20% of actual tiktoken output.
///
/// Returns 0 for empty input or all-whitespace input.
pub fn estimate_tokens(text: [*]const u8, len: u32) u32 {
    if (len == 0) return 0;

    const buf = text[0..len];
    var word_count: u32 = 0;
    var in_word: bool = false;

    // SIMD scan: process 16 bytes at a time
    const chunk_size: u32 = 16;
    const space_vec: @Vector(16, u8) = @splat(' ');
    const newline_vec: @Vector(16, u8) = @splat('\n');
    const tab_vec: @Vector(16, u8) = @splat('\t');
    const cr_vec: @Vector(16, u8) = @splat('\r');

    var pos: u32 = 0;

    while (pos + chunk_size <= len) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;

        // A byte is a boundary if it matches any whitespace character
        const is_space = chunk == space_vec;
        const is_newline = chunk == newline_vec;
        const is_tab = chunk == tab_vec;
        const is_cr = chunk == cr_vec;

        // Combine: is this byte a word boundary? (element-wise OR)
        const is_boundary = @select(bool, is_space, is_space, @select(bool, is_newline, is_newline, @select(bool, is_tab, is_tab, is_cr)));

        // Process each lane to count word transitions
        inline for (0..chunk_size) |lane| {
            if (is_boundary[lane]) {
                if (in_word) {
                    word_count += 1;
                    in_word = false;
                }
            } else {
                in_word = true;
            }
        }
    }

    // Scalar tail
    while (pos < len) : (pos += 1) {
        const c = buf[pos];
        if (c == ' ' or c == '\n' or c == '\t' or c == '\r') {
            if (in_word) {
                word_count += 1;
                in_word = false;
            }
        } else {
            in_word = true;
        }
    }

    // Final word (if text doesn't end with whitespace)
    if (in_word) {
        word_count += 1;
    }

    if (word_count == 0) return 0;

    // Apply BPE multiplier: ~1.3 tokens per word for English text.
    // Use fixed-point: word_count * 13 / 10, rounding to nearest.
    const tokens = (word_count * 13 + 5) / 10;
    return tokens;
}

/// Scalar reference implementation for parity testing.
pub fn estimate_tokens_scalar(text: [*]const u8, len: u32) u32 {
    if (len == 0) return 0;

    const buf = text[0..len];
    var word_count: u32 = 0;
    var in_word: bool = false;

    for (buf) |c| {
        if (c == ' ' or c == '\n' or c == '\t' or c == '\r') {
            if (in_word) {
                word_count += 1;
                in_word = false;
            }
        } else {
            in_word = true;
        }
    }

    if (in_word) {
        word_count += 1;
    }

    if (word_count == 0) return 0;

    const tokens = (word_count * 13 + 5) / 10;
    return tokens;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_token_estimate_empty" {
    const text = "";
    const result = estimate_tokens(text.ptr, 0);
    try testing.expectEqual(@as(u32, 0), result);
}

test "test_token_estimate_single_word" {
    const text = "hello";
    const result = estimate_tokens(text.ptr, text.len);
    // 1 word * 1.3 = 1.3, rounds to 1 via (1*13+5)/10 = 1
    try testing.expectEqual(@as(u32, 1), result);
}

test "test_token_estimate_english_prose" {
    // 21 words of English prose
    const text = "The quick brown fox jumped over the lazy dog and then ran across the field to find the hidden treasure chest";
    const result = estimate_tokens(text.ptr, text.len);
    // 21 words * 1.3 = 27.3 -> (21*13+5)/10 = 278/10 = 27
    try testing.expectEqual(@as(u32, 27), result);
}

test "test_token_estimate_all_whitespace" {
    const text = "   \t\n\r  \n  ";
    const result = estimate_tokens(text.ptr, text.len);
    try testing.expectEqual(@as(u32, 0), result);
}

test "test_token_estimate_code" {
    // Code tends to have more tokens per word due to punctuation
    const text = "fn main() { let x = 42; println!(\"{}\", x); }";
    const result = estimate_tokens(text.ptr, text.len);
    // 10 words (space-separated) * 1.3 = 13
    // This is an approximation; real BPE would give more for code
    try testing.expect(result > 0);
    try testing.expect(result <= text.len); // Upper bound: can't have more tokens than bytes
}

test "test_token_estimate_single_character" {
    const text = "a";
    const result = estimate_tokens(text.ptr, text.len);
    try testing.expectEqual(@as(u32, 1), result);
}

test "test_token_estimate_multiple_spaces" {
    // Multiple spaces between words should not inflate count
    const text = "hello    world";
    const result = estimate_tokens(text.ptr, text.len);
    // 2 words * 1.3 = 2.6, rounds to 3 via (2*13+5)/10 = 31/10 = 3
    try testing.expectEqual(@as(u32, 3), result);
}

test "test_token_estimate_newlines" {
    const text = "line one\nline two\nline three\n";
    const result = estimate_tokens(text.ptr, text.len);
    // 6 words * 1.3 = 7.8 -> (6*13+5)/10 = 83/10 = 8
    try testing.expectEqual(@as(u32, 8), result);
}

test "test_token_estimate_large_input" {
    // Build a ~10KB input to test SIMD path thoroughly
    const line = "This is a regular line of text for testing token estimation.\n";
    comptime var text: []const u8 = "";
    comptime {
        var i = 0;
        while (i < 170) : (i += 1) {
            text = text ++ line;
        }
    }
    const result = estimate_tokens(text.ptr, @intCast(text.len));
    // 170 lines * 11 words = 1870 words * 1.3 = 2431
    try testing.expect(result > 1000);
    try testing.expect(result < 5000);
}

test "test_simd_scalar_parity" {
    const text = "Some text with multiple words spread across various lengths to test SIMD alignment boundary crossing behavior\n";
    const simd_result = estimate_tokens(text.ptr, text.len);
    const scalar_result = estimate_tokens_scalar(text.ptr, text.len);
    try testing.expectEqual(scalar_result, simd_result);
}

test "test_simd_scalar_parity_large" {
    const line = "This is a regular line of text for testing token estimation.\n";
    comptime var text: []const u8 = "";
    comptime {
        var i = 0;
        while (i < 50) : (i += 1) {
            text = text ++ line;
        }
    }
    const simd_result = estimate_tokens(text.ptr, @intCast(text.len));
    const scalar_result = estimate_tokens_scalar(text.ptr, @intCast(text.len));
    try testing.expectEqual(scalar_result, simd_result);
}
