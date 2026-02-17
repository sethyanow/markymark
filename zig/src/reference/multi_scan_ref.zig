const std = @import("std");

/// Types of markdown structural elements detected by the multi-scanner.
pub const ScanType = enum(u8) {
    heading = 0, // ATX heading (# at line start)
    link_open = 1, // Markdown link [
    wiki_link = 2, // Wiki-link [[
    fence_backtick = 3, // Code fence ``` at line start
    fence_tilde = 4, // Code fence ~~~ at line start
    block_id = 5, // Block ID ^
    tag = 6, // Tag #word (preceded by whitespace)
};

/// Unified scan result for all element types.
/// C ABI compatible, 8 bytes packed.
pub const ScanResult = extern struct {
    offset: u32, // byte offset where the match is detected (last pattern byte)
    length: u16, // content length (filled by post-processing)
    scan_type: u8, // ScanType discriminator
    extra: u8, // type-specific data (filled by post-processing)
};

pub const NUM_SCAN_TYPES = 7;
pub const NUM_STATES = 13;

// State assignments (trie nodes):
//  0: root
//  1: after "#"      → output: tag
//  2: after "["      → output: link_open
//  3: after "\n"     → (also BOF initial state)
//  4: after "^"      → output: block_id
//  5: after "[["     → output: wiki_link (+link_open via failure)
//  6: after "\n#"    → output: heading (+tag via failure)
//  7: after "\n`"
//  8: after "\n~"
//  9: after "\n``"
// 10: after "\n~~"
// 11: after "\n```"  → output: fence_backtick
// 12: after "\n~~~"  → output: fence_tilde

/// Initial state when scanning from the beginning of a document.
/// Treats position 0 as if preceded by a newline (BOF = line start).
pub const STATE_BOF = 3;

/// Comptime-constructed Aho-Corasick automaton for markdown pattern matching.
///
/// Patterns compiled into the automaton:
///   "#"      → tag
///   "["      → link_open
///   "[["     → wiki_link
///   "\n#"    → heading
///   "\n```"  → fence_backtick
///   "\n~~~"  → fence_tilde
///   "^"      → block_id
///
/// Two flat arrays for O(1) per-byte lookup:
///   - goto_table[state][byte] → next state
///   - output[state] → bitmask of matching ScanTypes (includes failure chain outputs)
pub const Automaton = struct {
    goto_table: [NUM_STATES][256]u8,
    output: [NUM_STATES]u8,

    /// Advance the automaton by one byte.
    pub inline fn step(self: *const Automaton, state: u8, byte: u8) u8 {
        return self.goto_table[state][byte];
    }

    /// Get output bitmask for a state.
    /// Bit i set ⇒ ScanType @enumFromInt(i) matched.
    pub inline fn matches(self: *const Automaton, state: u8) u8 {
        return self.output[state];
    }

    /// Check if a specific ScanType matches at the given state.
    pub inline fn has_match(self: *const Automaton, state: u8, scan_type: ScanType) bool {
        return (self.output[state] & (@as(u8, 1) << @intFromEnum(scan_type))) != 0;
    }
};

/// Pattern length indexed by ScanType. Used to compute match start offsets.
pub const pattern_lengths = [NUM_SCAN_TYPES]u8{
    2, // heading:        "\n#"
    1, // link_open:      "["
    2, // wiki_link:      "[["
    4, // fence_backtick: "\n```"
    4, // fence_tilde:    "\n~~~"
    1, // block_id:       "^"
    1, // tag:            "#"
};

/// The compiled automaton, fully constructed at compile time.
pub const automaton: Automaton = build_automaton();

