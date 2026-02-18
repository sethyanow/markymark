const std = @import("std");

/// Result entry for a single key=value pair in an INI file.
///
/// Each entry carries the section it belongs to (embedded per-entry so callers
/// need no separate section table).  For keys that appear before any [section]
/// header (the implicit global section), `section_len` is 0 and
/// `section_offset` is 0.
///
/// All offsets are byte positions within the original input buffer.
pub const IniEntry = extern struct {
    /// Byte offset of the section name content (the text inside [], without brackets).
    /// Zero when the key belongs to the global section.
    section_offset: u32,
    /// Length of section name.  Zero for the global section.
    section_len: u16,
    /// Byte offset of the key name.
    key_offset: u32,
    /// Length of the key name.
    key_len: u16,
    /// Byte offset of the value (leading whitespace and quotes stripped).
    val_offset: u32,
    /// Length of the value (inline comments and trailing whitespace stripped).
    val_len: u16,
};

/// SIMD-accelerated INI file key-value extractor.
///
/// Scans `text[0..len]` for key=value pairs using SIMD newline detection to
/// find line boundaries, then scalar parsing per line.  No heap allocation.
///
/// Handles:
///   - `[section]` headers (section tracked, embedded in subsequent entries)
///   - `# comment` and `; comment` lines (skipped)
///   - `key=value` pairs with optional whitespace around `=`
///   - Global keys before the first `[section]` header
///   - Duplicate sections: both occurrences are tracked independently
///   - Inline comments: `key=value ; trailing` → value stops before `;`
///   - Windows CRLF line endings (`\r\n`)
///
/// Returns number of entries written to `out`.
pub fn scan_ini(
    text: [*]const u8,
    len: u32,
    out: [*]IniEntry,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var line_start: u32 = 0;
    var pos: u32 = 0;

    // Current section: offset of section name inside [] in buf (0 = global).
    var cur_section_offset: u32 = 0;
    var cur_section_len: u16 = 0;

    // SIMD scan: process 16 bytes at a time to locate '\n' boundaries.
    const newline_vec: @Vector(16, u8) = @splat('\n');
    const chunk_size = 16;

    while (pos + chunk_size <= len) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == newline_vec;

        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const nl_pos: u32 = pos + @as(u32, lane);
                if (process_line(buf, line_start, nl_pos, out, cap, &written, &cur_section_offset, &cur_section_len)) return written;
                line_start = nl_pos + 1;
            }
        }
    }

    // Scalar tail: bytes that don't fill a full SIMD chunk.
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '\n') {
            if (process_line(buf, line_start, pos, out, cap, &written, &cur_section_offset, &cur_section_len)) return written;
            line_start = pos + 1;
        }
    }

    // Last line with no trailing newline.
    if (line_start < len) {
        _ = process_line(buf, line_start, len, out, cap, &written, &cur_section_offset, &cur_section_len);
    }

    return written;
}

/// Parse one line and update section state or append an IniEntry if valid.
///
/// Returns true if the output buffer is now full (caller should stop).
fn process_line(
    buf: []const u8,
    raw_start: u32,
    raw_end: u32,
    out: [*]IniEntry,
    cap: u32,
    written: *u32,
    cur_section_offset: *u32,
    cur_section_len: *u16,
) bool {
    if (written.* >= cap) return true;
    if (raw_start >= raw_end) return false;

    // Strip trailing CR for Windows CRLF line endings.
    const line_end: u32 = if (buf[raw_end - 1] == '\r') raw_end - 1 else raw_end;

    // Skip leading whitespace.
    var p: u32 = raw_start;
    while (p < line_end and (buf[p] == ' ' or buf[p] == '\t')) : (p += 1) {}

    if (p >= line_end) return false; // blank line
    if (buf[p] == ';' or buf[p] == '#') return false; // comment line

    // Section header: [section name]
    if (buf[p] == '[') {
        const bracket_open = p + 1;
        // Find the closing ']'.
        var bracket_close = bracket_open;
        while (bracket_close < line_end and buf[bracket_close] != ']') : (bracket_close += 1) {}
        if (bracket_close >= line_end) return false; // malformed header — skip

        // Trim whitespace from section name interior.
        var sname_start = bracket_open;
        while (sname_start < bracket_close and (buf[sname_start] == ' ' or buf[sname_start] == '\t')) : (sname_start += 1) {}
        var sname_end = bracket_close;
        while (sname_end > sname_start and (buf[sname_end - 1] == ' ' or buf[sname_end - 1] == '\t')) : (sname_end -= 1) {}

        const slen = sname_end - sname_start;
        if (slen > std.math.maxInt(u16)) return false; // section name too long — skip

        cur_section_offset.* = sname_start;
        cur_section_len.* = @intCast(slen);
        return false; // section headers do not emit entries
    }

    // Key=value pair: find the first '='.
    var eq: u32 = p;
    while (eq < line_end and buf[eq] != '=') : (eq += 1) {}
    if (eq >= line_end) return false; // no '=' → not a key=value line

    // Key: from p to eq, trimming trailing whitespace.
    var key_end: u32 = eq;
    while (key_end > p and (buf[key_end - 1] == ' ' or buf[key_end - 1] == '\t')) : (key_end -= 1) {}
    if (key_end <= p) return false; // empty key

    const key_len_u32 = key_end - p;
    if (key_len_u32 > std.math.maxInt(u16)) return false;

    // Value: everything after '='.
    var val_start: u32 = eq + 1;
    // Skip leading whitespace in value.
    while (val_start < line_end and (buf[val_start] == ' ' or buf[val_start] == '\t')) : (val_start += 1) {}
    var val_end: u32 = line_end;

    // Strip inline comments: `;` or `#` preceded by whitespace.
    var vi: u32 = val_start;
    while (vi < val_end) : (vi += 1) {
        if ((buf[vi] == ';' or buf[vi] == '#') and
            vi > val_start and
            (buf[vi - 1] == ' ' or buf[vi - 1] == '\t'))
        {
            val_end = vi;
            break;
        }
    }

    // Strip trailing whitespace from value.
    while (val_end > val_start and (buf[val_end - 1] == ' ' or buf[val_end - 1] == '\t')) : (val_end -= 1) {}

    const val_len_u32: u32 = if (val_end > val_start) val_end - val_start else 0;
    if (val_len_u32 > std.math.maxInt(u16)) return false;

    out[written.*] = IniEntry{
        .section_offset = cur_section_offset.*,
        .section_len = cur_section_len.*,
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

test "ini_sections: section headers update context for subsequent keys" {
    // [database]
    // host=localhost
    // port=5432
    const text =
        \\[database]
        \\host=localhost
        \\port=5432
    ;
    var out: [4]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);

    // Section name "database" starts at offset 1 (after '['), length 8.
    try std.testing.expectEqual(@as(u32, 1), out[0].section_offset);
    try std.testing.expectEqual(@as(u16, 8), out[0].section_len);
    try std.testing.expectEqual(@as(u32, 1), out[1].section_offset);
    try std.testing.expectEqual(@as(u16, 8), out[1].section_len);

    // host key: offset 11 (after "[database]\n" = 11 chars), length 4
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len);
    // localhost: length 9
    try std.testing.expectEqual(@as(u16, 9), out[0].val_len);

    // port key: length 4
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len);
    // 5432: length 4
    try std.testing.expectEqual(@as(u16, 4), out[1].val_len);
}

