//! C ABI exports for binary index serialization.
//!
//! Handle from marky_index_deserialize is a small descriptor (base+len). The actual
//! buffer is caller-owned (typically mmap'd). marky_index_destroy frees the descriptor
//! only; caller must unmap the buffer.

const std = @import("std");
const index_serde = @import("kernels/index_serde.zig");
const IndexData = index_serde.IndexData;
const IndexView = index_serde.IndexView;
const IndexHeading = index_serde.IndexHeading;
const IndexLink = index_serde.IndexLink;
const IndexTag = index_serde.IndexTag;
const IndexBlockId = index_serde.IndexBlockId;

const Descriptor = struct {
    base: [*]const u8,
    len: usize,
};

const allocator = std.heap.page_allocator;

fn getView(handle: ?*anyopaque) ?IndexView {
    const ptr = handle orelse return null;
    const d: *const Descriptor = @ptrCast(@alignCast(ptr));
    return IndexView.init(d.base, d.len);
}

/// Serialize index data to output buffer.
///
/// Returns: 0 success, -1 invalid input, -2 buffer too small, -3 internal
export fn marky_index_serialize(
    data: ?*const IndexData,
    output: ?[*]u8,
    cap: u32,
    written: ?*u32,
) i32 {
    return index_serde.serialize_index(data, output, cap, written);
}

/// Deserialize from buffer. For mmap: caller passes pointer to mmap'd region.
/// Returns opaque handle on success, null on invalid/corrupt data.
/// Allocates a small descriptor; marky_index_destroy frees it. Caller owns the buffer.
export fn marky_index_deserialize(buf: ?[*]const u8, len: u32) ?*anyopaque {
    const b = buf orelse return null;
    if (len == 0) return null;

    _ = IndexView.init(b, len) orelse return null;

    const d = allocator.create(Descriptor) catch return null;
    d.base = b;
    d.len = len;
    return @ptrCast(d);
}

/// Free the descriptor. Does NOT free the buffer (caller owns it, e.g. munmap).
export fn marky_index_destroy(handle: ?*anyopaque) void {
    const ptr = handle orelse return;
    const d: *Descriptor = @ptrCast(@alignCast(ptr));
    allocator.destroy(d);
}

/// Query heading count.
export fn marky_index_heading_count(handle: ?*anyopaque) u32 {
    const v = getView(handle) orelse return 0;
    return v.headingCount();
}

/// Query link count.
export fn marky_index_link_count(handle: ?*anyopaque) u32 {
    const v = getView(handle) orelse return 0;
    return v.linkCount();
}

/// Query tag count.
export fn marky_index_tag_count(handle: ?*anyopaque) u32 {
    const v = getView(handle) orelse return 0;
    return v.tagCount();
}

/// Query block ID count.
export fn marky_index_block_id_count(handle: ?*anyopaque) u32 {
    const v = getView(handle) orelse return 0;
    return v.blockIdCount();
}

/// Query doc count.
export fn marky_index_doc_count(handle: ?*anyopaque) u32 {
    const v = getView(handle) orelse return 0;
    return v.docCount();
}

// ============================================================================
// C ABI tests
// ============================================================================

test "C ABI: serialize empty index" {
    var buf: [128]u8 = undefined;
    var written: u32 = 0;
    const empty: IndexData = .{};
    const rc = marky_index_serialize(&empty, &buf, 128, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(index_serde.Header.SIZE, written);
}

test "C ABI: deserialize and query" {
    var buf: [256]u8 = undefined;
    var written: u32 = 0;
    const heading_text = "Hello";
    var headings: [1]IndexHeading = .{
        .{ .doc_id = 0, .string_offset = 0, .length = 5, .level = 2 },
    };
    var data: IndexData = .{
        .doc_count = 1,
        .heading_count = 1,
        .headings = &headings,
        .string_table = heading_text.ptr,
        .string_table_size = 5,
    };
    const rc = marky_index_serialize(&data, &buf, 256, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);

    const h = marky_index_deserialize(&buf, written);
    try std.testing.expect(h != null);
    defer marky_index_destroy(h);

    try std.testing.expectEqual(@as(u32, 1), marky_index_heading_count(h));
    try std.testing.expectEqual(@as(u32, 1), marky_index_doc_count(h));
}

test "C ABI: deserialize corrupt returns null" {
    var buf: [32]u8 = undefined;
    buf[0] = 'X';
    const h = marky_index_deserialize(&buf, 32);
    try std.testing.expect(h == null);
}

test "C ABI: destroy null is no-op" {
    marky_index_destroy(null);
}
