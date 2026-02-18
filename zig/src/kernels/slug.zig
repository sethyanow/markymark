const std = @import("std");

inline fn is_ascii_upper(b: u8) bool {
    return b >= 'A' and b <= 'Z';
}

inline fn is_ascii_alnum(b: u8) bool {
    return (b >= 'a' and b <= 'z') or (b >= 'A' and b <= 'Z') or (b >= '0' and b <= '9');
}

inline fn is_hyphen_delim(b: u8) bool {
    return switch (b) {
        ' ', '\t', '\n', '\r' => true,
        '-', '_', '.', ',', ':', ';', '!', '?', '/', '\\', '|', '+', '=', '*', '&', '%', '$', '#', '@', '^', '~', '`' => true,
        '(', ')', '[', ']', '{', '}', '<', '>', '"', '\'' => true,
        else => false,
    };
}

/// Slugify heading text into output buffer.
///
/// Returns:
///  >=0 bytes written
///  -2  output buffer too small (partial output may be written)
pub fn slugify(text: [*]const u8, len: u32, output: [*]u8, output_cap: u32) i32 {
    if (len == 0) return 0;
    if (output_cap == 0) return -2;

    const in_buf = text[0..len];
    const out_buf = output[0..output_cap];

    var out_idx: u32 = 0;
    var pending_hyphen = false;

    var i: u32 = 0;
    while (i < len) : (i += 1) {
        const b = in_buf[i];

        if (b >= 128) {
            if (pending_hyphen and out_idx > 0) {
                if (out_idx >= output_cap) return -2;
                out_buf[out_idx] = '-';
                out_idx += 1;
                pending_hyphen = false;
            }
            if (out_idx >= output_cap) return -2;
            out_buf[out_idx] = b;
            out_idx += 1;
            continue;
        }

        if (is_ascii_alnum(b)) {
            if (pending_hyphen and out_idx > 0) {
                if (out_idx >= output_cap) return -2;
                out_buf[out_idx] = '-';
                out_idx += 1;
                pending_hyphen = false;
            }
            if (out_idx >= output_cap) return -2;
            out_buf[out_idx] = if (is_ascii_upper(b)) b + 32 else b;
            out_idx += 1;
            continue;
        }

        if (is_hyphen_delim(b)) {
            pending_hyphen = out_idx > 0;
            continue;
        }

        // Strip other ASCII punctuation/symbols entirely.
    }

    // Trim trailing hyphen by never flushing pending_hyphen at end.
    return @intCast(out_idx);
}

// ============================================================================
// Tests
// ============================================================================

test "slugify empty input" {
    var out: [8]u8 = undefined;
    const rc = slugify("".ptr, 0, &out, out.len);
    try std.testing.expectEqual(@as(i32, 0), rc);
}

test "slugify basic heading" {
    var out: [32]u8 = undefined;
    const text = "Hello World";
    const rc = slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, 11), rc);
    try std.testing.expectEqualStrings("hello-world", out[0..@as(usize, @intCast(rc))]);
}

test "slugify collapses delimiters" {
    var out: [32]u8 = undefined;
    const text = "Using `fmt`!!!";
    const rc = slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, 9), rc);
    try std.testing.expectEqualStrings("using-fmt", out[0..@as(usize, @intCast(rc))]);
}

test "slugify preserves unicode bytes" {
    var out: [32]u8 = undefined;
    const text = "Café au lait";
    const rc = slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, 13), rc);
    try std.testing.expectEqualStrings("café-au-lait", out[0..@as(usize, @intCast(rc))]);
}

test "slugify truncation returns -2" {
    var out: [5]u8 = undefined;
    const text = "hello-world";
    const rc = slugify(text.ptr, text.len, &out, out.len);
    try std.testing.expectEqual(@as(i32, -2), rc);
    try std.testing.expectEqualStrings("hello", out[0..]);
}
