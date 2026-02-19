const std = @import("std");

/// Entry kind for TomlEntry.
pub const TomlKind = enum(u8) {
    /// A key = value pair in the current table context.
    kv = 0,
    /// A [table] header.
    table_header = 1,
    /// A [[array_table]] header.
    array_table_header = 2,
};

/// Result entry for TOML file scanning.
///
/// For `kind == table_header` or `kind == array_table_header`:
///   - `table_offset`/`table_len` — byte range of the header name (without brackets)
///   - `key_len == 0`, `val_len == 0`
///
/// For `kind == kv`:
///   - `table_offset`/`table_len` — current table context (0/0 for root)
///   - `key_offset`/`key_len`     — dotted key path (e.g. "a.b.c")
///   - `val_offset`/`val_len`     — value text (inline tables, quoted strings kept as-is)
///
/// All offsets are byte positions within the original input buffer.
pub const TomlEntry = extern struct {
    /// TomlKind discriminant.
    kind: u8,
    /// Byte offset of the table/array-table name (without brackets).
    /// For kv entries: the current table context.  Zero for the root table.
    table_offset: u32,
    /// Length of the table/array-table name.  Zero for root-level kv entries.
    table_len: u16,
    /// Byte offset of the key name.  Zero for header entries.
    key_offset: u32,
    /// Length of the key name.  Dotted keys (a.b.c) are reported as-is.  Zero for headers.
    key_len: u16,
    /// Byte offset of the value text.  Zero for header entries.
    val_offset: u32,
    /// Length of the value text.  Zero for header entries or empty values.
    val_len: u16,
};

/// SIMD-accelerated TOML file key-value extractor.
///
/// Scans `text[0..len]` for TOML structure using SIMD newline detection to
/// find line boundaries, then scalar parsing per line.  No heap allocation.
///
/// Emits three kinds of entries (see `TomlKind`):
///   - `table_header`       — `[table.name]` line
///   - `array_table_header` — `[[array.name]]` line
///   - `kv`                 — `key = value` assignment
///
/// Handles:
///   - `# comment` lines (skipped)
///   - Empty and blank lines (skipped)
///   - `key = value` pairs with optional whitespace around `=`
///   - Dotted keys: `a.b.c = "value"` (full path reported verbatim)
///   - Windows CRLF line endings (`\r\n`)
///   - Inline tables: `config = { a = 1 }` (value captured as opaque text, no crash)
///   - Multi-line strings `"""..."""` / `'''...'''` (interior lines skipped)
///
/// Does NOT implement a full TOML parser.  Values are reported as raw text slices.
///
/// Returns number of entries written to `out`.
pub fn scan_toml(
    text: [*]const u8,
    len: u32,
    out: [*]TomlEntry,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var line_start: u32 = 0;
    var pos: u32 = 0;

    var cur_table_offset: u32 = 0;
    var cur_table_len: u16 = 0;
    var in_multiline: bool = false;

    // SIMD scan: process 16 bytes at a time to locate '\n' boundaries.
    const newline_vec: @Vector(16, u8) = @splat('\n');
    const chunk_size = 16;

    while (pos + chunk_size <= len) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == newline_vec;

        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const nl_pos: u32 = pos + @as(u32, lane);
                if (process_line(buf, line_start, nl_pos, out, cap, &written, &cur_table_offset, &cur_table_len, &in_multiline)) return written;
                line_start = nl_pos + 1;
            }
        }
    }

    // Scalar tail: bytes that don't fill a full SIMD chunk.
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '\n') {
            if (process_line(buf, line_start, pos, out, cap, &written, &cur_table_offset, &cur_table_len, &in_multiline)) return written;
            line_start = pos + 1;
        }
    }

    // Last line with no trailing newline.
    if (line_start < len) {
        _ = process_line(buf, line_start, len, out, cap, &written, &cur_table_offset, &cur_table_len, &in_multiline);
    }

    return written;
}

