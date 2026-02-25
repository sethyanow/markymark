// Tests for raw source scanning functions (properties, query blocks, link definitions).
// Separated from extraction_renderer_tests.zig to keep test files under 1000 lines.

const std = @import("std");
const testing = std.testing;
const extraction_renderer = @import("extraction_renderer.zig");
const extractFromMarkdown = extraction_renderer.extractFromMarkdown;
const scans = @import("extraction_renderer_scans.zig");
const ExtractedProperty = extraction_renderer.ExtractedProperty;

// ── Property scanning: unit tests on scanPropertiesInSource ──────────

test "scanProperties basic string" {
    const src = "tags:: project\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("tags", props.items[0].key);
    try testing.expectEqualStrings("project", props.items[0].value);
    try testing.expectEqual(@as(u8, 0), props.items[0].value_type); // string
}

test "scanProperties list with commas" {
    const src = "tags:: foo, bar, baz\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("tags", props.items[0].key);
    try testing.expectEqualStrings("foo, bar, baz", props.items[0].value);
    try testing.expectEqual(@as(u8, 1), props.items[0].value_type); // list
}

test "scanProperties single page ref" {
    const src = "author:: [[Jane]]\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("author", props.items[0].key);
    try testing.expectEqualStrings("[[Jane]]", props.items[0].value);
    try testing.expectEqual(@as(u8, 2), props.items[0].value_type); // page_ref
}

test "scanProperties multiple page refs is list" {
    const src = "related:: [[A]], [[B]]\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("related", props.items[0].key);
    try testing.expectEqualStrings("[[A]], [[B]]", props.items[0].value);
    try testing.expectEqual(@as(u8, 1), props.items[0].value_type); // list (multiple page refs)
}

test "scanProperties stops at blank line" {
    const src = "tags:: project\nstatus:: active\n\nauthor:: ignored\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 2), props.items.len);
    try testing.expectEqualStrings("tags", props.items[0].key);
    try testing.expectEqualStrings("status", props.items[1].key);
}

test "scanProperties stops at heading" {
    const src = "tags:: project\n# My Heading\nauthor:: ignored\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("tags", props.items[0].key);
}

test "scanProperties empty value" {
    const src = "key::\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("key", props.items[0].key);
    try testing.expectEqualStrings("", props.items[0].value);
    try testing.expectEqual(@as(u8, 0), props.items[0].value_type); // string
}

test "scanProperties no properties (heading only)" {
    const src = "# Just a heading\nSome content\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 0), props.items.len);
}

test "scanProperties first double colon is delimiter" {
    const src = "key:: val:: more\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("key", props.items[0].key);
    try testing.expectEqualStrings("val:: more", props.items[0].value);
}

test "scanProperties multiple properties" {
    const src = "title:: My Page\ntags:: foo, bar\nstatus:: active\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 3), props.items.len);
    try testing.expectEqualStrings("title", props.items[0].key);
    try testing.expectEqualStrings("My Page", props.items[0].value);
    try testing.expectEqual(@as(u8, 0), props.items[0].value_type); // string
    try testing.expectEqualStrings("tags", props.items[1].key);
    try testing.expectEqual(@as(u8, 1), props.items[1].value_type); // list
    try testing.expectEqualStrings("status", props.items[2].key);
    try testing.expectEqual(@as(u8, 0), props.items[2].value_type); // string
}

test "scanProperties empty key skipped" {
    const src = ":: no key here\ntags:: valid\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("tags", props.items[0].key);
}

test "scanProperties line without double colon skipped" {
    const src = "not a property\ntags:: project\n";
    var props: std.ArrayListUnmanaged(ExtractedProperty) = .{};
    defer {
        for (props.items) |p| {
            testing.allocator.free(p.key);
            testing.allocator.free(p.value);
        }
        props.deinit(testing.allocator);
    }

    const oom = scans.scanPropertiesInSource(src, testing.allocator, &props);
    try testing.expect(!oom);
    try testing.expectEqual(@as(usize, 1), props.items.len);
    try testing.expectEqualStrings("tags", props.items[0].key);
}

// ── Integration tests: properties through extractFromMarkdown pipeline ──

test "extractFromMarkdown properties basic" {
    const input = "tags:: project\nstatus:: active\n\n# Heading\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 2), result.properties.len);
    try testing.expectEqualStrings("tags", result.properties[0].key);
    try testing.expectEqualStrings("project", result.properties[0].value);
    try testing.expectEqual(@as(u8, 0), result.properties[0].value_type);
    try testing.expectEqualStrings("status", result.properties[1].key);
    try testing.expectEqualStrings("active", result.properties[1].value);
}

test "extractFromMarkdown properties with page ref" {
    const input = "author:: [[Jane Doe]]\n\n# Title\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.properties.len);
    try testing.expectEqualStrings("author", result.properties[0].key);
    try testing.expectEqualStrings("[[Jane Doe]]", result.properties[0].value);
    try testing.expectEqual(@as(u8, 2), result.properties[0].value_type); // page_ref
}

test "extractFromMarkdown no properties" {
    const input = "# Just a heading\n\nSome content.\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.properties.len);
}

test "extractFromMarkdown properties coexist with headings and links" {
    const input = "tags:: project\n\n# My Heading\n\n[Link](https://example.com)\n";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 1), result.properties.len);
    try testing.expectEqualStrings("tags", result.properties[0].key);
    try testing.expectEqual(@as(usize, 1), result.headings.len);
    try testing.expectEqual(@as(usize, 1), result.links.len);
}

test "extractFromMarkdown empty input has no properties" {
    const input = "";
    var result = try extractFromMarkdown(input, testing.allocator);
    defer result.deinit();

    try testing.expectEqual(@as(usize, 0), result.properties.len);
}
