const std = @import("std");

/// Maximum nesting depth tracked by the scanner.
///
/// Objects or arrays nested deeper than this will be skipped, and the
/// extraction result will be flagged as depth-exceeded (C ABI returns -2).
pub const MAX_DEPTH: u32 = 100;

/// Result entry for a single JSON object key.
///
/// Callers can reconstruct dot-separated key paths by maintaining a stack
/// of `(depth, key)` pairs as they iterate the entries in order.  The
/// `depth` field gives the 0-indexed nesting level: 0 = key in the
/// outermost object, 1 = key one level deep, etc.
///
/// Note: arrays contribute to depth.  A key inside `{"arr": [{"k": 1}]}`
/// has depth 2 (inside object → array → object).
///
/// All offsets are byte positions within the original input buffer
/// (excluding the surrounding double-quote characters).
pub const JsonKeyEntry = extern struct {
    /// Byte offset of the key content (after the opening `"`) in the buffer.
    key_offset: u32,
    /// Length of the key content in bytes (not including quotes).
    key_len: u16,
    /// Nesting depth: 0 = top-level object key.
    depth: u16,
};

/// Container kind for the depth stack.
const Container = enum(u8) { object, array };

/// SIMD-accelerated JSON key extractor.
///
/// Scans `text[0..len]` for JSON object keys using SIMD string content
/// skipping and character-level structural tracking.  No heap allocation.
///
/// Handles:
///   - Flat and deeply nested objects
///   - Objects nested inside arrays (array index contributes to depth)
///   - Escaped characters inside strings, including `\"`
///   - Unicode escapes (`\uXXXX`) — skipped correctly
///   - Nesting depth limit of MAX_DEPTH (sets `depth_exceeded`)
///
/// Does NOT implement a full JSON parser.  Malformed JSON may produce
/// unexpected but safe results (no UB, no allocation).
///
/// Parameters:
///   `depth_exceeded` — set to true if any block was skipped due to depth limit.
///
/// Returns the number of entries written to `out`.
pub fn scan_json_keys(
    text: [*]const u8,
    len: u32,
    out: [*]JsonKeyEntry,
    cap: u32,
    depth_exceeded: *bool,
) u32 {
    depth_exceeded.* = false;

    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var pos: u32 = 0;
    var written: u32 = 0;

    // Container kind stack (bounded to MAX_DEPTH).
    // stack_top is the number of containers currently entered.
    var stack: [MAX_DEPTH + 1]Container = undefined;
    var stack_top: u32 = 0;

    while (pos < len) {
        switch (buf[pos]) {
            '{' => {
                if (stack_top >= MAX_DEPTH) {
                    // Depth limit: skip the entire balanced block.
                    depth_exceeded.* = true;
                    pos = skip_balanced(buf, pos, len);
                    continue;
                }
                stack[stack_top] = .object;
                stack_top += 1;
                pos += 1;
            },
            '[' => {
                if (stack_top >= MAX_DEPTH) {
                    depth_exceeded.* = true;
                    pos = skip_balanced(buf, pos, len);
                    continue;
                }
                stack[stack_top] = .array;
                stack_top += 1;
                pos += 1;
            },
            '}', ']' => {
                if (stack_top > 0) stack_top -= 1;
                pos += 1;
            },
            '"' => {
                // Scan the string content (SIMD-accelerated).
                pos += 1; // skip opening '"'
                const str_start = pos;

                pos = scan_string_end(buf, pos, len);
                const str_end = pos;
                // Skip closing '"' if present.
                if (pos < len and buf[pos] == '"') pos += 1;

                // Only consider this string as a key if we are directly
                // inside an object (not an array or at top level).
                if (stack_top > 0 and stack[stack_top - 1] == .object) {
                    // Skip whitespace between the string and a potential ':'.
                    var tmp = pos;
                    while (tmp < len and is_ws(buf[tmp])) tmp += 1;

                    if (tmp < len and buf[tmp] == ':') {
                        // This string is an object key.
                        const key_len: u32 = str_end - str_start;
                        const depth: u32 = stack_top - 1;

                        if (written < cap and
                            key_len <= std.math.maxInt(u16) and
                            depth <= std.math.maxInt(u16))
                        {
                            out[written] = JsonKeyEntry{
                                .key_offset = str_start,
                                .key_len = @intCast(key_len),
                                .depth = @intCast(depth),
                            };
                            written += 1;
                        }
                        // Leave pos at the char after the closing '"'; the
                        // colon will be consumed in the next loop iteration
                        // as an `else` byte.
                    }
                }
                // pos already advanced past the string.
                continue;
            },
            // Skip whitespace, colons, commas, numbers, booleans, null, etc.
            else => {
                pos += 1;
            },
        }
    }

    return written;
}