/// Parse one TOML line and append entries or update table state.
///
/// Returns true if the output buffer is now full (caller should stop).
fn process_line(
    buf: []const u8,
    raw_start: u32,
    raw_end: u32,
    out: [*]TomlEntry,
    cap: u32,
    written: *u32,
    cur_table_offset: *u32,
    cur_table_len: *u16,
    in_multiline: *bool,
) bool {
    if (written.* >= cap) return true;
    if (raw_start >= raw_end) return false;

    // Strip trailing CR for Windows CRLF line endings.
    const line_end: u32 = if (buf[raw_end - 1] == '\r') raw_end - 1 else raw_end;

    // --- Multi-line string continuation ---
    // While inside a """ or ''' block, skip lines until the closing delimiter.
    if (in_multiline.*) {
        // Scan this line for the triple-quote that closes the block.
        var i: u32 = raw_start;
        while (i + 2 < line_end) : (i += 1) {
            const c = buf[i];
            if ((c == '"' or c == '\'') and buf[i + 1] == c and buf[i + 2] == c) {
                in_multiline.* = false;
                break;
            }
        }
        return false; // always skip the continuation line itself
    }

    // Skip leading whitespace.
    var p: u32 = raw_start;
    while (p < line_end and (buf[p] == ' ' or buf[p] == '\t')) : (p += 1) {}

    if (p >= line_end) return false; // blank line
    if (buf[p] == '#') return false; // comment line

    // --- Table/array-table header ---
    if (buf[p] == '[') {
        const is_array = (p + 1 < line_end and buf[p + 1] == '[');
        const open: u32 = if (is_array) p + 2 else p + 1;

        // Find the first closing ']'.
        var close: u32 = open;
        while (close < line_end and buf[close] != ']') : (close += 1) {}
        if (close >= line_end) return false; // malformed header — skip

        // Trim interior whitespace from the table name.
        var tname_start: u32 = open;
        while (tname_start < close and (buf[tname_start] == ' ' or buf[tname_start] == '\t')) : (tname_start += 1) {}
        var tname_end: u32 = close;
        while (tname_end > tname_start and (buf[tname_end - 1] == ' ' or buf[tname_end - 1] == '\t')) : (tname_end -= 1) {}

        const tlen = tname_end - tname_start;
        if (tlen > std.math.maxInt(u16)) return false; // name too long — skip

        // Update current table context for subsequent kv entries.
        cur_table_offset.* = tname_start;
        cur_table_len.* = @intCast(tlen);

        const kind: u8 = if (is_array)
            @intFromEnum(TomlKind.array_table_header)
        else
            @intFromEnum(TomlKind.table_header);

        out[written.*] = TomlEntry{
            .kind = kind,
            .table_offset = tname_start,
            .table_len = @intCast(tlen),
            .key_offset = 0,
            .key_len = 0,
            .val_offset = 0,
            .val_len = 0,
        };
        written.* += 1;
        return written.* >= cap;
    }

    // --- Key = value pair ---
    // Find the '=' delimiter.  Stop before '#' (which could be a bare comment).
    var eq: u32 = p;
    while (eq < line_end and buf[eq] != '=' and buf[eq] != '#') : (eq += 1) {}
    if (eq >= line_end or buf[eq] != '=') return false; // no '=' — skip

    // Key: from p to eq, trimming trailing whitespace.
    var key_end: u32 = eq;
    while (key_end > p and (buf[key_end - 1] == ' ' or buf[key_end - 1] == '\t')) : (key_end -= 1) {}
    if (key_end <= p) return false; // empty key

    const key_len_u32 = key_end - p;
    if (key_len_u32 > std.math.maxInt(u16)) return false;

    // Value: everything after '=', leading whitespace stripped.
    var val_start: u32 = eq + 1;
    while (val_start < line_end and (buf[val_start] == ' ' or buf[val_start] == '\t')) : (val_start += 1) {}
    var val_end: u32 = line_end;

    // Detect multi-line string opening: """ or '''.
    // If the closing triple-quote does NOT appear on this line, enter multi-line mode.
    if (val_end >= val_start + 3) {
        const q = buf[val_start];
        if ((q == '"' or q == '\'') and buf[val_start + 1] == q and buf[val_start + 2] == q) {
            // Search for a closing triple-quote after the opening one.
            var scan: u32 = val_start + 3;
            var found_close = false;
            while (scan + 2 <= val_end) : (scan += 1) {
                if (buf[scan] == q and buf[scan + 1] == q and buf[scan + 2] == q) {
                    found_close = true;
                    break;
                }
            }
            if (!found_close) {
                // Opening delimiter found but no close: multi-line string.
                in_multiline.* = true;
                // Capture the opening delimiter as the value text.
                val_end = val_start + 3;
            }
            // If found_close: single-line triple-quoted string — value as-is.
        }
    }

    // For non-multi-line values, strip trailing inline # comments and whitespace.
    if (!in_multiline.*) {
        var vi: u32 = val_start;
        while (vi < val_end) : (vi += 1) {
            if (buf[vi] == '#' and vi > val_start and (buf[vi - 1] == ' ' or buf[vi - 1] == '\t')) {
                val_end = vi;
                break;
            }
        }
        // Strip trailing whitespace from value.
        while (val_end > val_start and (buf[val_end - 1] == ' ' or buf[val_end - 1] == '\t')) : (val_end -= 1) {}
    }

    const val_len_u32: u32 = if (val_end > val_start) val_end - val_start else 0;
    if (val_len_u32 > std.math.maxInt(u16)) return false;

    out[written.*] = TomlEntry{
        .kind = @intFromEnum(TomlKind.kv),
        .table_offset = cur_table_offset.*,
        .table_len = cur_table_len.*,
        .key_offset = p,
        .key_len = @intCast(key_len_u32),
        .val_offset = val_start,
        .val_len = @intCast(val_len_u32),
    };
    written.* += 1;
    return written.* >= cap;
}