test "ini_keys_under_section: keys carry correct section context" {
    const text =
        \\[alpha]
        \\a=1
        \\[beta]
        \\b=2
    ;
    var out: [4]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);

    // First entry belongs to [alpha], second to [beta].
    // [alpha] → section name "alpha" at offset 1, len 5.
    try std.testing.expectEqual(@as(u16, 5), out[0].section_len);
    // [beta] → section name "beta", len 4.
    try std.testing.expectEqual(@as(u16, 4), out[1].section_len);

    // Verify they differ in section_offset (different sections).
    try std.testing.expect(out[0].section_offset != out[1].section_offset);
}

test "ini_comments: semicolon and hash comment lines are skipped" {
    const text =
        \\; this is a comment
        \\# also a comment
        \\[section]
        \\; inline comment line — skipped
        \\key=value
    ;
    var out: [4]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 4);
    // Only one key=value pair should be emitted.
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "key"
    try std.testing.expectEqual(@as(u16, 5), out[0].val_len); // "value"
}

test "ini_global_keys: keys before first section have section_len=0" {
    const text =
        \\global_key=global_value
        \\[section]
        \\section_key=section_value
    ;
    var out: [4]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);

    // First entry is global (before any [section]).
    try std.testing.expectEqual(@as(u16, 0), out[0].section_len);
    try std.testing.expectEqual(@as(u32, 0), out[0].section_offset);

    // Second entry belongs to [section].
    try std.testing.expect(out[1].section_len > 0);
}

test "ini_inline_comment: trailing inline comment stripped from value" {
    const text = "key=value ; trailing comment\n";
    var out: [2]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    // Value should be "value" (5), not include "; trailing comment".
    try std.testing.expectEqual(@as(u16, 5), out[0].val_len);
}

test "ini_inline_comment_hash: trailing hash comment stripped from value" {
    const text = "key=value # trailing\n";
    var out: [2]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 5), out[0].val_len);
}

test "ini_duplicate_sections: both occurrences kept" {
    const text =
        \\[server]
        \\host=a
        \\[server]
        \\host=b
    ;
    var out: [4]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    // Both entries have section_len > 0 (both belong to [server]).
    try std.testing.expect(out[0].section_len > 0);
    try std.testing.expect(out[1].section_len > 0);
    // Values differ: "a" (len 1) and "b" (len 1).
    try std.testing.expectEqual(@as(u16, 1), out[0].val_len);
    try std.testing.expectEqual(@as(u16, 1), out[1].val_len);
}

test "ini_empty_value: key= with no value yields val_len 0" {
    const text = "[s]\nk=\n";
    var out: [2]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 1), out[0].key_len); // "k"
    try std.testing.expectEqual(@as(u16, 0), out[0].val_len);
}

test "ini_whitespace_around_equals: spaces trimmed from key and value" {
    const text = "key = value\n";
    var out: [2]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "key"
    try std.testing.expectEqual(@as(u16, 5), out[0].val_len); // "value"
}

test "ini_empty_input: returns 0 entries" {
    var out: [4]IniEntry = undefined;
    const w = scan_ini("".ptr, 0, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "ini_cap_zero: returns 0 when cap is 0" {
    const text = "[s]\nk=v\n";
    var out: [1]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 0);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "ini_no_trailing_newline: last line without newline is parsed" {
    const text = "[s]\nkey=val";
    var out: [2]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 3), out[0].val_len);
}

test "ini_crlf: windows CRLF line endings handled" {
    const text = "[section]\r\nkey=value\r\n";
    var out: [2]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 5), out[0].val_len);
}

test "ini_no_section_no_equals: bare text lines without = are skipped" {
    const text = "notakv\nkey=val\n";
    var out: [4]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len);
}

test "ini_section_whitespace: section name interior whitespace trimmed" {
    const text = "[ my section ]\nk=v\n";
    var out: [2]IniEntry = undefined;
    const w = scan_ini(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    // "my section" = 10 chars
    try std.testing.expectEqual(@as(u16, 10), out[0].section_len);
}
