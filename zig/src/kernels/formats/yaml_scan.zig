const std = @import("std");

/// Result entry for a single YAML key.
///
/// The `indent` field holds the column offset (in spaces, with tabs
/// normalised to 1 space each) of the key on its source line.  Callers
/// can reconstruct the key-path hierarchy by maintaining a stack of
/// `(indent, key)` pairs as they iterate the entries in order.
///
/// All offsets are byte positions within the original input buffer.
pub const YamlEntry = extern struct {
    /// Byte offset of the key name (text before ':') in the input buffer.
    key_offset: u32,
    /// Length of the key name in bytes.
    key_len: u16,
    /// Indentation depth: count of leading spaces on the source line
    /// (tabs normalised to 1 space each).  Zero = top-level key.
    indent: u16,
};

/// SIMD-accelerated YAML key extractor.
///
/// Scans `text[0..len]` for mapping keys using SIMD newline detection to
/// locate line boundaries, then scalar parsing per line.  No heap allocation.
///
/// Handles:
///   - Simple `key: value` pairs and bare `key:` (null value) at any indent
///   - Nested keys via indentation hierarchy
///   - Both spaces and tabs for indentation (tabs normalised to 1 space each)
///   - `# comment` lines (skipped)
///   - YAML document markers (`---`, `...`) (skipped)
///   - YAML directives (`%YAML`, `%TAG`) (skipped)
///   - Block scalars (`key: |` and `key: >`) — interior lines skipped until
///     indentation drops back to the key's indentation level or below
///   - List item prefixes (`- `): skipped to expose mapping keys within lists
///   - Windows CRLF line endings (`\r\n`)
///
/// Does NOT implement a full YAML parser.  Anchors, aliases, and flow-style
/// inline JSON syntax are skipped or treated as opaque values.
///
/// Returns number of entries written to `out`.
pub fn scan_yaml_keys(
    text: [*]const u8,
    len: u32,
    out: [*]YamlEntry,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var line_start: u32 = 0;
    var pos: u32 = 0;

    // Block-scalar state.  When active, lines whose indentation exceeds
    // `block_base_indent` are skipped (they are scalar content, not keys).
    var in_block_scalar: bool = false;
    var block_base_indent: u32 = 0;

    // SIMD scan: process 16 bytes at a time to locate '\n' boundaries.
    const newline_vec: @Vector(16, u8) = @splat('\n');
    const chunk_size = 16;

    while (pos + chunk_size <= len) : (pos += chunk_size) {
        const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
        const matches = chunk == newline_vec;

        inline for (0..chunk_size) |lane| {
            if (matches[lane]) {
                const nl_pos: u32 = pos + @as(u32, lane);
                if (process_line(buf, line_start, nl_pos, out, cap, &written, &in_block_scalar, &block_base_indent)) return written;
                line_start = nl_pos + 1;
            }
        }
    }

    // Scalar tail: bytes that don't fill a full SIMD chunk.
    while (pos < len) : (pos += 1) {
        if (buf[pos] == '\n') {
            if (process_line(buf, line_start, pos, out, cap, &written, &in_block_scalar, &block_base_indent)) return written;
            line_start = pos + 1;
        }
    }

    // Last line with no trailing newline.
    if (line_start < len) {
        _ = process_line(buf, line_start, len, out, cap, &written, &in_block_scalar, &block_base_indent);
    }

    return written;
}