/// Return the position of the closing `"` of the string whose content
/// starts at `start` (i.e., `start` is the byte immediately after the
/// opening `"`).  Uses SIMD to skip over runs of ordinary characters.
///
/// On return, `buf[result]` is either `"` or out-of-bounds (`result == len`).
fn scan_string_end(buf: []const u8, start: u32, len: u32) u32 {
    var pos = start;

    const quote_vec: @Vector(16, u8) = @splat(@as(u8, '"'));
    const backslash_vec: @Vector(16, u8) = @splat(@as(u8, '\\'));
    const chunk_size = 16;

    while (pos < len) {
        if (pos + chunk_size <= len) {
            const chunk: @Vector(16, u8) = buf[pos..][0..chunk_size].*;
            const q = chunk == quote_vec;
            const bs = chunk == backslash_vec;

            // Find the first lane with a '"' or '\'.
            var first: u32 = chunk_size; // no match sentinel
            inline for (0..chunk_size) |i| {
                if ((q[i] or bs[i]) and @as(u32, i) < first) {
                    first = i;
                }
            }

            if (first == chunk_size) {
                // No special chars in this chunk — skip all 16 bytes.
                pos += chunk_size;
                continue;
            }

            pos += first; // advance to the special character
        }

        // Scalar: handle the special character at `pos`.
        if (pos >= len) break;

        if (buf[pos] == '"') return pos; // found closing quote

        if (buf[pos] == '\\') {
            // Skip escape indicator and the following escaped byte.
            pos += 1;
            if (pos < len) pos += 1;
        } else {
            pos += 1;
        }
    }

    return len; // unclosed string (malformed JSON)
}

/// Skip over a balanced `{…}` or `[…]` block, correctly handling nested
/// structures and string literals.
///
/// `start` points at the opening `{` or `[`.  Returns the position
/// immediately after the matching closing bracket, or `len` if unmatched.
fn skip_balanced(buf: []const u8, start: u32, len: u32) u32 {
    var pos = start + 1; // skip opening bracket
    var depth: u32 = 1;

    while (pos < len and depth > 0) {
        switch (buf[pos]) {
            '{', '[' => {
                depth += 1;
                pos += 1;
            },
            '}', ']' => {
                depth -= 1;
                pos += 1;
            },
            '"' => {
                pos += 1; // skip opening '"'
                pos = scan_string_end(buf, pos, len);
                if (pos < len and buf[pos] == '"') pos += 1; // skip closing '"'
            },
            else => pos += 1,
        }
    }

    return pos;
}

/// Returns true for JSON whitespace characters.
inline fn is_ws(c: u8) bool {
    return c == ' ' or c == '\t' or c == '\n' or c == '\r';
}

// ============================================================================
// Tests
// ============================================================================

test "test_json_flat_keys: basic flat object key extraction" {
    const text =
        \\{"host": "localhost", "port": 5432}
    ;
    var out: [8]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 8, &exceeded);
    try std.testing.expect(!exceeded);
    try std.testing.expectEqual(@as(u32, 2), w);

    // "host" at offset 2 (after `{"`)
    try std.testing.expectEqual(@as(u32, 2), out[0].key_offset);
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 0), out[0].depth);

    // "port" key_len = 4, depth = 0
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len);
    try std.testing.expectEqual(@as(u16, 0), out[1].depth);
}

test "test_json_nested_keys: nested object depth tracking" {
    const text =
        \\{"root": {"child": 1, "sibling": 2}}
    ;
    var out: [8]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 8, &exceeded);
    try std.testing.expect(!exceeded);
    try std.testing.expectEqual(@as(u32, 3), w);

    // "root" at depth 0
    try std.testing.expectEqual(@as(u16, 4), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 0), out[0].depth);

    // "child" at depth 1
    try std.testing.expectEqual(@as(u16, 5), out[1].key_len);
    try std.testing.expectEqual(@as(u16, 1), out[1].depth);

    // "sibling" at depth 1
    try std.testing.expectEqual(@as(u16, 7), out[2].key_len);
    try std.testing.expectEqual(@as(u16, 1), out[2].depth);
}

