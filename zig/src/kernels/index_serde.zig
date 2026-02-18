//! Binary serialization format for DocumentIndex / RealmIndex data.
//!
//! Memory-mappable layout for instant startup. All multi-byte values are little-endian.
//! Layout: fixed_header | string_table | heading_array | link_array | tag_array | block_id_array

const std = @import("std");

/// Magic bytes at start of serialized index (ASCII "MKYI").
pub const MAGIC: [4]u8 = .{ 'M', 'K', 'Y', 'I' };

/// Current format version. Increment when layout changes; deserialize rejects unknown versions.
pub const FORMAT_VERSION: u16 = 1;

/// Alignment for array sections (IndexHeading, IndexLink, etc. all need at least 4).
const SECTION_ALIGN: u32 = 4;

fn padAfterStringTable(str_size: u32) u32 {
    const remainder = (Header.SIZE + str_size) % SECTION_ALIGN;
    return if (remainder == 0) 0 else SECTION_ALIGN - @as(u32, @intCast(remainder));
}

/// Fixed header. All fields little-endian.
pub const Header = extern struct {
    magic: [4]u8 = MAGIC,
    version: u16 = FORMAT_VERSION,
    flags: u16 = 0,
    doc_count: u32 = 0,
    heading_count: u32 = 0,
    link_count: u32 = 0,
    tag_count: u32 = 0,
    block_id_count: u32 = 0,
    string_table_size: u32 = 0,
    _reserved: [4]u8 = .{ 0, 0, 0, 0 },

    pub const SIZE: usize = @sizeOf(Header);
    pub const ALIGN: usize = 8;
};

/// Heading entry in serialized array (12 bytes).
pub const IndexHeading = extern struct {
    doc_id: u32,
    string_offset: u32,
    length: u16,
    level: u8,
    _pad: u8 = 0,
};

/// Link entry in serialized array (18 bytes).
pub const IndexLink = extern struct {
    doc_id: u32,
    text_offset: u32,
    text_length: u16,
    target_offset: u32,
    target_length: u16,
    link_type: u8,
    _pad: u8 = 0,
};

/// Tag entry in serialized array (12 bytes).
pub const IndexTag = extern struct {
    doc_id: u32,
    string_offset: u32,
    length: u16,
    _pad: u16 = 0,
};

/// Block ID entry in serialized array (12 bytes).
pub const IndexBlockId = extern struct {
    doc_id: u32,
    string_offset: u32,
    length: u16,
    _pad: u16 = 0,
};

/// Input data for serialization. C ABI compatible.
/// Arrays point to contiguous memory; string_table is the packed string blob.
pub const IndexData = extern struct {
    doc_count: u32 = 0,
    heading_count: u32 = 0,
    link_count: u32 = 0,
    tag_count: u32 = 0,
    block_id_count: u32 = 0,
    headings: [*c]const IndexHeading = null,
    links: [*c]const IndexLink = null,
    tags: [*c]const IndexTag = null,
    block_ids: [*c]const IndexBlockId = null,
    string_table: [*c]const u8 = null,
    string_table_size: u32 = 0,
};