/// Parse one YAML line and append a YamlEntry if it contains a mapping key.
///
/// Returns true if the output buffer is now full (caller should stop).
fn process_line(
    buf: []const u8,
    raw_start: u32,
    raw_end: u32,
    out: [*]YamlEntry,
    cap: u32,
    written: *u32,
    in_block_scalar: *bool,
    block_base_indent: *u32,
) bool {
    if (written.* >= cap) return true;
    if (raw_start >= raw_end) return false;

    // Strip trailing CR for Windows CRLF line endings.
    const line_end: u32 = if (buf[raw_end - 1] == '\r') raw_end - 1 else raw_end;

    // Measure leading indentation (spaces and tabs; tabs count as 1 space).
    var p: u32 = raw_start;
    while (p < line_end and (buf[p] == ' ' or buf[p] == '\t')) : (p += 1) {}

    var indent: u32 = p - raw_start;

    // Blank lines do not exit block-scalar mode (they are scalar content).
    if (p >= line_end) return false;

    // Block-scalar continuation: if the current line's indent is greater
    // than the key indent that started the block, skip it (scalar content).
    // When indent falls back to key indent or less, the block ends.
    if (in_block_scalar.*) {
        if (indent > block_base_indent.*) {
            return false; // interior block-scalar line — skip
        }
        // Indent dropped back: block scalar is over, process this line.
        in_block_scalar.* = false;
    }

    // Skip YAML comment lines.
    if (buf[p] == '#') return false;

    // Skip document markers (--- and ...).
    if (line_end - p >= 3 and
        buf[p] == '-' and buf[p + 1] == '-' and buf[p + 2] == '-') return false;
    if (line_end - p >= 3 and
        buf[p] == '.' and buf[p + 1] == '.' and buf[p + 2] == '.') return false;

    // Skip YAML directives (%YAML 1.2, %TAG ...).
    if (buf[p] == '%') return false;

    // Strip list-item prefix "- " to expose mapping keys within sequences.
    // Recompute indent to the key column (after "- " and any extra whitespace),
    // not the dash column, so that hierarchy and block-scalar detection are correct.
    if (buf[p] == '-' and p + 1 < line_end and buf[p + 1] == ' ') {
        const list_prefix_start = p;
        p += 2;
        // Skip any additional whitespace after "- ".
        while (p < line_end and (buf[p] == ' ' or buf[p] == '\t')) : (p += 1) {}
        indent += p - list_prefix_start;
    }

    if (p >= line_end) return false;

    // Skip lines starting with anchor (&) or alias (*) sigils.
    // These are not mapping keys in the key-extraction sense.
    if (buf[p] == '&' or buf[p] == '*') return false;

    // Skip quoted keys (flow-style or complex keys are treated as opaque).
    if (buf[p] == '"' or buf[p] == '\'') return false;

    // Skip flow-style mappings and sequences: { ... } and [ ... ].
    if (buf[p] == '{' or buf[p] == '[') return false;

    // Find the ':' key-value separator.
    // We require ':' to be followed by ' ', '\t', '\r', or end-of-line to
    // distinguish mapping keys from bare colons in values (e.g. URLs).
    var colon: u32 = p;
    while (colon < line_end and buf[colon] != ':') : (colon += 1) {}

    if (colon >= line_end) return false; // no ':' — not a mapping key line

    // Reject URL-like patterns: colon followed by a non-whitespace character
    // that is not end-of-line (e.g. "http://example.com").
    if (colon + 1 < line_end) {
        const after = buf[colon + 1];
        if (after != ' ' and after != '\t' and after != '\r') return false;
    }

    // Extract key: text from p to colon, trailing whitespace stripped.
    var key_end: u32 = colon;
    while (key_end > p and (buf[key_end - 1] == ' ' or buf[key_end - 1] == '\t')) : (key_end -= 1) {}

    if (key_end <= p) return false; // empty key
    const key_len: u32 = key_end - p;
    if (key_len > std.math.maxInt(u16)) return false; // key > 64 KiB — skip
    if (indent > std.math.maxInt(u16)) return false; // absurd indent — skip

    // Detect block-scalar opening: value starts with '|' or '>'.
    // If found, subsequent lines will be treated as scalar content until
    // indentation drops back to this key's level.
    var val_start: u32 = colon + 1;
    while (val_start < line_end and (buf[val_start] == ' ' or buf[val_start] == '\t')) : (val_start += 1) {}

    if (val_start < line_end) {
        const first = buf[val_start];
        if (first == '|' or first == '>') {
            // Verify it is a true block-scalar indicator: after the '|'/'>'
            // and optional chomping/indentation modifiers, only whitespace
            // or a comment should remain on this line.
            var scan: u32 = val_start + 1;
            // Optional chomping indicator (+/-) or explicit indentation digit.
            if (scan < line_end and
                (buf[scan] == '+' or buf[scan] == '-' or
                    (buf[scan] >= '1' and buf[scan] <= '9')))
            {
                scan += 1;
            }
            // Skip trailing whitespace.
            while (scan < line_end and (buf[scan] == ' ' or buf[scan] == '\t')) : (scan += 1) {}
            // Remainder must be end-of-line or an inline comment.
            if (scan >= line_end or buf[scan] == '#') {
                in_block_scalar.* = true;
                block_base_indent.* = indent;
            }
        }
    }

    out[written.*] = YamlEntry{
        .key_offset = p,
        .key_len = @intCast(key_len),
        .indent = @intCast(indent),
    };
    written.* += 1;
    return written.* >= cap;
}

// ============================================================================
// Tests
// ============================================================================

test "yaml_basic_keys: simple key: value pairs are extracted" {
    const text =
        \\host: localhost
        \\port: 5432
    ;
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);

    // "host" at offset 0, length 4, indent 0.
    try std.testing.expectEqual(@as(u32, 0), out[0].key_offset);
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 0), out[0].indent);

    // "port" at indent 0, length 4.
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len);
    try std.testing.expectEqual(@as(u16, 0), out[1].indent);
}