fn build_automaton() Automaton {
    @setEvalBranchQuota(20000);
    var result: Automaton = undefined;

    // Initialize: all transitions go to state 0 (root), no outputs
    for (0..NUM_STATES) |s| {
        for (0..256) |b| {
            result.goto_table[s][b] = 0;
        }
        result.output[s] = 0;
    }

    // --- Phase 1: Trie construction (goto function for pattern bytes) ---

    // Root (state 0) transitions for pattern-starting bytes
    result.goto_table[0]['#'] = 1;
    result.goto_table[0]['['] = 2;
    result.goto_table[0]['\n'] = 3;
    result.goto_table[0]['^'] = 4;

    // State 2 ([): child for second [
    result.goto_table[2]['['] = 5;

    // State 3 (\n): children for #, `, ~
    result.goto_table[3]['#'] = 6;
    result.goto_table[3]['`'] = 7;
    result.goto_table[3]['~'] = 8;

    // State 7 (\n`): child for second `
    result.goto_table[7]['`'] = 9;

    // State 8 (\n~): child for second ~
    result.goto_table[8]['~'] = 10;

    // State 9 (\n``): child for third `
    result.goto_table[9]['`'] = 11;

    // State 10 (\n~~): child for third ~
    result.goto_table[10]['~'] = 12;

    // --- Phase 2: Direct outputs (own pattern matches) ---

    result.output[1] = 1 << @intFromEnum(ScanType.tag); // "#"
    result.output[2] = 1 << @intFromEnum(ScanType.link_open); // "["
    result.output[4] = 1 << @intFromEnum(ScanType.block_id); // "^"
    result.output[5] = 1 << @intFromEnum(ScanType.wiki_link); // "[["
    result.output[6] = 1 << @intFromEnum(ScanType.heading); // "\n#"
    result.output[11] = 1 << @intFromEnum(ScanType.fence_backtick); // "\n```"
    result.output[12] = 1 << @intFromEnum(ScanType.fence_tilde); // "\n~~~"

    // --- Phase 3: Failure function (longest proper suffix that is a trie prefix) ---

    const failure = [NUM_STATES]u8{
        0, // 0:  root → root
        0, // 1:  # → root
        0, // 2:  [ → root
        0, // 3:  \n → root
        0, // 4:  ^ → root
        2, // 5:  [[ → [ (state 2)
        1, // 6:  \n# → # (state 1)
        0, // 7:  \n` → root
        0, // 8:  \n~ → root
        0, // 9:  \n`` → root
        0, // 10: \n~~ → root
        0, // 11: \n``` → root
        0, // 12: \n~~~ → root
    };

    // --- Phase 4: Output links (OR in failure chain outputs) ---

    for (0..NUM_STATES) |s| {
        var f = failure[s];
        while (f != 0) {
            result.output[s] |= result.output[f];
            f = failure[f];
        }
    }

    // --- Phase 5: Complete DFA transitions using failure function ---
    // For each non-root state, any byte not explicitly in the trie
    // falls through to goto(failure(state), byte).
    // Process in BFS order (depth 1 first, then depth 2, etc.).

    const bfs_order = [_]u8{ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12 };

    for (bfs_order) |s| {
        const f = failure[s];
        for (0..256) |b_usize| {
            const b: u8 = @intCast(b_usize);

            // Check if this byte was explicitly set as a trie edge from state s
            const is_trie_edge = switch (s) {
                0 => (b == '#' or b == '[' or b == '\n' or b == '^'),
                2 => (b == '['),
                3 => (b == '#' or b == '`' or b == '~'),
                7 => (b == '`'),
                8 => (b == '~'),
                9 => (b == '`'),
                10 => (b == '~'),
                else => false,
            };

            if (!is_trie_edge) {
                result.goto_table[s][b] = result.goto_table[f][b];
            }
        }
    }

    return result;
}

// ============================================================================
// Scalar scan function (reference implementation)
// ============================================================================

/// Scalar single-pass scan using the Aho-Corasick automaton.
///
/// Scans text and emits raw match candidates (unfiltered, unvalidated).
/// The offset in each ScanResult is the byte position of the last byte
/// of the matched pattern. Post-processing (marky-77m) handles validation,
/// content extraction, and fence-map filtering.
///
/// Parameters:
///   text, len: input text
///   out, cap: output buffer for ScanResult entries
///
/// Returns: number of results written
pub fn scan_multi_scalar(
    text: [*]const u8,
    len: u32,
    out: [*]ScanResult,
    cap: u32,
) u32 {
    if (len == 0 or cap == 0) return 0;

    const buf = text[0..len];
    var written: u32 = 0;
    var state: u8 = STATE_BOF; // Treat BOF as after-newline

    for (buf, 0..) |byte, pos_usize| {
        const pos: u32 = @intCast(pos_usize);
        state = automaton.step(state, byte);
        const output = automaton.matches(state);

        if (output != 0) {
            // Emit one ScanResult per matching ScanType
            inline for (0..NUM_SCAN_TYPES) |t| {
                if (output & (@as(u8, 1) << t) != 0) {
                    if (written >= cap) return written;

                    out[written] = ScanResult{
                        .offset = pos,
                        .length = 0,
                        .scan_type = @intCast(t),
                        .extra = 0,
                    };
                    written += 1;
                }
            }
        }
    }

    return written;
}

// ============================================================================
// Tests
// ============================================================================

const testing = std.testing;

test "test_automaton_finds_heading" {
    // Verifies the automaton detects headings at line start
    const text = "# Hello\n## World\n";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    // Should find heading matches
    var heading_count: u32 = 0;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.heading)) heading_count += 1;
    }
    // Two headings: "# Hello" and "## World"
    // First heading at BOF (state starts as after-newline), second after "\n"
    try testing.expectEqual(@as(u32, 2), heading_count);
}

