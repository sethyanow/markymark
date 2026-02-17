const std = @import("std");

/// Byte range of a fenced code block.
/// All bytes in [start, end) are inside the fenced region (including fence lines).
/// Matches C ABI layout for FFI — 8 bytes total, no padding.
pub const FenceRange = extern struct {
    /// Byte offset where fenced region starts (first char of opening fence line)
    start: u32,
    /// Byte offset past the end of fenced region (byte after closing fence line newline, or EOF)
    end: u32,
};

/// Info about a detected opening fence.
const FenceOpening = struct {
    char: u8,
    count: u32,
};

/// Scalar (byte-by-byte) fence map builder. Serves as the correctness reference
/// for verifying the SIMD implementation.
///
/// Scans `text[0..len]` for fenced code blocks (triple+ backtick or tilde at
/// column 0). Writes ranges into `out[0..cap]`. Returns the number of ranges
/// found. If return value == cap, there may be more ranges (buffer full).
pub fn build_fence_map_scalar(
    text: [*]const u8,
    len: u32,
    out: [*]FenceRange,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var count: u32 = 0;
    var pos: u32 = 0;

    // Fence state
    var fence_open = false;
    var fence_char: u8 = 0;
    var fence_count: u32 = 0;
    var fence_start: u32 = 0;

    while (pos < len) {
        const line_start = pos;
        const ch = buf[pos];

        if (ch == '`' or ch == '~') {
            // Count consecutive fence characters
            var ch_count: u32 = 0;
            var scan = pos;
            while (scan < len and buf[scan] == ch) {
                ch_count += 1;
                scan += 1;
            }

            if (ch_count >= 3) {
                if (!fence_open) {
                    // Potential opening fence
                    if (is_valid_opening(buf, ch, scan, len)) {
                        fence_open = true;
                        fence_char = ch;
                        fence_count = ch_count;
                        fence_start = line_start;
                        pos = skip_to_next_line(buf, pos, len);
                        continue;
                    }
                } else if (ch == fence_char and ch_count >= fence_count) {
                    // Potential closing fence — rest of line must be only whitespace
                    if (is_only_whitespace_after(buf, scan, len)) {
                        const line_end = skip_to_next_line(buf, pos, len);
                        out[count] = FenceRange{
                            .start = fence_start,
                            .end = line_end,
                        };
                        count += 1;
                        fence_open = false;
                        pos = line_end;
                        if (count >= cap) return count;
                        continue;
                    }
                }
            }
        }

        pos = skip_to_next_line(buf, pos, len);
    }

    // Unclosed fence at EOF
    if (fence_open and count < cap) {
        out[count] = FenceRange{
            .start = fence_start,
            .end = len,
        };
        count += 1;
    }

    return count;
}

/// Check if an opening fence is valid.
/// For backtick fences, the info string (rest of line) must not contain backticks.
/// For tilde fences, any info string is allowed.
pub fn is_valid_opening(buf: []const u8, ch: u8, after_fence: u32, len: u32) bool {
    if (ch == '`') {
        // Backtick fence: info string must not contain backticks (CommonMark rule)
        var p = after_fence;
        while (p < len and buf[p] != '\n') {
            if (buf[p] == '`') return false;
            p += 1;
        }
    }
    return true;
}

/// Check if only whitespace remains on this line from `pos` to the next newline.
pub fn is_only_whitespace_after(buf: []const u8, pos: u32, len: u32) bool {
    var p = pos;
    while (p < len and buf[p] != '\n') {
        if (buf[p] != ' ' and buf[p] != '\t') return false;
        p += 1;
    }
    return true;
}

/// Advance past the current line, returning the position after the '\n'.
/// If no newline is found, returns len (EOF).
pub fn skip_to_next_line(buf: []const u8, pos: u32, len: u32) u32 {
    var p = pos;
    while (p < len and buf[p] != '\n') p += 1;
    if (p < len) p += 1; // skip '\n'
    return p;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "scalar: empty input" {
    const text = "";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, 0, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "scalar: no fences" {
    const text = "# Heading\nSome text\nMore text\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "scalar: single fence pair" {
    const text = "```\ncode here\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "scalar: unclosed fence" {
    const text = "```\ncode here\nmore code\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "scalar: multiple fences" {
    const text = "```\nblock1\n```\ntext\n```\nblock2\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 2), n);
    // First block
    try testing.expectEqual(@as(u32, 0), out[0].start);
    // Second block
    try testing.expect(out[1].start > out[0].end);
}

test "scalar: tilde fences" {
    const text = "~~~\ntilde code\n~~~\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "scalar: mixed fence types cannot close each other" {
    const text = "```\ncode\n~~~\nstill code\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    // The ~~~ does NOT close the backtick fence; ``` does
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "scalar: inline code not a fence" {
    const text = "Use `code` inline\nAnd `more` here\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "scalar: fence with language specifier" {
    const text = "```python\ndef foo():\n    pass\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "scalar: four backtick fence" {
    const text = "````\ncode with ``` inside\n````\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    // Three backticks inside don't close the four-backtick fence
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "scalar: four backtick fence not closed by three" {
    const text = "````\ncode\n```\nstill code\n````\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    // ``` doesn't close ```` — need at least 4
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "scalar: indented backticks not a fence" {
    const text = "    ```\nnot fenced\n    ```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "scalar: adjacent empty fences" {
    const text = "```\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}

test "scalar: buffer overflow" {
    const text = "```\na\n```\n```\nb\n```\n```\nc\n```\n";
    var out: [1]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 1);
    try testing.expectEqual(@as(u32, 1), n);
}

test "scalar: fence with trailing spaces" {
    const text = "```   \ncode\n```\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
}

test "scalar: closing fence with trailing spaces" {
    const text = "```\ncode\n```   \n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
}

test "scalar: backtick fence info string with backtick is not opening" {
    // CommonMark: backtick fence info string cannot contain backticks
    const text = "``` foo`bar\nnot fenced\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "scalar: tilde fence info string with backtick is valid" {
    const text = "~~~ foo`bar\nfenced content\n~~~\n";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
}

test "scalar: fence at EOF without newline" {
    const text = "```\ncode";
    var out: [4]FenceRange = undefined;
    const n = build_fence_map_scalar(text.ptr, text.len, &out, 4);
    try testing.expectEqual(@as(u32, 1), n);
    try testing.expectEqual(@as(u32, 0), out[0].start);
    try testing.expectEqual(@as(u32, text.len), out[0].end);
}
