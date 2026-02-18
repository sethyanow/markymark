const std = @import("std");

/// Result entry for a single KEY=value pair.
///
/// All offsets are byte positions within the original input buffer.
pub const EnvEntry = extern struct {
    key_offset: u32,
    key_len: u16,
    val_offset: u32,
    val_len: u16,
};

/// SIMD-accelerated .env file key-value extractor.
///
/// Scans `text[0..len]` for KEY=value pairs using SIMD newline detection
/// to find line boundaries, then scalar parsing per line. No heap allocation.
///
/// Handles:
///   - `# comment` lines (skipped)
///   - Empty lines (skipped)
///   - `export KEY=value` prefix (stripped)
///   - Quoted values: `KEY="value"` or `KEY='value'`
///   - Empty values: `KEY=` → val_len = 0
///
/// Multi-line values are not supported (documented limitation).
///
/// Returns number of entries written to `out`.
pub fn scan_env(
    text: [*]const u8,
    len: u32,
    out: [*]EnvEntry,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var line_start: u32 = 0;
    var pos: u32 = 0;

    // SIMD scan: process 16 bytes at a time to locate '\n' boundaries.
    const newline_vec: @Vector(16, u8) = @splat('\n');
    const chunk_size = 16;

    while (pos + chunk_size <= len) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == newline_vec;

        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const nl_pos: u32 = pos + @as(u32, lane);
                if (process_line(buf, line_start, nl_pos, out, cap, &written)) return written;
                line_start = nl_pos + 1;
            }
        }
    }

    // Scalar tail: bytes that don't fill a full SIMD chunk.
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '\n') {
            if (process_line(buf, line_start, pos, out, cap, &written)) return written;
            line_start = pos + 1;
        }
    }

    // Last line with no trailing newline.
    if (line_start < len) {
        _ = process_line(buf, line_start, len, out, cap, &written);
    }

    return written;
}

/// Parse one line and append an EnvEntry if valid.
///
/// Returns true if the output buffer is now full (caller should stop).
fn process_line(
    buf: []const u8,
    raw_start: u32,
    raw_end: u32,
    out: [*]EnvEntry,
    cap: u32,
    written: *u32,
) bool {
    if (written.* >= cap) return true;
    if (raw_start >= raw_end) return false;

    // Strip trailing CR for Windows CRLF line endings.
    const line_end: u32 = if (buf[raw_end - 1] == '\r') raw_end - 1 else raw_end;

    // Skip leading whitespace.
    var p: u32 = raw_start;
    while (p < line_end and (buf[p] == ' ' or buf[p] == '\t')) : (p += 1) {}

    if (p >= line_end) return false; // blank line
    if (buf[p] == '#') return false; // comment

    // Strip optional `export ` prefix (7 characters).
    if (line_end - p >= 7 and
        buf[p] == 'e' and buf[p + 1] == 'x' and buf[p + 2] == 'p' and
        buf[p + 3] == 'o' and buf[p + 4] == 'r' and buf[p + 5] == 't' and
        buf[p + 6] == ' ')
    {
        p += 7;
        while (p < line_end and (buf[p] == ' ' or buf[p] == '\t')) : (p += 1) {}
    }

    // Find '=' delimiter.
    var eq: u32 = p;
    while (eq < line_end and buf[eq] != '=') : (eq += 1) {}
    if (eq >= line_end) return false; // no '=' → not a key-value pair

    // Key: from p to eq, trimming trailing whitespace.
    var key_end: u32 = eq;
    while (key_end > p and (buf[key_end - 1] == ' ' or buf[key_end - 1] == '\t')) : (key_end -= 1) {}
    if (key_end <= p) return false; // empty key

    const key_len: u32 = key_end - p;
    if (key_len > std.math.maxInt(u16)) return false; // key exceeds u16 (> 64 KiB, skip)

    // Value: everything after '='.
    var val_start: u32 = eq + 1;
    var val_end: u32 = line_end;

    // Strip surrounding matched quotes (double or single).
    if (val_end > val_start + 1) {
        const first = buf[val_start];
        const last = buf[val_end - 1];
        if ((first == '"' and last == '"') or (first == '\'' and last == '\'')) {
            val_start += 1;
            val_end -= 1;
        }
    }

    const val_len: u32 = if (val_end > val_start) val_end - val_start else 0;
    if (val_len > std.math.maxInt(u16)) return false; // value exceeds u16 (skip)

    out[written.*] = EnvEntry{
        .key_offset = p,
        .key_len = @intCast(key_len),
        .val_offset = val_start,
        .val_len = @intCast(val_len),
    };
    written.* += 1;
    return written.* >= cap;
}

// ============================================================================
// Tests
// ============================================================================

test "env_basic: standard KEY=value pairs" {
    const text =
        \\FOO=bar
        \\BAZ=qux
    ;
    var out: [4]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    // FOO at offset 0, length 3
    try std.testing.expectEqual(@as(u32, 0), out[0].key_offset);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);
    // bar: offset = 4 (after "FOO="), length 3
    try std.testing.expectEqual(@as(u32, 4), out[0].val_offset);
    try std.testing.expectEqual(@as(u16, 3), out[0].val_len);
    // BAZ
    try std.testing.expectEqual(@as(u16, 3), out[1].key_len);
    // qux length
    try std.testing.expectEqual(@as(u16, 3), out[1].val_len);
}

test "env_empty_value: KEY= with no value yields val_len 0" {
    const text = "KEY=\n";
    var out: [2]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 0), out[0].val_len);
}

test "env_comments: comment lines are skipped" {
    const text =
        \\# This is a comment
        \\KEY=value
        \\# another comment
    ;
    var out: [4]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    // KEY = 3 chars
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);
    // value = 5 chars
    try std.testing.expectEqual(@as(u16, 5), out[0].val_len);
}

test "env_export_prefix: export keyword is stripped from key" {
    const text = "export DATABASE_URL=postgres://localhost/db\n";
    var out: [2]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    // Key should be "DATABASE_URL" (12 chars), not include "export "
    try std.testing.expectEqual(@as(u16, 12), out[0].key_len);
    // Key starts after "export " (7 chars)
    try std.testing.expectEqual(@as(u32, 7), out[0].key_offset);
}

test "env_quoted_double: double-quoted values have quotes stripped" {
    const text = "MESSAGE=\"hello world\"\n";
    var out: [2]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    // "hello world" = 11 chars (quotes excluded)
    try std.testing.expectEqual(@as(u16, 11), out[0].val_len);
}

test "env_quoted_single: single-quoted values have quotes stripped" {
    const text = "MSG='hi there'\n";
    var out: [2]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    // 'hi there' = 8 chars
    try std.testing.expectEqual(@as(u16, 8), out[0].val_len);
}

test "env_empty_input: returns 0 entries" {
    var out: [4]EnvEntry = undefined;
    const w = scan_env("".ptr, 0, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "env_no_equals: lines without = are skipped" {
    const text = "NOTAKVPAIR\nFOO=bar\n";
    var out: [4]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // FOO
}

test "env_blank_lines: blank lines are skipped" {
    const text = "\nFOO=bar\n\nBAZ=qux\n";
    var out: [4]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "env_no_trailing_newline: last line without newline is parsed" {
    const text = "FOO=bar";
    var out: [4]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 3), out[0].val_len);
}

test "env_cap_zero: returns 0 when cap is 0" {
    const text = "FOO=bar\n";
    var out: [1]EnvEntry = undefined;
    const w = scan_env(text.ptr, text.len, &out, 0);
    try std.testing.expectEqual(@as(u32, 0), w);
}