test "test_automaton_finds_wiki_link" {
    // Verifies [[ is distinguished from [
    const text = "[[page]] and [link](url)";
    var out: [32]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 32);

    var wiki_count: u32 = 0;
    var link_count: u32 = 0;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.wiki_link)) wiki_count += 1;
        if (r.scan_type == @intFromEnum(ScanType.link_open)) link_count += 1;
    }
    // [[ emits wiki_link + link_open (via output link), plus first [ emits link_open
    // And the [ in [link] emits link_open
    try testing.expect(wiki_count >= 1);
    try testing.expect(link_count >= 2); // at least [ from [[ and [ from [link]
}

test "test_automaton_overlapping_prefix" {
    // [[ contains [ as prefix — both should match
    const text = "[[foo]]";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    // First [ at pos 0: link_open
    // Second [ at pos 1: wiki_link + link_open (via output link)
    var found_wiki = false;
    var found_link_at_0 = false;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.wiki_link) and r.offset == 1) found_wiki = true;
        if (r.scan_type == @intFromEnum(ScanType.link_open) and r.offset == 0) found_link_at_0 = true;
    }
    try testing.expect(found_wiki);
    try testing.expect(found_link_at_0);
}

test "test_automaton_no_matches" {
    // Plain text with no markdown structural elements
    const text = "Hello world, this is plain text without special chars.";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_heading_at_bof" {
    // Heading at beginning of file (no preceding newline)
    const text = "# Title";
    var out: [8]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 8);

    var found_heading = false;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.heading)) {
            found_heading = true;
            try testing.expectEqual(@as(u32, 0), r.offset);
        }
    }
    try testing.expect(found_heading);
}

test "test_fence_backtick_detection" {
    const text = "text\n```python\ncode\n```\n";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    var fence_count: u32 = 0;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.fence_backtick)) fence_count += 1;
    }
    // Two fence markers: opening ``` and closing ```
    try testing.expectEqual(@as(u32, 2), fence_count);
}

test "test_fence_tilde_detection" {
    const text = "text\n~~~\ncode\n~~~\n";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    var fence_count: u32 = 0;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.fence_tilde)) fence_count += 1;
    }
    try testing.expectEqual(@as(u32, 2), fence_count);
}

test "test_fence_at_bof" {
    // Fence at beginning of file
    const text = "```\ncode\n```\n";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    var fence_count: u32 = 0;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.fence_backtick)) fence_count += 1;
    }
    try testing.expectEqual(@as(u32, 2), fence_count);
}

test "test_block_id_detection" {
    const text = "text ^block-id\n";
    var out: [8]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 8);

    var found_block_id = false;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.block_id)) {
            found_block_id = true;
            try testing.expectEqual(@as(u32, 5), r.offset); // position of ^
        }
    }
    try testing.expect(found_block_id);
}

test "test_tag_detection" {
    // # at positions that could be tags (preceded by whitespace)
    const text = "text #tag1 #tag2";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    var tag_count: u32 = 0;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.tag)) tag_count += 1;
    }
    // The automaton emits tag for every # it sees
    try testing.expectEqual(@as(u32, 2), tag_count);
}

test "test_heading_also_emits_tag" {
    // "\n#" should emit both heading and tag (via output link)
    const text = "text\n# Heading\n";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    var heading_at_5 = false;
    var tag_at_5 = false;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.heading) and r.offset == 5) heading_at_5 = true;
        if (r.scan_type == @intFromEnum(ScanType.tag) and r.offset == 5) tag_at_5 = true;
    }
    // Both heading and tag should fire at the # position
    try testing.expect(heading_at_5);
    try testing.expect(tag_at_5);
}

test "test_mixed_document" {
    // Document with all element types
    const text = "# Title\nSome text #tag1\n[[wiki]] and [link](url)\ntext ^block-id\n```\ncode\n```\n~~~\ntilde\n~~~\n";
    var out: [64]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 64);

    var counts = [_]u32{0} ** NUM_SCAN_TYPES;
    for (out[0..n]) |r| {
        counts[r.scan_type] += 1;
    }

    try testing.expect(counts[@intFromEnum(ScanType.heading)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.tag)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.wiki_link)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.link_open)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.block_id)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.fence_backtick)] >= 1);
    try testing.expect(counts[@intFromEnum(ScanType.fence_tilde)] >= 1);
}

test "test_empty_input" {
    const text = "";
    var out: [4]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, 0, &out, 4);
    try testing.expectEqual(@as(u32, 0), n);
}

test "test_buffer_overflow" {
    const text = "# A\n# B\n# C\n";
    var out: [1]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 1);
    // Should stop at capacity
    try testing.expectEqual(@as(u32, 1), n);
}