test "test_json_escaped_quotes: escaped quote does not terminate string scan" {
    // Key is: the\"key (with an embedded escaped double-quote)
    // Written as JSON: {"the\"key": 1}
    const text = "{\"the\\\"key\": 1}";
    var out: [4]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 4, &exceeded);
    try std.testing.expect(!exceeded);
    try std.testing.expectEqual(@as(u32, 1), w);
    // key content is: the\"key → 9 bytes (t,h,e,\,",k,e,y → wait, \\" is 2 chars: \ and ")
    // In the source string `"the\\\"key"`:
    //   \" → "  (in Zig string literal)
    //   \\" → \"  meaning backslash then quote in actual bytes
    // Actual bytes in text: { " t h e \ " k e y " : space 1 }
    // Key content (between the outer quotes): t h e \ " k e y → 8 bytes
    try std.testing.expectEqual(@as(u16, 8), out[0].key_len);
    try std.testing.expectEqual(@as(u16, 0), out[0].depth);
}

test "test_json_depth_limit: depth > 100 sets depth_exceeded" {
    // Build a JSON string with 101 nested objects: {"k":{"k":...{"k":1}...}}
    // Each level costs 5 bytes opening ("{"k":") + 1 closing ("}").
    // 101 levels = 101*5 + 1 + 101 = 607 bytes.
    const depth_count = 101;
    var buf: [768]u8 = undefined;
    var pos: usize = 0;
    for (0..depth_count) |_| {
        buf[pos] = '{';
        pos += 1;
        buf[pos] = '"';
        pos += 1;
        buf[pos] = 'k';
        pos += 1;
        buf[pos] = '"';
        pos += 1;
        buf[pos] = ':';
        pos += 1;
    }
    buf[pos] = '1';
    pos += 1;
    for (0..depth_count) |_| {
        buf[pos] = '}';
        pos += 1;
    }

    var out: [256]JsonKeyEntry = undefined;
    var exceeded = false;
    _ = scan_json_keys(&buf, @intCast(pos), &out, 256, &exceeded);
    try std.testing.expect(exceeded);
}

test "test_json_array_keys: keys inside objects nested within arrays are extracted" {
    // {"arr": [{"k": 1}, {"k": 2}]}
    const text =
        \\{"arr": [{"k": 1}, {"k": 2}]}
    ;
    var out: [8]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 8, &exceeded);
    try std.testing.expect(!exceeded);
    // "arr" at depth 0, then two "k" entries at depth 2
    // (object → array → object = depth 2)
    try std.testing.expectEqual(@as(u32, 3), w);

    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "arr"
    try std.testing.expectEqual(@as(u16, 0), out[0].depth);

    try std.testing.expectEqual(@as(u16, 1), out[1].key_len); // "k"
    try std.testing.expectEqual(@as(u16, 2), out[1].depth); // obj→arr→obj

    try std.testing.expectEqual(@as(u16, 1), out[2].key_len); // "k"
    try std.testing.expectEqual(@as(u16, 2), out[2].depth);
}

test "json_empty_input: returns 0 entries" {
    var out: [4]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys("".ptr, 0, &out, 4, &exceeded);
    try std.testing.expectEqual(@as(u32, 0), w);
    try std.testing.expect(!exceeded);
}

test "json_cap_zero: returns 0 when cap is 0" {
    const text = "{\"k\": 1}";
    var out: [1]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 0, &exceeded);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "json_string_value_not_extracted: string values are not treated as keys" {
    // Value strings must not be emitted as keys.
    const text =
        \\{"key": "value"}
    ;
    var out: [8]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 8, &exceeded);
    try std.testing.expectEqual(@as(u32, 1), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "key" only
}

test "json_cap_enforcement: stops writing when cap is reached" {
    const text =
        \\{"a": 1, "b": 2, "c": 3}
    ;
    var out: [2]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 2, &exceeded);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 1), out[0].key_len); // "a"
    try std.testing.expectEqual(@as(u16, 1), out[1].key_len); // "b"
}

test "json_colon_in_string_value: colons in string values do not confuse scanner" {
    // The value contains "://" — must not split at the wrong colon.
    const text =
        \\{"url": "https://example.com", "name": "test"}
    ;
    var out: [8]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 8, &exceeded);
    try std.testing.expectEqual(@as(u32, 2), w);
    try std.testing.expectEqual(@as(u16, 3), out[0].key_len); // "url"
    try std.testing.expectEqual(@as(u16, 4), out[1].key_len); // "name"
}

test "json_empty_object: {} returns 0 keys" {
    const text = "{}";
    var out: [4]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 4, &exceeded);
    try std.testing.expectEqual(@as(u32, 0), w);
    try std.testing.expect(!exceeded);
}

test "json_empty_array: [] returns 0 keys" {
    const text = "[]";
    var out: [4]JsonKeyEntry = undefined;
    var exceeded = false;
    const w = scan_json_keys(text.ptr, @intCast(text.len), &out, 4, &exceeded);
    try std.testing.expectEqual(@as(u32, 0), w);
    try std.testing.expect(!exceeded);
}