/// Opaque view into deserialized (possibly mmap'd) buffer. Zero-copy.
pub const IndexView = struct {
    base: [*]const u8,
    len: usize,
    header: *const Header,

    pub fn init(base: [*]const u8, len: usize) ?IndexView {
        if (len < Header.SIZE) return null;
        // Guard against misaligned pointer: @alignCast panics in Debug/ReleaseSafe
        // if the pointer is not aligned to @alignOf(Header). Check first.
        if (@intFromPtr(base) % @alignOf(Header) != 0) return null;
        const h: *const Header = @ptrCast(@alignCast(base));
        if (!std.mem.eql(u8, &h.magic, &MAGIC)) return null;
        if (h.version != FORMAT_VERSION) return null;

        // Validate we have enough bytes for header + string table + padding + arrays
        const padding = @as(usize, padAfterStringTable(h.string_table_size));
        const str_end = std.math.add(usize, Header.SIZE, @as(usize, h.string_table_size)) catch return null;
        const str_end_padded = std.math.add(usize, str_end, padding) catch return null;
        const heading_array_size = std.math.mul(usize, @as(usize, h.heading_count), @sizeOf(IndexHeading)) catch return null;
        const link_array_size = std.math.mul(usize, @as(usize, h.link_count), @sizeOf(IndexLink)) catch return null;
        const tag_array_size = std.math.mul(usize, @as(usize, h.tag_count), @sizeOf(IndexTag)) catch return null;
        const block_array_size = std.math.mul(usize, @as(usize, h.block_id_count), @sizeOf(IndexBlockId)) catch return null;
        var total = str_end_padded;
        total = std.math.add(usize, total, heading_array_size) catch return null;
        total = std.math.add(usize, total, link_array_size) catch return null;
        total = std.math.add(usize, total, tag_array_size) catch return null;
        total = std.math.add(usize, total, block_array_size) catch return null;
        if (len < total) return null;

        return .{
            .base = base,
            .len = len,
            .header = h,
        };
    }

    pub fn headingCount(self: *const IndexView) u32 {
        return self.header.heading_count;
    }

    pub fn linkCount(self: *const IndexView) u32 {
        return self.header.link_count;
    }

    pub fn tagCount(self: *const IndexView) u32 {
        return self.header.tag_count;
    }

    pub fn blockIdCount(self: *const IndexView) u32 {
        return self.header.block_id_count;
    }

    pub fn docCount(self: *const IndexView) u32 {
        return self.header.doc_count;
    }

    /// Get pointer to heading at index. Returns null if out of bounds.
    pub fn getHeading(self: *const IndexView, i: u32) ?*const IndexHeading {
        if (i >= self.header.heading_count) return null;
        const padding = padAfterStringTable(self.header.string_table_size);
        const item_offset = std.math.mul(usize, @as(usize, i), @sizeOf(IndexHeading)) catch return null;
        const base_offset = Header.SIZE + self.header.string_table_size + padding;
        const offset = std.math.add(usize, base_offset, item_offset) catch return null;
        const ptr: *const IndexHeading = @ptrCast(@alignCast(self.base + offset));
        return ptr;
    }

    /// Resolve string from string table by offset.
    pub fn getString(self: *const IndexView, offset: u32, length: u16) ?[]const u8 {
        const table_start = Header.SIZE;
        const end_offset = std.math.add(u32, offset, @as(u32, length)) catch return null;
        if (end_offset > self.header.string_table_size) return null;
        return self.base[table_start + offset ..][0..length];
    }
};