test "yaml_indentation: hierarchy tracked via indent field" {
    // root:
    //   child: 1
    //   sibling: 2
    const text =
        \\root:
        \\  child: 1
        \\  sibling: 2
    ;
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 3), w);

    // "root" at indent 0.
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len); // "root"
    try std.testing.expectEqual(@as(u16, 0), out[0].indent);

    // "child" at indent 2.
    try std.testing.expectEqual(@as(u16, 5), out[1].key_len); // "child"
    try std.testing.expectEqual(@as(u16, 2), out[1].indent);

    // "sibling" at indent 2.
    try std.testing.expectEqual(@as(u16, 7), out[2].key_len); // "sibling"
    try std.testing.expectEqual(@as(u16, 2), out[2].indent);
}

test "yaml_mixed_indent: tabs in indentation do not crash the scanner" {
    // Tabs are normalised to 1 space each; no undefined behaviour.
    const text = "\tkey: value\n\t\tnested: val\n";
    var out: [4]YamlEntry = undefined;
    // Must not crash; we just check it returns without panic.
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    // Both lines have keys, so at least 1 entry should be emitted.
    try std.testing.expect(w >= 1);
    // Tab-indented keys have indent >= 1.
    try std.testing.expect(out[0].indent >= 1);
}

test "yaml_comments: comment lines are skipped" {
    const text =
        \\# This is a comment
        \\key: value
        \\# another comment
    ;
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "key"
    try std.testing.expectEqual(@as(u16, 0), out[0].indent);
}

test "yaml_multiline: block scalar content is not scanned for keys" {
    // desc: |
    //   key = fake_key_inside_string  ← must not be extracted
    //   another = fake
    // real_key: actual
    const text =
        \\desc: |
        \\  key: fake_inside
        \\  another: fake
        \\real_key: actual
    ;
    var out: [8]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 8);
    // Only "desc" and "real_key" should be extracted, not the interior lines.
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len); // "desc"
    try std.testing.expectEqual(@as(u16, 8), out[1].key_len); // "real_key"
}

test "yaml_folded_scalar: folded block scalar (>) content is not scanned" {
    const text =
        \\summary: >
        \\  This is folded
        \\  content here
        \\title: My Doc
    ;
    var out: [8]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 8);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 7), out[0].key_len); // "summary"
    try std.testing.expectEqual(@as(u16, 5), out[1].key_len); // "title"
}

test "yaml_list_items: mapping keys within sequence items are extracted" {
    // - host: a
    // - host: b
    const text =
        \\- host: a
        \\- host: b
    ;
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    // Both keys are "host" (4 chars).
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len);
}

test "yaml_null_value_key: bare 'key:' with no value is extracted" {
    const text = "key:\nnext: value\n";
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "key"
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len); // "next"
}

test "yaml_url_values: lines with URLs are not treated as keys" {
    // URL values contain '://' which must not split at the wrong colon.
    const text = "homepage: https://example.com\nname: test\n";
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 8), out[0].key_len); // "homepage"
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len); // "name"
}

test "yaml_document_markers: --- and ... lines are skipped" {
    const text = "---\nkey: value\n...\n";
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "key"
}

test "yaml_empty_input: returns 0 entries" {
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys("".ptr, 0, &out, 4);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "yaml_cap_zero: returns 0 when cap is 0" {
    const text = "key: value\n";
    var out: [1]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 0);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "yaml_no_trailing_newline: last line without newline is parsed" {
    const text = "key: value";
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "key"
}

test "yaml_crlf: windows CRLF line endings handled" {
    const text = "key: value\r\nnext: val\r\n";
    var out: [4]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 4);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "key"
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len); // "next"
}

test "yaml_nested_block_scalar: block scalar inside nested key" {
    //   inner: |
    //     content
    //   sibling: value
    const text =
        \\outer:
        \\  inner: |
        \\    content
        \\  sibling: value
    ;
    var out: [8]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 8);
    // outer, inner, sibling — not "content"
    try std.testing.expectEqual(@as(u32, 3), w);
    try std.testing.expectEqual(@as(u16, 5), out[0].key_len); // "outer"
    try std.testing.expectEqual(@as(u16, 5), out[1].key_len); // "inner"
    try std.testing.expectEqual(@as(u16, 7), out[2].key_len); // "sibling"
}

test "yaml_cap_enforcement: stops writing when cap is reached" {
    const text = "a: 1\nb: 2\nc: 3\n";
    var out: [2]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 2);
    // Only 2 entries should be written.
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 1), out[0].key_len); // "a"
    try std.testing.expectEqual(@as(u16, 1), out[1].key_len); // "b"
}

test "yaml_list_item_indent: indent reflects key column not dash column" {
    // "  - host: a" — 2 spaces before '-', so key "host" is at column 4 (after "- ").
    const text = "  - host: a\n";
    var out: [2]YamlEntry = undefined;
    const w = scan_yaml_keys(text.ptr, text.len, &out, 2);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len); // "host"
    // Indent must be 4 (key column), not 2 (dash column).
    try std.testing.expectEqual(@as(u16, 4), out[0].indent);
}
