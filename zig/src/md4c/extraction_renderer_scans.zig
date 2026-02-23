// Raw source scanning for query blocks and link definitions.
// Called from ExtractionRenderer.leaveBlock(.doc).
// Separated from extraction_renderer.zig to keep it under 1000 lines.

const std = @import("std");
const Allocator = std.mem.Allocator;
const ext_mod = @import("extraction_renderer.zig");
const ExtractedQueryBlock = ext_mod.ExtractedQueryBlock;
const ExtractedLinkDefinition = ext_mod.ExtractedLinkDefinition;
const ExtractedProperty = ext_mod.ExtractedProperty;

/// Scan raw source for `{{query ...}}` patterns, skipping fenced code blocks.
/// Returns true if OOM occurred.
pub fn scanQueryBlocksInSource(
    src: []const u8,
    allocator: Allocator,
    query_blocks: *std.ArrayListUnmanaged(ExtractedQueryBlock),
) bool {
    var pos: u32 = 0;
    var in_fence: bool = false;

    while (pos < src.len) {
        // Track fenced code blocks (``` or ~~~)
        const at_line_start = (pos == 0 or src[pos - 1] == '\n');
        if (at_line_start) {
            var lp = pos;
            while (lp < src.len and src[lp] == ' ') : (lp += 1) {}
            if (lp < src.len and (src[lp] == '`' or src[lp] == '~')) {
                const fence_char = src[lp];
                var fence_len: u32 = 0;
                while (lp < src.len and src[lp] == fence_char) : (lp += 1) {
                    fence_len += 1;
                }
                if (fence_len >= 3) {
                    in_fence = !in_fence;
                    // Skip to end of line
                    while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
                    if (pos < src.len) pos += 1;
                    continue;
                }
            }
        }

        if (in_fence) {
            while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
            if (pos < src.len) pos += 1;
            continue;
        }

        // Look for {{query
        if (pos + 7 < src.len and src[pos] == '{' and src[pos + 1] == '{' and
            std.mem.eql(u8, src[pos + 2 .. pos + 7], "query"))
        {
            // Must be followed by whitespace
            if (pos + 7 < src.len and (src[pos + 7] == ' ' or src[pos + 7] == '\t')) {
                const query_start = pos + 8; // past "{{query "
                // Find closing }}
                var end = query_start;
                var found = false;
                while (end + 1 < src.len) {
                    if (src[end] == '}' and src[end + 1] == '}') {
                        const query_text = std.mem.trim(u8, src[query_start..end], " \t");
                        if (query_text.len > 0) {
                            const owned_query = allocator.dupe(u8, query_text) catch return true;
                            query_blocks.append(allocator, .{
                                .query = owned_query,
                                .offset = pos,
                                .end_offset = end + 2,
                            }) catch {
                                allocator.free(owned_query);
                                return true;
                            };
                        }
                        pos = end + 2;
                        found = true;
                        break;
                    }
                    end += 1;
                }
                if (found) continue;
                pos += 2; // no closing }}, skip opening {{
                continue;
            }
        }
        pos += 1;
    }
    return false;
}

/// Scan raw source for `[label]: url "title"` link definitions, skipping fenced code blocks.
/// Returns true if OOM occurred.
pub fn scanLinkDefinitionsInSource(
    src: []const u8,
    allocator: Allocator,
    link_definitions: *std.ArrayListUnmanaged(ExtractedLinkDefinition),
) bool {
    var pos: u32 = 0;
    var in_fence: bool = false;

    while (pos < src.len) {
        // Must be at line start
        const at_line_start = (pos == 0 or src[pos - 1] == '\n');
        if (!at_line_start) {
            while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
            if (pos < src.len) pos += 1;
            continue;
        }

        // Skip leading spaces (up to 3 per CommonMark)
        var lp = pos;
        var spaces: u32 = 0;
        while (lp < src.len and src[lp] == ' ' and spaces < 3) : (lp += 1) {
            spaces += 1;
        }

        // Check for fence toggle
        if (lp < src.len and (src[lp] == '`' or src[lp] == '~')) {
            const fence_char = src[lp];
            var fence_len: u32 = 0;
            while (lp < src.len and src[lp] == fence_char) : (lp += 1) {
                fence_len += 1;
            }
            if (fence_len >= 3) {
                in_fence = !in_fence;
                while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
                if (pos < src.len) pos += 1;
                continue;
            }
        }

        if (in_fence) {
            while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
            if (pos < src.len) pos += 1;
            continue;
        }

        // Reset lp to after leading spaces
        lp = pos;
        spaces = 0;
        while (lp < src.len and src[lp] == ' ' and spaces < 3) : (lp += 1) {
            spaces += 1;
        }

        // Match [label]:
        if (lp < src.len and src[lp] == '[') {
            if (parseLinkDefinition(src, lp, allocator, link_definitions)) return true;
        }

        // Advance to next line
        while (pos < src.len and src[pos] != '\n') : (pos += 1) {}
        if (pos < src.len) pos += 1;
    }
    return false;
}