// ============================================================================
// Tests
// ============================================================================

test "toml_tables: [table] header emits table_header entry, kv carries table context" {
    // [database]
    // host = "localhost"
    // port = 5432
    const text =
        \\[database]
        \\host = "localhost"
        \\port = 5432
    ;
    var out: [4]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 3), w);

    // First entry: [database] header
    try std.testing.expectEqual(@intFromEnum(TomlKind.table_header), out[0].kind);
    // "database" starts at offset 1 (after '['), length 8
    try std.testing.expectEqual(@as(u32, 1), out[0].table_offset);
    try std.testing.expectEqual(@as(u16, 8), out[0].table_len);
    try std.testing.expectEqual(@as(u16, 0), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 0), out[0].val_len);

    // Second entry: host = "localhost" (kind=kv, inherits table context)
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[1].kind);
    try std.testing.expectEqual(@as(u32, 1), out[1].table_offset);  // same table
    try std.testing.expectEqual(@as(u16, 8), out[1].table_len);
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len);       // "host"

    // Third entry: port = 5432
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[2].kind);
    try std.testing.expectEqual(@as(u16, 4), out[2].key_len);       // "port"
}

test "toml_array_tables: [[array]] header emits array_table_header entry" {
    const text =
        \\[[servers]]
        \\host = "alpha"
        \\[[servers]]
        \\host = "beta"
    ;
    var out: [8]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 8);
    try std.testing.expectEqual(@as(u32, 4), w);

    // First [[servers]] header
    try std.testing.expectEqual(@intFromEnum(TomlKind.array_table_header), out[0].kind);
    try std.testing.expectEqual(@as(u16, 7), out[0].table_len);   // "servers"

    // host = "alpha" kv under first [[servers]]
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[1].kind);
    try std.testing.expectEqual(@as(u16, 7), out[1].table_len);   // table context = "servers"

    // Second [[servers]] header
    try std.testing.expectEqual(@intFromEnum(TomlKind.array_table_header), out[2].kind);

    // host = "beta" kv under second [[servers]]
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[3].kind);
    try std.testing.expectEqual(@as(u16, 4), out[3].key_len);     // "host"
}

test "toml_dotted_keys: dotted key a.b.c reported verbatim" {
    const text = "[config]\na.b.c = \"value\"\n";
    var out: [4]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 4);
    // 1 table header + 1 kv
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[1].kind);
    // "a.b.c" = 5 characters
    try std.testing.expectEqual(@as(u16, 5), out[1].key_len);
}

test "toml_inline_tables: inline { } value does not crash scanner" {
    // Inline tables are treated as opaque value text — no recursive parsing.
    const text = "config = { a = 1, b = 2 }\n";
    var out: [4]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 4);
    // One kv entry: key="config", value="{ a = 1, b = 2 }"
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[0].kind);
    try std.testing.expectEqual(@as(u16, 6), out[0].key_len);  // "config"
    try std.testing.expect(out[0].val_len > 0);
}