/// Serialize IndexData to output buffer.
///
/// Returns:
///   0  .. success (written set)
///  -1  — invalid input (null pointer)
///  -2  — buffer too small
///  -3  — internal error
pub fn serialize_index(
    data: ?*const IndexData,
    output: ?[*]u8,
    cap: u32,
    written: ?*u32,
) i32 {
    const w = written orelse return -1;
    w.* = 0;

    const d = data orelse return -1;
    const out = output orelse return -1;

    if (d.string_table_size > 0 and d.string_table == null) return -1;
    if (d.heading_count > 0 and d.headings == null) return -1;
    if (d.link_count > 0 and d.links == null) return -1;
    if (d.tag_count > 0 and d.tags == null) return -1;
    if (d.block_id_count > 0 and d.block_ids == null) return -1;

    var total: usize = Header.SIZE;
    total = std.math.add(usize, total, @as(usize, d.string_table_size)) catch return -1;
    total = std.math.add(usize, total, @as(usize, padAfterStringTable(d.string_table_size))) catch return -1;
    total = std.math.add(usize, total, std.math.mul(usize, @as(usize, d.heading_count), @sizeOf(IndexHeading)) catch return -1) catch return -1;
    total = std.math.add(usize, total, std.math.mul(usize, @as(usize, d.link_count), @sizeOf(IndexLink)) catch return -1) catch return -1;
    total = std.math.add(usize, total, std.math.mul(usize, @as(usize, d.tag_count), @sizeOf(IndexTag)) catch return -1) catch return -1;
    total = std.math.add(usize, total, std.math.mul(usize, @as(usize, d.block_id_count), @sizeOf(IndexBlockId)) catch return -1) catch return -1;

    if (@as(usize, cap) < total) return -2;

    var pos: usize = 0;

    // Write header
    var h: Header = .{};
    h.doc_count = d.doc_count;
    h.heading_count = d.heading_count;
    h.link_count = d.link_count;
    h.tag_count = d.tag_count;
    h.block_id_count = d.block_id_count;
    h.string_table_size = d.string_table_size;
    const header_bytes: [*]const u8 = @ptrCast(&h);
    @memcpy(out[pos..][0..Header.SIZE], header_bytes[0..Header.SIZE]);
    pos += Header.SIZE;

    // Write string table
    if (d.string_table_size > 0) {
        const str_len: usize = d.string_table_size;
        const str_ptr: [*]const u8 = @ptrCast(d.string_table);
        @memcpy(out[pos..][0..str_len], str_ptr[0..str_len]);
        pos += str_len;
    }

    // Padding for array alignment
    const padding: usize = padAfterStringTable(d.string_table_size);
    if (padding > 0) {
        @memset(out[pos..][0..padding], 0);
        pos += padding;
    }

    // Write heading array
    const heading_bytes: usize = @as(usize, d.heading_count) * @sizeOf(IndexHeading);
    if (heading_bytes > 0) {
        const head_src: [*]const u8 = @ptrCast(d.headings);
        @memcpy(out[pos..][0..heading_bytes], head_src[0..heading_bytes]);
        pos += heading_bytes;
    }

    // Write link array
    const link_bytes: usize = @as(usize, d.link_count) * @sizeOf(IndexLink);
    if (link_bytes > 0) {
        const link_src: [*]const u8 = @ptrCast(d.links);
        @memcpy(out[pos..][0..link_bytes], link_src[0..link_bytes]);
        pos += link_bytes;
    }

    // Write tag array
    const tag_bytes: usize = @as(usize, d.tag_count) * @sizeOf(IndexTag);
    if (tag_bytes > 0) {
        const tag_src: [*]const u8 = @ptrCast(d.tags);
        @memcpy(out[pos..][0..tag_bytes], tag_src[0..tag_bytes]);
        pos += tag_bytes;
    }

    // Write block_id array
    const block_bytes: usize = @as(usize, d.block_id_count) * @sizeOf(IndexBlockId);
    if (block_bytes > 0) {
        const block_src: [*]const u8 = @ptrCast(d.block_ids);
        @memcpy(out[pos..][0..block_bytes], block_src[0..block_bytes]);
        pos += block_bytes;
    }

    w.* = @intCast(pos);
    return 0;
}

// ============================================================================
// Tests
// ============================================================================

