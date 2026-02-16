const std = @import("std");
const heading_scan = @import("kernels/heading_scan.zig");

/// Re-export HeadingScan for C consumers
pub const HeadingScan = heading_scan.HeadingScan;

/// Version constant for markymark kernels
/// Format: 0xMMmmpp (major, minor, patch)
const MARKY_VERSION: u32 = 0x000100; // 0.1.0

/// Returns the version of libmarky_kernels.
/// This is a no-op export to verify build system linkage.
/// `export fn` gets C calling convention by default in Zig 0.15.x.
export fn marky_version() u32 {
    return MARKY_VERSION;
}

/// SIMD-accelerated heading extraction.
///
/// Scans `text[0..len]` for ATX headings (# at line start followed by space).
/// Writes results into `out[0..cap]`, sets `*written` to the number found.
///
/// Returns:
///   0  — success
///  -1  — invalid input (null pointer)
///  -2  — buffer too small (cap=0, or more headings than cap)
export fn marky_scan_headings(
    text: ?[*]const u8,
    len: u32,
    out: ?[*]HeadingScan,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    const t = text orelse {
        if (len == 0) {
            w.* = 0;
            return 0;
        }
        return -1;
    };
    const o = out orelse return -1;

    if (len == 0) {
        w.* = 0;
        return 0;
    }

    if (cap == 0) return -2;

    const count = heading_scan.scan_headings(t, len, o, cap);
    w.* = count;

    // If we filled the buffer exactly, there may be more headings
    if (count >= cap) return -2;

    return 0;
}

// ============================================================================
// Tests
// ============================================================================

// Pull in kernel tests so they run as part of `zig build test`
test {
    _ = @import("kernels/heading_scan.zig");
    _ = @import("reference/heading_scan_ref.zig");
}

test "marky_version returns expected version" {
    const version = marky_version();
    try std.testing.expectEqual(@as(u32, 0x000100), version);
}

test "version format is correct" {
    const version = marky_version();
    const major: u8 = @truncate(version >> 16);
    const minor: u8 = @truncate(version >> 8);
    const patch: u8 = @truncate(version);

    try std.testing.expectEqual(@as(u8, 0), major);
    try std.testing.expectEqual(@as(u8, 1), minor);
    try std.testing.expectEqual(@as(u8, 0), patch);
}

test "marky_scan_headings basic" {
    const text = "# Hello\n## World\n";
    var out: [8]HeadingScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 8, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 2), w);
}

test "marky_scan_headings null text with zero len" {
    var w: u32 = undefined;
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(null, 0, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(@as(u32, 0), w);
}

test "marky_scan_headings null text with nonzero len" {
    var w: u32 = undefined;
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(null, 10, &out, 4, &w);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_headings null written" {
    const text = "# Hello\n";
    var out: [4]HeadingScan = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 4, null);
    try std.testing.expectEqual(@as(i32, -1), rc);
}

test "marky_scan_headings zero cap" {
    const text = "# Hello\n";
    var out: [4]HeadingScan = undefined;
    var w: u32 = undefined;
    const rc = marky_scan_headings(text.ptr, text.len, &out, 0, &w);
    try std.testing.expectEqual(@as(i32, -2), rc);
}