test "toml_root_keys: keys before any table header have table_len=0" {
    const text =
        \\title = "My App"
        \\version = "1.0"
        \\[section]
        \\key = "value"
    ;
    var out: [8]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 8);
    try std.testing.expectEqual(@as(u32, 4), w);

    // root-level keys have table_len = 0
    try std.testing.expectEqual(@as(u16, 0), out[0].table_len);
    try std.testing.expectEqual(@as(u16, 0), out[1].table_len);

    // [section] header
    try std.testing.expectEqual(@intFromEnum(TomlKind.table_header), out[2].kind);

    // key under [section] inherits table context
    try std.testing.expect(out[3].table_len > 0);
}

test "toml_comments: # comment lines are skipped" {
    const text =
        \\# this is a comment
        \\key = "value"
        \\# another comment
    ;
    var out: [4]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[0].kind);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);  // "key"
}

test "toml_inline_comment: trailing # comment stripped from value" {
    const text = "key = \"value\" # trailing comment\n";
    var out: [2]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    // value should be "\"value\"" (7 chars with quotes), not include "# trailing comment"
    try std.testing.expect(out[0].val_len < 20);
    try std.testing.expect(out[0].val_len > 0);
}

test "toml_crlf: Windows CRLF line endings handled" {
    const text = "[section]\r\nkey = \"value\"\r\n";
    var out: [4]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@intFromEnum(TomlKind.table_header), out[0].kind);
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[1].kind);
    try std.testing.expectEqual(@as(u16, 3), out[1].key_len);  // "key"
}

test "toml_no_trailing_newline: last line without newline is parsed" {
    const text = "[s]\nkey = val";
    var out: [4]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 3), out[1].key_len);  // "key"
    try std.testing.expectEqual(@as(u16, 3), out[1].val_len);  // "val"
}

test "toml_empty_input: returns 0 entries" {
    var out: [4]TomlEntry = undefined;
    const w = scan_toml("".ptr, 0, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "toml_cap_zero: returns 0 when cap is 0" {
    const text = "[s]\nk = v\n";
    var out: [2]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 0);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "toml_empty_value: key = with no value yields val_len 0" {
    const text = "[s]\nk = \n";
    var out: [4]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 4);
    // table header + kv
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 1), out[1].key_len);  // "k"
    try std.testing.expectEqual(@as(u16, 0), out[1].val_len);
}

test "toml_multiline_string: lines inside triple-quoted block skipped" {
    // The scanner should not emit false kv entries from inside """ blocks.
    const text =
        \\desc = """
        \\key = fake_key_inside_string
        \\another = fake
        \\"""
        \\real_key = "actual"
    ;
    var out: [8]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 8);
    // Only two real entries: desc = """... and real_key = "actual"
    try std.testing.expectEqual(@as(u32, 2), w);
    // First: desc = """ (multi-line open, val truncated to """)
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len);  // "desc"
    // Second: real_key = "actual"
    try std.testing.expectEqual(@as(u16, 8), out[1].key_len);  // "real_key"
}

test "toml_mixed_tables_and_arrays: interleaved [table] and [[array]] entries" {
    const text =
        \\[owner]
        \\name = "Alice"
        \\[[products]]
        \\name = "Widget"
        \\[owner.details]
        \\email = "alice@example.com"
    ;
    var out: [8]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 8);
    try std.testing.expectEqual(@as(u32, 6), w);

    try std.testing.expectEqual(@intFromEnum(TomlKind.table_header), out[0].kind);       // [owner]
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[1].kind);                  // name = "Alice"
    try std.testing.expectEqual(@intFromEnum(TomlKind.array_table_header), out[2].kind); // [[products]]
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[3].kind);                  // name = "Widget"
    try std.testing.expectEqual(@intFromEnum(TomlKind.table_header), out[4].kind);        // [owner.details]
    try std.testing.expectEqual(@intFromEnum(TomlKind.kv), out[5].kind);                  // email = ...
}

test "toml_whitespace_around_eq: spaces trimmed from key and value" {
    const text = "key   =   value\n";
    var out: [2]TomlEntry = undefined;
    const w = scan_toml(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);  // "key"
    try std.testing.expectEqual(@as(u16, 5), out[0].val_len);  // "value"
}