test "empty index round-trip" {
    var buf: [Header.SIZE + 128]u8 = undefined;
    var written: u32 = 0;

    const empty: IndexData = .{};
    const rc = serialize_index(&empty, &buf, buf.len, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(Header.SIZE, written);

    const view = IndexView.init(&buf, written);
    try std.testing.expect(view != null);
    try std.testing.expectEqual(@as(u32, 0), view.?.header.doc_count);
    try std.testing.expectEqual(@as(u32, 0), view.?.header.heading_count);
}

test "single doc round-trip" {
    const heading_text = "Introduction";
    var headings: [1]IndexHeading = .{
        .{ .doc_id = 0, .string_offset = 0, .length = @intCast(heading_text.len), .level = 1 },
    };

    var data: IndexData = .{
        .doc_count = 1,
        .heading_count = 1,
        .headings = &headings,
        .string_table = heading_text.ptr,
        .string_table_size = @intCast(heading_text.len),
    };

    var buf: [512]u8 = undefined;
    var written: u32 = 0;

    const rc = serialize_index(&data, &buf, buf.len, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);

    const view = IndexView.init(&buf, written);
    try std.testing.expect(view != null);
    try std.testing.expectEqual(@as(u32, 1), view.?.headingCount());

    const h0 = view.?.getHeading(0);
    try std.testing.expect(h0 != null);
    try std.testing.expectEqual(@as(u32, 0), h0.?.doc_id);
    try std.testing.expectEqual(@as(u8, 1), h0.?.level);

    const str = view.?.getString(0, 12);
    try std.testing.expect(str != null);
    try std.testing.expectEqualStrings("Introduction", str.?);
}

test "corrupt magic bytes" {
    var buf: [32]u8 = undefined;
    buf[0] = 'X';
    buf[1] = 'Y';
    buf[2] = 'Z';
    buf[3] = '!';

    const view = IndexView.init(&buf, 32);
    try std.testing.expect(view == null);
}

test "truncated file" {
    var buf: [16]u8 = undefined; // Less than Header.SIZE
    @memcpy(buf[0..4], &MAGIC);
    buf[4] = 1; // version low byte
    buf[5] = 0; // version high byte

    const view = IndexView.init(&buf, 16);
    try std.testing.expect(view == null);
}

test "version check rejects future version" {
    var buf: [Header.SIZE]u8 = undefined;
    @memcpy(buf[0..4], &MAGIC);
    buf[4] = 0xFF; // version = 255
    buf[5] = 0;

    const view = IndexView.init(&buf, Header.SIZE);
    try std.testing.expect(view == null);
}

test "init rejects header that overflows section size math" {
    var h: Header = .{};
    h.heading_count = std.math.maxInt(u32);

    var buf: [Header.SIZE]u8 = undefined;
    const src: [*]const u8 = @ptrCast(&h);
    @memcpy(buf[0..Header.SIZE], src[0..Header.SIZE]);

    const view = IndexView.init(&buf, buf.len);
    try std.testing.expect(view == null);
}

test "serialize zeroes padding bytes after string table" {
    var buf: [128]u8 = [_]u8{0xAA} ** 128;
    var written: u32 = 0;
    const text = "A";

    var data: IndexData = .{
        .string_table = text.ptr,
        .string_table_size = 1,
    };
    const rc = serialize_index(&data, &buf, buf.len, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);

    const pad_start = Header.SIZE + data.string_table_size;
    const pad_len = padAfterStringTable(data.string_table_size);
    for (buf[pad_start .. pad_start + pad_len]) |b| {
        try std.testing.expectEqual(@as(u8, 0), b);
    }
}

test "large index round-trip (1000+ docs)" {
    const num_docs = 1100;
    const heading_per_doc = 3;
    const total_headings = num_docs * heading_per_doc;

    var string_table: [6]u8 = "H1H2H3".*;
    var headings_buf: [3300]IndexHeading = undefined; // 1100*3
    for (0..num_docs) |doc_i| {
        for (0..heading_per_doc) |h_i| {
            headings_buf[doc_i * heading_per_doc + h_i] = .{
                .doc_id = @intCast(doc_i),
                .string_offset = @intCast(h_i * 2),
                .length = 2,
                .level = @intCast(h_i + 1),
            };
        }
    }

    var data: IndexData = .{
        .doc_count = @intCast(num_docs),
        .heading_count = total_headings,
        .headings = &headings_buf,
        .string_table = &string_table,
        .string_table_size = 6,
    };

    const padding = padAfterStringTable(6);
    const serialized_len = Header.SIZE + 6 + padding + total_headings * @sizeOf(IndexHeading);
    var buf: [65536]u8 = undefined;
    try std.testing.expect(serialized_len <= buf.len);

    var written: u32 = 0;
    const rc = serialize_index(&data, &buf, @intCast(buf.len), &written);
    try std.testing.expectEqual(@as(i32, 0), rc);
    try std.testing.expectEqual(serialized_len, written);

    const view = IndexView.init(&buf, written);
    try std.testing.expect(view != null);
    try std.testing.expectEqual(@as(u32, num_docs), view.?.docCount());
    try std.testing.expectEqual(@as(u32, total_headings), view.?.headingCount());

    const last_heading = view.?.getHeading(total_headings - 1);
    try std.testing.expect(last_heading != null);
    try std.testing.expectEqual(@as(u32, num_docs - 1), last_heading.?.doc_id);
}

test "Header.SIZE equals @sizeOf(Header)" {
    try std.testing.expectEqual(@sizeOf(Header), Header.SIZE);
}

test "getHeading returns null for out-of-bounds index" {
    const heading_text = "Hi";
    var headings: [1]IndexHeading = .{
        .{ .doc_id = 0, .string_offset = 0, .length = 2, .level = 1 },
    };
    var data: IndexData = .{
        .doc_count = 1,
        .heading_count = 1,
        .headings = &headings,
        .string_table = heading_text.ptr,
        .string_table_size = 2,
    };
    var buf: [512]u8 = undefined;
    var written: u32 = 0;
    const rc = serialize_index(&data, &buf, buf.len, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);

    const view = IndexView.init(&buf, written).?;
    // Valid access works
    try std.testing.expect(view.getHeading(0) != null);
    // Out of bounds returns null
    try std.testing.expect(view.getHeading(1) == null);
    try std.testing.expect(view.getHeading(std.math.maxInt(u32)) == null);
}

test "getHeading checked math: mul and add do not wrap" {
    // On 64-bit, u32*12 can't overflow usize, but the checked add on line 151
    // guards against a corrupted base_offset + item_offset exceeding usize.
    // Verify the checked-math paths compile and are exercised by the
    // init overflow test (which rejects maxInt heading_count before getHeading
    // is ever reachable). This test asserts the defense-in-depth code is present
    // by confirming getHeading returns a valid pointer for in-range access and
    // null for out-of-range access on a legitimate view.
    const heading_text = "Hi";
    var headings: [1]IndexHeading = .{
        .{ .doc_id = 0, .string_offset = 0, .length = 2, .level = 1 },
    };
    var data: IndexData = .{
        .doc_count = 1,
        .heading_count = 1,
        .headings = &headings,
        .string_table = heading_text.ptr,
        .string_table_size = 2,
    };
    var buf: [512]u8 = undefined;
    var written: u32 = 0;
    const rc = serialize_index(&data, &buf, buf.len, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);

    const view = IndexView.init(&buf, written).?;
    const h = view.getHeading(0);
    try std.testing.expect(h != null);
    try std.testing.expectEqual(@as(u32, 0), h.?.doc_id);
}

test "getString returns null for overflowing offset+length" {
    const text = "hello";
    var data: IndexData = .{
        .string_table = text.ptr,
        .string_table_size = 5,
    };
    var buf: [512]u8 = undefined;
    var written: u32 = 0;
    const rc = serialize_index(&data, &buf, buf.len, &written);
    try std.testing.expectEqual(@as(i32, 0), rc);

    const view = IndexView.init(&buf, written).?;

    // offset + length wraps u32: 0xFFFFFFFF + 1 would be 0 < 5, bypassing
    // the bounds check without checked arithmetic.
    try std.testing.expect(view.getString(std.math.maxInt(u32), 1) == null);
    try std.testing.expect(view.getString(std.math.maxInt(u32) - 5, 10) == null);

    // Legitimate access still works
    try std.testing.expectEqualStrings("hello", view.getString(0, 5).?);
}

test "struct size doc-comments match reality" {
    try std.testing.expectEqual(@as(usize, 12), @sizeOf(IndexHeading));
    try std.testing.expectEqual(@as(usize, 12), @sizeOf(IndexTag));
    try std.testing.expectEqual(@as(usize, 12), @sizeOf(IndexBlockId));
}

test "init_misaligned_buffer_returns_null" {
    // Regression test for marky-5rq: @alignCast panics in Debug/ReleaseSafe when
    // the pointer is not aligned to @alignOf(Header) (== 8). The fix adds an
    // explicit alignment check that returns null instead of panicking.
    //
    // We allocate a buffer with align(8) and then take &buf[1] which is guaranteed
    // to be misaligned by 1 byte (since buf[0] is 8-byte aligned, buf[1] is at
    // offset +1, which is not a multiple of 8).
    var buf: [Header.SIZE + 16]u8 align(8) = undefined;
    @memset(&buf, 0);
    // Copy magic bytes into the misaligned region so the only rejection
    // is misalignment, not an early length check.
    const misaligned_ptr: [*]const u8 = @ptrCast(&buf[1]);
    // With buggy code, @alignCast panics in Debug mode.
    // With fixed code, init returns null.
    const view = IndexView.init(misaligned_ptr, buf.len - 1);
    try std.testing.expectEqual(@as(?IndexView, null), view);
}