test "test_newline_resets_state" {
    // After \n, # should be detected as heading
    // Multiple newlines should each reset properly
    const text = "text\n\n# Heading\n";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    var found_heading = false;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.heading)) found_heading = true;
    }
    try testing.expect(found_heading);
}

test "test_less_than_three_backticks" {
    // Two backticks at line start should NOT emit fence_backtick
    const text = "\n``not a fence\n";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    for (out[0..n]) |r| {
        try testing.expect(r.scan_type != @intFromEnum(ScanType.fence_backtick));
    }
}

test "test_four_backticks_still_detected" {
    // Four+ backticks at line start should emit fence_backtick (at the third)
    const text = "\n````code\n";
    var out: [16]ScanResult = undefined;
    const n = scan_multi_scalar(text.ptr, text.len, &out, 16);

    var found_fence = false;
    for (out[0..n]) |r| {
        if (r.scan_type == @intFromEnum(ScanType.fence_backtick)) found_fence = true;
    }
    try testing.expect(found_fence);
}

test "test_automaton_state_correctness" {
    // Direct automaton state transition verification

    // Root → # → state 1 (tag output)
    try testing.expectEqual(@as(u8, 1), automaton.step(0, '#'));
    try testing.expect(automaton.has_match(1, .tag));

    // Root → [ → state 2 (link_open output)
    try testing.expectEqual(@as(u8, 2), automaton.step(0, '['));
    try testing.expect(automaton.has_match(2, .link_open));

    // State 2 → [ → state 5 (wiki_link output + link_open via failure)
    try testing.expectEqual(@as(u8, 5), automaton.step(2, '['));
    try testing.expect(automaton.has_match(5, .wiki_link));
    try testing.expect(automaton.has_match(5, .link_open));

    // Root → \n → state 3 (no output)
    try testing.expectEqual(@as(u8, 3), automaton.step(0, '\n'));
    try testing.expectEqual(@as(u8, 0), automaton.matches(3));

    // State 3 → # → state 6 (heading + tag via failure)
    try testing.expectEqual(@as(u8, 6), automaton.step(3, '#'));
    try testing.expect(automaton.has_match(6, .heading));
    try testing.expect(automaton.has_match(6, .tag));

    // Root → ^ → state 4 (block_id output)
    try testing.expectEqual(@as(u8, 4), automaton.step(0, '^'));
    try testing.expect(automaton.has_match(4, .block_id));

    // State 3 → ` → 7 → ` → 9 → ` → 11 (fence_backtick)
    try testing.expectEqual(@as(u8, 7), automaton.step(3, '`'));
    try testing.expectEqual(@as(u8, 9), automaton.step(7, '`'));
    try testing.expectEqual(@as(u8, 11), automaton.step(9, '`'));
    try testing.expect(automaton.has_match(11, .fence_backtick));

    // State 3 → ~ → 8 → ~ → 10 → ~ → 12 (fence_tilde)
    try testing.expectEqual(@as(u8, 8), automaton.step(3, '~'));
    try testing.expectEqual(@as(u8, 10), automaton.step(8, '~'));
    try testing.expectEqual(@as(u8, 12), automaton.step(10, '~'));
    try testing.expect(automaton.has_match(12, .fence_tilde));
}

test "test_failure_transitions" {
    // Verify failure function: after reaching a dead end, transition correctly

    // State 2 ([) + 'x' → should follow failure to root, then root('x') = 0
    try testing.expectEqual(@as(u8, 0), automaton.step(2, 'x'));

    // State 3 (\n) + 'x' → failure to root, root('x') = 0
    try testing.expectEqual(@as(u8, 0), automaton.step(3, 'x'));

    // State 3 (\n) + '\n' → should stay in state 3 (newline resets)
    try testing.expectEqual(@as(u8, 3), automaton.step(3, '\n'));

    // State 7 (\n`) + 'x' → failure to root, root('x') = 0
    try testing.expectEqual(@as(u8, 0), automaton.step(7, 'x'));

    // State 5 ([[) + 'x' → failure to state 2 ([), then state 2('x') → failure to root
    // Actually: goto_table[5][x] = goto_table[failure(5)][x] = goto_table[2][x]
    // goto_table[2][x] = goto_table[failure(2)][x] = goto_table[0][x] = 0
    try testing.expectEqual(@as(u8, 0), automaton.step(5, 'x'));

    // State 5 ([[) + '[' → failure to state 2 ([), goto_table[2]['['] = 5
    try testing.expectEqual(@as(u8, 5), automaton.step(5, '['));

    // State 6 (\n#) + '[' → failure to state 1 (#), goto_table[1]['['] = goto_table[0]['['] = 2
    try testing.expectEqual(@as(u8, 2), automaton.step(6, '['));
}
