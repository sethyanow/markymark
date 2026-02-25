// Tests for ExtractionRenderer — tasks, embeds, callouts, and block refs.
// Split from extraction_renderer_tests.zig to keep files under 1000 lines.

const std = @import("std");
const testing = std.testing;
const extraction_renderer = @import("./extraction_renderer.zig");
const extractFromMarkdown = extraction_renderer.extractFromMarkdown;

// --- Task tests ---

test "extract task unchecked" {
    const input = "- [ ] Todo\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.tasks.len);
    try testing.expectEqual(@as(u8, ' '), result.tasks[0].state);
    try testing.expectEqualStrings("Todo", result.tasks[0].text);
}

test "extract task checked lowercase" {
    const input = "- [x] Done\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.tasks.len);
    try testing.expectEqual(@as(u8, 'x'), result.tasks[0].state);
    try testing.expectEqualStrings("Done", result.tasks[0].text);
}

test "extract task checked uppercase" {
    const input = "- [X] Done\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.tasks.len);
    try testing.expectEqual(@as(u8, 'X'), result.tasks[0].state);
}

test "extract nested tasks" {
    const input = "- [x] Parent\n  - [ ] Child\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.tasks.len);
    try testing.expectEqualStrings("Parent", result.tasks[0].text);
    try testing.expectEqualStrings("Child", result.tasks[1].text);
}

test "extract non-task li produces zero tasks" {
    const input = "- Not a task\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.tasks.len);
}

test "extract non-task nested in task" {
    const input = "- [x] Parent\n  - Child\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.tasks.len);
    try testing.expectEqualStrings("Parent", result.tasks[0].text);
}

test "extract task with formatting" {
    const input = "- [x] **bold** text\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.tasks.len);
    try testing.expectEqualStrings("bold text", result.tasks[0].text);
}

test "extract task in ordered list" {
    const input = "1. [x] Task\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.tasks.len);
    try testing.expectEqualStrings("Task", result.tasks[0].text);
}

test "extract task offset points to bracket" {
    const input = "- [ ] Todo\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.tasks.len);
    try testing.expect(input[result.tasks[0].offset] == '[');
}

test "task offset skips non-checkbox bracket pairs (marky-4do1)" {
    // Regression: findTaskOffset must only match valid checkbox markers [ ], [x], [X].
    // Without fix, [a] in prose before the task checkbox would match first.
    // "See [a] for details\n\n- [ ] do the thing\n"
    // Byte layout:
    //   "See [a] for details\n" = 0..20
    //   "\n"                    = 20..21
    //   "- "                    = 21..23
    //   "[ ] "                  = 23..27  (checkbox '[' at 23)
    //   "do the thing\n"       = 27..40
    const input = "See [a] for details\n\n- [ ] do the thing\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.tasks.len);
    try testing.expectEqualStrings("do the thing", result.tasks[0].text);
    try testing.expectEqual(@as(u32, 23), result.tasks[0].offset);
    try testing.expect(input[result.tasks[0].offset] == '[');
    // The middle character must be a valid checkbox marker
    try testing.expect(input[result.tasks[0].offset + 1] == ' ');
}

// --- Embed tests ---

test "extract embed basic" {
    const input = "![[target]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.embeds.len);
    try testing.expectEqualStrings("target", result.embeds[0].target);
    // Also recorded as a wikilink
    try testing.expectEqual(@as(usize, 1), result.links.len);
    try testing.expect(result.links[0].is_wiki);
}

test "extract wikilink is not embed" {
    const input = "[[link]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.embeds.len);
    try testing.expectEqual(@as(usize, 1), result.links.len);
}

test "extract embed with heading fragment" {
    const input = "![[page#heading]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.embeds.len);
    try testing.expectEqualStrings("page#heading", result.embeds[0].target);
}

test "extract multiple embeds" {
    const input = "![[a]] text ![[b]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.embeds.len);
    try testing.expectEqualStrings("a", result.embeds[0].target);
    try testing.expectEqualStrings("b", result.embeds[1].target);
}

test "extract embed offset points to bang" {
    const input = "![[target]]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.embeds.len);
    try testing.expect(input[result.embeds[0].offset] == '!');
}

// ── Callout tests ──────────────────────────────────────────────────

test "callout basic note" {
    const input = "> [!note]\n> Content\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.callouts.len);
    try testing.expectEqualStrings("note", result.callouts[0].callout_type);
    try testing.expectEqual(@as(?[]const u8, null), result.callouts[0].title);
}

test "callout with title" {
    const input = "> [!tip] My Title\n> Content\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.callouts.len);
    try testing.expectEqualStrings("tip", result.callouts[0].callout_type);
    try testing.expectEqualStrings("My Title", result.callouts[0].title.?);
}

test "callout nested blockquote ignored" {
    // Only depth-1 blockquotes are checked for callout markers.
    // Inner blockquote's [!note] at depth 2 is not a callout.
    const input = "> > [!note]\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.callouts.len);
}

test "standard blockquote no callout" {
    const input = "> Some text\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.callouts.len);
}

test "callout uppercase type normalized" {
    const input = "> [!WARNING]\n> Be careful\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.callouts.len);
    try testing.expectEqualStrings("warning", result.callouts[0].callout_type);
}

test "callout empty type rejected" {
    const input = "> [!]\n> Content\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.callouts.len);
}

// ── Block ref tests ────────────────────────────────────────────────

test "block ref basic uuid" {
    const input = "Text ((a1b2c3d4-e5f6-7890-abcd-ef1234567890)) more\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.block_refs.len);
    try testing.expectEqualStrings("a1b2c3d4-e5f6-7890-abcd-ef1234567890", result.block_refs[0].uuid);
}

test "block ref in code block not extracted" {
    const input = "```\n((a1b2c3d4-e5f6-7890-abcd-ef1234567890))\n```\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.block_refs.len);
}

test "block ref in code span not extracted" {
    const input = "`((a1b2c3d4-e5f6-7890-abcd-ef1234567890))`\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.block_refs.len);
}

test "block ref multiple on line" {
    const input = "((a1b2c3d4-e5f6-7890-abcd-ef1234567890)) and ((b2c3d4e5-f6a7-8901-bcde-f12345678901))\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.block_refs.len);
    try testing.expectEqualStrings("a1b2c3d4-e5f6-7890-abcd-ef1234567890", result.block_refs[0].uuid);
    try testing.expectEqualStrings("b2c3d4e5-f6a7-8901-bcde-f12345678901", result.block_refs[1].uuid);
}

test "block ref invalid uuid rejected" {
    const input = "((not-valid)) and ((too-short))\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.block_refs.len);
}

test "block ref uppercase hex accepted" {
    const input = "((A1B2C3D4-E5F6-7890-ABCD-EF1234567890))\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.block_refs.len);
    try testing.expectEqualStrings("A1B2C3D4-E5F6-7890-ABCD-EF1234567890", result.block_refs[0].uuid);
}