/// Parse a single link definition starting at `[`. Returns true on OOM.
fn parseLinkDefinition(
    src: []const u8,
    start: u32,
    allocator: Allocator,
    link_definitions: *std.ArrayListUnmanaged(ExtractedLinkDefinition),
) bool {
    const label_start = start + 1;
    var label_end = label_start;
    while (label_end < src.len and src[label_end] != ']' and src[label_end] != '\n') : (label_end += 1) {}
    if (label_end >= src.len or src[label_end] != ']' or label_end == label_start) return false;

    var cp = label_end + 1;
    if (cp >= src.len or src[cp] != ':') return false;
    cp += 1;

    // Skip whitespace after colon
    while (cp < src.len and (src[cp] == ' ' or src[cp] == '\t')) : (cp += 1) {}

    // Extract URL (non-whitespace)
    const url_start = cp;
    while (cp < src.len and src[cp] != ' ' and src[cp] != '\t' and src[cp] != '\n' and src[cp] != '\r') : (cp += 1) {}
    if (cp == url_start) return false;

    const label = src[label_start..label_end];
    const url = src[url_start..cp];

    // Optional title
    var title: ?[]const u8 = null;
    while (cp < src.len and (src[cp] == ' ' or src[cp] == '\t')) : (cp += 1) {}
    if (cp < src.len and src[cp] == '"') {
        const title_start = cp + 1;
        var title_end = title_start;
        while (title_end < src.len and src[title_end] != '"' and src[title_end] != '\n') : (title_end += 1) {}
        if (title_end < src.len and src[title_end] == '"') {
            title = src[title_start..title_end];
            cp = title_end + 1;
        }
    }

    // End offset: end of content (before newline)
    while (cp < src.len and src[cp] != '\n') : (cp += 1) {}
    const end_offset = cp;

    const owned_label = allocator.dupe(u8, label) catch return true;
    const owned_url = allocator.dupe(u8, url) catch {
        allocator.free(owned_label);
        return true;
    };
    const owned_title: ?[]const u8 = if (title) |t|
        allocator.dupe(u8, t) catch {
            allocator.free(owned_label);
            allocator.free(owned_url);
            return true;
        }
    else
        null;

    link_definitions.append(allocator, .{
        .label = owned_label,
        .url = owned_url,
        .title = owned_title,
        .offset = start,
        .end_offset = end_offset,
    }) catch {
        allocator.free(owned_label);
        allocator.free(owned_url);
        if (owned_title) |t| allocator.free(t);
        return true;
    };
    return false;
}

/// Scan raw source for `key:: value` properties at document start.
/// Properties are Logseq-style key-value pairs before any blank line or heading.
/// Returns true if OOM occurred.
pub fn scanPropertiesInSource(
    src: []const u8,
    allocator: Allocator,
    properties: *std.ArrayListUnmanaged(ExtractedProperty),
) bool {
    var pos: u32 = 0;

    while (pos < src.len) {
        // Find end of current line
        var line_end = pos;
        while (line_end < src.len and src[line_end] != '\n') : (line_end += 1) {}
        const line = src[pos..line_end];

        // Stop at blank line
        if (line.len == 0) break;

        // Stop at heading (line starts with #)
        if (line[0] == '#') break;

        // Look for first :: in line
        if (std.mem.indexOf(u8, line, "::")) |dcolon_rel| {
            const key_raw = line[0..dcolon_rel];
            const value_raw = if (dcolon_rel + 2 < line.len) line[dcolon_rel + 2 ..] else "";
            const key = std.mem.trim(u8, key_raw, " \t");
            const value = std.mem.trim(u8, value_raw, " \t");

            if (key.len > 0) {
                // Classify value type
                const value_type = classifyPropertyValue(value);

                const owned_key = allocator.dupe(u8, key) catch return true;
                const owned_value = allocator.dupe(u8, value) catch {
                    allocator.free(owned_key);
                    return true;
                };
                properties.append(allocator, .{
                    .key = owned_key,
                    .value = owned_value,
                    .value_type = value_type,
                }) catch {
                    allocator.free(owned_key);
                    allocator.free(owned_value);
                    return true;
                };
            }
        }

        // Advance past newline
        pos = line_end;
        if (pos < src.len) pos += 1; // skip \n
    }
    return false;
}

/// Classify a property value: 0=string, 1=list, 2=page_ref.
fn classifyPropertyValue(value: []const u8) u8 {
    // Count occurrences of [[
    var bracket_count: u32 = 0;
    var i: usize = 0;
    while (i + 1 < value.len) : (i += 1) {
        if (value[i] == '[' and value[i + 1] == '[') {
            bracket_count += 1;
            i += 1; // skip second [
        }
    }

    if (bracket_count >= 2) return 1; // list (multiple page refs)
    if (bracket_count == 1) return 2; // single page ref

    // Check for comma (list)
    if (std.mem.indexOfScalar(u8, value, ',') != null) return 1;

    return 0; // string
}

// ── Tests ────────────────────────────────────────────────────────────

test {
    _ = @import("extraction_renderer_scans_tests.zig");
}
