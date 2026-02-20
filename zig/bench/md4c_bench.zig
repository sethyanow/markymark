/// md4c ExtractionRenderer benchmark — measures pure Zig extraction time
/// without FFI overhead. Run: zig build bench-md4c
const std = @import("std");
const extraction_renderer = @import("extraction_renderer");
const extractFromMarkdown = extraction_renderer.extractFromMarkdown;

const ITERATIONS: u32 = 1_000;

fn generateMarkdownDoc(allocator: std.mem.Allocator, target_bytes: usize) ![]u8 {
    // Build document by appending repeated sections
    var buf = std.ArrayListUnmanaged(u8){};
    errdefer buf.deinit(allocator);

    try buf.appendSlice(allocator, "# BRZA benchmark corpus\n\n");
    var section: usize = 1;
    while (buf.items.len < target_bytes) {
        // Section header
        var hdr: [64]u8 = undefined;
        const hdr_slice = std.fmt.bufPrint(&hdr, "## Section {d}\n\n", .{section}) catch unreachable;
        try buf.appendSlice(allocator, hdr_slice);
        try buf.appendSlice(allocator, "Fast paths should skip fenced code and still catch [[wiki links]].\n");
        try buf.appendSlice(allocator, "- Item with [markdown](https://example.com/a)\n");
        try buf.appendSlice(allocator, "- Item with [[Wiki Target|Alias]] and #tag\n");
        try buf.appendSlice(allocator, "Paragraph with ^block-id and [docs](https://example.com/docs).\n\n");
        try buf.appendSlice(allocator, "```rust\n");
        try buf.appendSlice(allocator, "fn ignored() { let x = \"[[not_a_link]]\"; }\n");
        try buf.appendSlice(allocator, "```\n\n");
        section += 1;
    }

    return buf.toOwnedSlice(allocator);
}

fn benchSize(allocator: std.mem.Allocator, label: []const u8, target_bytes: usize) !void {
    const doc = try generateMarkdownDoc(allocator, target_bytes);
    defer allocator.free(doc);

    // Warm up
    for (0..10) |_| {
        var result = try extractFromMarkdown(doc, allocator);
        result.deinit();
    }

    // Timed run
    var timer = try std.time.Timer.start();
    var heading_total: usize = 0;
    var link_total: usize = 0;
    for (0..ITERATIONS) |_| {
        var result = try extractFromMarkdown(doc, allocator);
        heading_total += result.headings.len;
        link_total += result.links.len;
        result.deinit();
    }
    const elapsed_ns = timer.read();

    const per_iter_ns = elapsed_ns / ITERATIONS;
    const per_iter_us: f64 = @as(f64, @floatFromInt(per_iter_ns)) / 1_000.0;
    const throughput_mbs: f64 = @as(f64, @floatFromInt(doc.len)) * @as(f64, @floatFromInt(ITERATIONS)) / (@as(f64, @floatFromInt(elapsed_ns)) / 1_000_000_000.0) / (1024.0 * 1024.0);

    std.debug.print("  {s:<6} ({d:>6} bytes): {d:>8.1} us/iter  {d:>6.1} MB/s  ({d} headings, {d} links per iter)\n", .{
        label,
        doc.len,
        per_iter_us,
        throughput_mbs,
        heading_total / ITERATIONS,
        link_total / ITERATIONS,
    });
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    std.debug.print("\nmd4c ExtractionRenderer benchmark ({d} iterations, ReleaseFast)\n", .{ITERATIONS});
    std.debug.print("─────────────────────────────────────────────────────────────\n", .{});

    try benchSize(allocator, "1kb", 1_024);
    try benchSize(allocator, "10kb", 10_240);
    try benchSize(allocator, "50kb", 51_200);
    try benchSize(allocator, "100kb", 102_400);

    std.debug.print("─────────────────────────────────────────────────────────────\n", .{});
}
