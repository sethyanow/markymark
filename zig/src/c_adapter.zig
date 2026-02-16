const std = @import("std");
const heading_scan = @import("kernels/heading_scan.zig");
const token_estimate = @import("kernels/token_estimate.zig");
const content_hash_mod = @import("kernels/content_hash.zig");

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

/// Approximate BPE token count via SIMD word boundary detection.
///
/// Returns approximate token count for the given text.
/// Returns 0 for null text pointer or zero length.
export fn marky_estimate_tokens(
    text: ?[*]const u8,
    len: u32,
) u32 {
    const t = text orelse return 0;
    if (len == 0) return 0;
    return token_estimate.estimate_tokens(t, len);
}

/// FNV-1a 64-bit content fingerprint.
///
/// Returns a deterministic hash of the text content.
/// Returns 0 for null text pointer. Returns FNV offset basis for zero length.
export fn marky_content_hash(
    text: ?[*]const u8,
    len: u32,
) u64 {
    const t = text orelse return 0;
    return content_hash_mod.content_hash(t, len);
}

// ============================================================================
// Tests
// ============================================================================

// Pull in kernel tests so they run as part of `zig build test`
test {
    _ = @import("kernels/heading_scan.zig");
    _ = @import("reference/heading_scan_ref.zig");
    _ = @import("kernels/token_estimate.zig");
    _ = @import("kernels/content_hash.zig");
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

// -- marky_estimate_tokens tests --

test "marky_estimate_tokens basic" {
    const text = "hello world foo bar";
    const result = marky_estimate_tokens(text.ptr, text.len);
    // 4 words * 1.3 = 5.2 -> (4*13+5)/10 = 57/10 = 5
    try std.testing.expectEqual(@as(u32, 5), result);
}

test "marky_estimate_tokens null text" {
    const result = marky_estimate_tokens(null, 10);
    try std.testing.expectEqual(@as(u32, 0), result);
}

test "marky_estimate_tokens zero length" {
    const text = "hello";
    const result = marky_estimate_tokens(text.ptr, 0);
    try std.testing.expectEqual(@as(u32, 0), result);
}

// -- marky_content_hash tests --

test "marky_content_hash basic" {
    const text = "hello";
    const hash = marky_content_hash(text.ptr, text.len);
    try std.testing.expect(hash != 0);
    // Deterministic
    const hash2 = marky_content_hash(text.ptr, text.len);
    try std.testing.expectEqual(hash, hash2);
}

test "marky_content_hash null text" {
    const result = marky_content_hash(null, 10);
    try std.testing.expectEqual(@as(u64, 0), result);
}

test "marky_content_hash zero length" {
    const text = "hello";
    const result = marky_content_hash(text.ptr, 0);
    // FNV offset basis for empty
    try std.testing.expectEqual(@as(u64, 0xcbf29ce484222325), result);
}

test "marky_content_hash distinct" {
    const hash1 = marky_content_hash("abc".ptr, 3);
    const hash2 = marky_content_hash("def".ptr, 3);
    try std.testing.expect(hash1 != hash2);
}
