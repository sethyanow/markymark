// Blob format for DocumentEngine serialization.
//
// Flat binary format: header + packed struct arrays + contiguous text pool.
// mmap-compatible layout, zero pointer chasing.
//
// Section order (fixed, offsets computed from counts):
//   [ScanBlobHeader]             (v2: 128 bytes)
//   [BlobHeading × heading_count]
//   [BlobLink × link_count]
//   [BlobTag × tag_count]
//   [BlobBlockId × block_id_count]
//   [BlobCodeSpan × code_span_count]
//   [u32 × line_count]           (line_starts)
//   [u8 × text_pool_size]        (text pool)

const std = @import("std");

pub const BLOB_MAGIC: u32 = 0x4D4B5343; // "MKSC" (MarKy SCan)
pub const BLOB_VERSION: u16 = 2;

// ── Blob structs ────────────────────────────────────────────────────

/// Blob header. Fixed size at the start of every blob.
/// content_hash placed at offset 8 for natural u64 alignment.
pub const ScanBlobHeader = extern struct {
    magic: u32 = BLOB_MAGIC,
    version: u16 = BLOB_VERSION,
    flags: u16 = 0,
    content_hash: u64 = 0,
    heading_count: u32 = 0,
    link_count: u32 = 0,
    tag_count: u32 = 0,
    block_id_count: u32 = 0,
    line_count: u32 = 0,
    text_pool_size: u32 = 0,
    token_estimate: u32 = 0,
    total_blob_size: u32 = 0,
    code_span_count: u32 = 0,
    embed_count: u32 = 0,
    task_count: u32 = 0,
    callout_count: u32 = 0,
    query_block_count: u32 = 0,
    link_def_count: u32 = 0,
    block_ref_count: u32 = 0,
    property_count: u32 = 0,
    xml_tag_count: u32 = 0,
    _reserved_v2: [44]u8 = .{0} ** 44,
};

pub const BlobHeading = extern struct {
    text_off: u32 = 0,
    text_len: u32 = 0,
    slug_off: u32 = 0,
    slug_len: u32 = 0,
    source_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
    level: u8 = 0,
    _pad: [3]u8 = .{ 0, 0, 0 },
};

pub const BlobLink = extern struct {
    text_off: u32 = 0,
    text_len: u32 = 0,
    target_off: u32 = 0,
    target_len: u32 = 0,
    source_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
    is_wiki: u8 = 0,
    _pad: [3]u8 = .{ 0, 0, 0 },
};

pub const BlobTag = extern struct {
    name_off: u32 = 0,
    name_len: u32 = 0,
    source_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    _pad: u32 = 0,
};

pub const BlobBlockId = extern struct {
    id_off: u32 = 0,
    id_len: u32 = 0,
    source_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
};

pub const BlobTask = extern struct {
    text_off: u32 = 0,
    text_len: u32 = 0,
    source_offset: u32 = 0,
    end_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
    state: u8 = 0,
    _pad: [3]u8 = .{ 0, 0, 0 },
};

pub const BlobEmbed = extern struct {
    target_off: u32 = 0,
    target_len: u32 = 0,
    source_offset: u32 = 0,
    end_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
};

pub const BlobCodeSpan = extern struct {
    text_off: u32 = 0,
    text_len: u32 = 0,
    source_offset: u32 = 0, // byte offset of opening backtick
    end_offset: u32 = 0, // byte offset past closing backtick
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
};

pub const BlobCallout = extern struct {
    type_off: u32 = 0, // callout type text offset in text pool
    type_len: u32 = 0,
    title_off: u32 = 0, // title offset (0/0 sentinel for None)
    title_len: u32 = 0,
    source_offset: u32 = 0,
    end_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
};

pub const BlobBlockRef = extern struct {
    uuid_off: u32 = 0, // UUID text offset in text pool
    uuid_len: u32 = 0,
    source_offset: u32 = 0,
    start_line: u32 = 0,
    start_col: u32 = 0,
    end_line: u32 = 0,
    end_col: u32 = 0,
};

// ── Comptime size assertions ────────────────────────────────────────

comptime {
    std.debug.assert(@sizeOf(ScanBlobHeader) == 128);
    std.debug.assert(@sizeOf(BlobHeading) == 40);
    std.debug.assert(@sizeOf(BlobLink) == 40);
    std.debug.assert(@sizeOf(BlobTag) == 24);
    std.debug.assert(@sizeOf(BlobBlockId) == 28);
    std.debug.assert(@sizeOf(BlobTask) == 36);
    std.debug.assert(@sizeOf(BlobEmbed) == 32);
    std.debug.assert(@sizeOf(BlobCodeSpan) == 32);
    std.debug.assert(@sizeOf(BlobCallout) == 40);
    std.debug.assert(@sizeOf(BlobBlockRef) == 28);
}

// ── Blob size computation ───────────────────────────────────────────

/// Compute total blob size from section counts. Returns null on u32 overflow.
pub fn computeBlobSize(
    heading_count: u32,
    link_count: u32,
    tag_count: u32,
    block_id_count: u32,
    code_span_count: u32,
    task_count: u32,
    embed_count: u32,
    callout_count: u32,
    block_ref_count: u32,
    line_count: u32,
    text_pool_size: u32,
) ?u32 {
    const size: u64 = @as(u64, @sizeOf(ScanBlobHeader)) +
        @as(u64, heading_count) * @sizeOf(BlobHeading) +
        @as(u64, link_count) * @sizeOf(BlobLink) +
        @as(u64, tag_count) * @sizeOf(BlobTag) +
        @as(u64, block_id_count) * @sizeOf(BlobBlockId) +
        @as(u64, code_span_count) * @sizeOf(BlobCodeSpan) +
        @as(u64, task_count) * @sizeOf(BlobTask) +
        @as(u64, embed_count) * @sizeOf(BlobEmbed) +
        @as(u64, callout_count) * @sizeOf(BlobCallout) +
        @as(u64, block_ref_count) * @sizeOf(BlobBlockRef) +
        @as(u64, line_count) * @sizeOf(u32) +
        @as(u64, text_pool_size);

    if (size > std.math.maxInt(u32)) return null;
    return @intCast(size);
}

/// Compute offset of each section within the blob.
pub const SectionOffsets = struct {
    headings: u32,
    links: u32,
    tags: u32,
    block_ids: u32,
    code_spans: u32,
    tasks: u32,
    embeds: u32,
    callouts: u32,
    block_refs: u32,
    line_starts: u32,
    text_pool: u32,
};

/// Compute byte offset of each section within a blob.
///
/// Returns null if the header counts would overflow u32 arithmetic (defense-in-depth
/// for release builds where the debug assertion in the old signature was stripped).
/// Callers with validated headers should use `.?` or `orelse unreachable`; production
/// code should propagate the error with `orelse return error.OutOfMemory`.
pub fn computeSectionOffsets(header: ScanBlobHeader) ?SectionOffsets {
    if (computeBlobSize(
        header.heading_count,
        header.link_count,
        header.tag_count,
        header.block_id_count,
        header.code_span_count,
        header.task_count,
        header.embed_count,
        header.callout_count,
        header.block_ref_count,
        header.line_count,
        header.text_pool_size,
    ) == null) return null;
    const base: u32 = @sizeOf(ScanBlobHeader);
    const headings = base;
    const links = headings + header.heading_count * @sizeOf(BlobHeading);
    const tags = links + header.link_count * @sizeOf(BlobLink);
    const block_ids = tags + header.tag_count * @sizeOf(BlobTag);
    const code_spans = block_ids + header.block_id_count * @sizeOf(BlobBlockId);
    const tasks = code_spans + header.code_span_count * @sizeOf(BlobCodeSpan);
    const embeds = tasks + header.task_count * @sizeOf(BlobTask);
    const callouts = embeds + header.embed_count * @sizeOf(BlobEmbed);
    const block_refs = callouts + header.callout_count * @sizeOf(BlobCallout);
    const line_starts = block_refs + header.block_ref_count * @sizeOf(BlobBlockRef);
    const text_pool = line_starts + header.line_count * @sizeOf(u32);
    return .{
        .headings = headings,
        .links = links,
        .tags = tags,
        .block_ids = block_ids,
        .code_spans = code_spans,
        .tasks = tasks,
        .embeds = embeds,
        .callouts = callouts,
        .block_refs = block_refs,
        .line_starts = line_starts,
        .text_pool = text_pool,
    };
}

// ── Blob validation ─────────────────────────────────────────────────

pub const BlobError = error{
    InvalidMagic,
    UnsupportedVersion,
    BlobTooSmall,
    SizeMismatch,
    OutOfRange,
};

/// Validate a blob's header and size consistency. Returns the header on success.
pub fn validateBlob(data: []const u8) BlobError!ScanBlobHeader {
    if (data.len < @sizeOf(ScanBlobHeader)) return error.BlobTooSmall;

    // Read header via memcpy (alignment-safe)
    const header = readHeader(data);

    if (header.magic != BLOB_MAGIC) return error.InvalidMagic;
    if (header.version != BLOB_VERSION) return error.UnsupportedVersion;

    // Validate total size
    const expected = computeBlobSize(
        header.heading_count,
        header.link_count,
        header.tag_count,
        header.block_id_count,
        header.code_span_count,
        header.task_count,
        header.embed_count,
        header.callout_count,
        header.block_ref_count,
        header.line_count,
        header.text_pool_size,
    ) orelse return error.OutOfRange;

    if (expected != header.total_blob_size) return error.SizeMismatch;
    if (expected != data.len) return error.SizeMismatch;

    return header;
}

/// Read header from raw bytes (alignment-safe bytewise copy).
///
/// Precondition: `data.len >= @sizeOf(ScanBlobHeader)` (128 bytes for v2).
/// Panics via Zig slice bounds check on undersized input.
/// Callers should use `validateBlob()` first, which enforces the minimum size.
pub fn readHeader(data: []const u8) ScanBlobHeader {
    var header: ScanBlobHeader = undefined;
    const dst: [*]u8 = @ptrCast(&header);
    @memcpy(dst[0..@sizeOf(ScanBlobHeader)], data[0..@sizeOf(ScanBlobHeader)]);
    return header;
}

/// Write header to raw bytes (alignment-safe bytewise copy).
///
/// Precondition: `data.len >= @sizeOf(ScanBlobHeader)` (128 bytes for v2).
/// Panics via Zig slice bounds check on undersized input.
pub fn writeHeader(data: []u8, header: ScanBlobHeader) void {
    const src: [*]const u8 = @ptrCast(&header);
    @memcpy(data[0..@sizeOf(ScanBlobHeader)], src[0..@sizeOf(ScanBlobHeader)]);
}

/// Write a struct into the blob buffer at a given byte offset.
/// Returns `error.OutOfRange` if the struct does not fit in the buffer.
pub fn writeStruct(comptime T: type, buf: []u8, offset: usize, value: T) !void {
    if (offset + @sizeOf(T) > buf.len) return error.OutOfRange;
    const src: [*]const u8 = @ptrCast(&value);
    @memcpy(buf[offset..][0..@sizeOf(T)], src[0..@sizeOf(T)]);
}

/// Read a struct from the blob buffer at a given byte offset.
/// Returns `error.OutOfRange` if the struct does not fit in the buffer.
pub fn readStruct(comptime T: type, buf: []const u8, offset: usize) !T {
    if (offset + @sizeOf(T) > buf.len) return error.OutOfRange;
    var result: T = undefined;
    const dst: [*]u8 = @ptrCast(&result);
    @memcpy(dst[0..@sizeOf(T)], buf[offset..][0..@sizeOf(T)]);
    return result;
}

// ── Tests ───────────────────────────────────────────────────────────

const testing = std.testing;

test "comptime size assertions hold" {
    try testing.expectEqual(@as(usize, 128), @sizeOf(ScanBlobHeader));
    try testing.expectEqual(@as(usize, 40), @sizeOf(BlobHeading));
    try testing.expectEqual(@as(usize, 40), @sizeOf(BlobLink));
    try testing.expectEqual(@as(usize, 24), @sizeOf(BlobTag));
    try testing.expectEqual(@as(usize, 28), @sizeOf(BlobBlockId));
    try testing.expectEqual(@as(usize, 36), @sizeOf(BlobTask));
    try testing.expectEqual(@as(usize, 32), @sizeOf(BlobEmbed));
    try testing.expectEqual(@as(usize, 32), @sizeOf(BlobCodeSpan));
}

test "v2 header includes all planned count fields" {
    try testing.expect(@hasField(ScanBlobHeader, "embed_count"));
    try testing.expect(@hasField(ScanBlobHeader, "task_count"));
    try testing.expect(@hasField(ScanBlobHeader, "callout_count"));
    try testing.expect(@hasField(ScanBlobHeader, "query_block_count"));
    try testing.expect(@hasField(ScanBlobHeader, "link_def_count"));
    try testing.expect(@hasField(ScanBlobHeader, "block_ref_count"));
    try testing.expect(@hasField(ScanBlobHeader, "property_count"));
    try testing.expect(@hasField(ScanBlobHeader, "xml_tag_count"));
}

test "computeBlobSize empty document" {
    const size = computeBlobSize(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    try testing.expectEqual(@as(?u32, 128), size);
}

test "computeBlobSize with counts" {
    // 1 heading (40) + 1 link (40) + 1 tag (24) + 1 block_id (28) + 0 code_spans + 0 tasks + 0 embeds + 0 callouts + 0 block_refs + 2 lines (8) + 10 text
    const size = computeBlobSize(1, 1, 1, 1, 0, 0, 0, 0, 0, 2, 10);
    const expected: u32 = 128 + 40 + 40 + 24 + 28 + 8 + 10;
    try testing.expectEqual(@as(?u32, expected), size);
}

test "computeBlobSize with code spans" {
    // 1 heading (40) + 1 link (40) + 1 tag (24) + 1 block_id (28) + 2 code_spans (64) + 0 tasks + 0 embeds + 0 callouts + 0 block_refs + 2 lines (8) + 10 text
    const size = computeBlobSize(1, 1, 1, 1, 2, 0, 0, 0, 0, 2, 10);
    const expected: u32 = 128 + 40 + 40 + 24 + 28 + 64 + 8 + 10;
    try testing.expectEqual(@as(?u32, expected), size);
}

test "computeBlobSize with tasks and embeds" {
    // 0 headings + 0 links + 0 tags + 0 block_ids + 0 code_spans + 1 task (36) + 1 embed (32) + 0 callouts + 0 block_refs + 0 lines + 5 text
    const size = computeBlobSize(0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 5);
    const expected: u32 = 128 + 36 + 32 + 5;
    try testing.expectEqual(@as(?u32, expected), size);
}

test "computeBlobSize with callouts and block refs" {
    // 0 headings + 0 links + 0 tags + 0 block_ids + 0 code_spans + 0 tasks + 0 embeds + 1 callout (40) + 1 block_ref (28) + 0 lines + 10 text
    const size = computeBlobSize(0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 10);
    const expected: u32 = 128 + 40 + 28 + 10;
    try testing.expectEqual(@as(?u32, expected), size);
}

test "computeBlobSize overflow returns null" {
    const size = computeBlobSize(std.math.maxInt(u32), 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    try testing.expectEqual(@as(?u32, null), size);
}

test "validateBlob rejects too small" {
    const data = [_]u8{0} ** 32;
    try testing.expectError(error.BlobTooSmall, validateBlob(&data));
}

test "validateBlob rejects bad magic" {
    var buf: [128]u8 = .{0} ** 128;
    // Write wrong magic
    std.mem.writeInt(u32, buf[0..4], 0xDEADBEEF, .little);
    try testing.expectError(error.InvalidMagic, validateBlob(&buf));
}

test "validateBlob accepts valid empty blob" {
    var header = ScanBlobHeader{};
    header.total_blob_size = @sizeOf(ScanBlobHeader);
    var buf: [128]u8 = undefined;
    writeHeader(&buf, header);
    const validated = try validateBlob(&buf);
    try testing.expectEqual(BLOB_MAGIC, validated.magic);
    try testing.expectEqual(BLOB_VERSION, validated.version);
    try testing.expectEqual(@as(u32, 0), validated.heading_count);
    try testing.expectEqual(@as(u32, 128), validated.total_blob_size);
}

test "validateBlob rejects size mismatch" {
    var header = ScanBlobHeader{};
    header.total_blob_size = 64; // Wrong: actual data is 128 bytes
    var buf: [128]u8 = undefined;
    writeHeader(&buf, header);
    try testing.expectError(error.SizeMismatch, validateBlob(&buf));
}

test "computeSectionOffsets correct for known counts" {
    const header = ScanBlobHeader{
        .heading_count = 2,
        .link_count = 1,
        .tag_count = 3,
        .block_id_count = 1,
        .code_span_count = 0,
        .line_count = 4,
        .text_pool_size = 20,
    };
    const offsets = computeSectionOffsets(header).?;
    try testing.expectEqual(@as(u32, 128), offsets.headings);
    try testing.expectEqual(@as(u32, 128 + 2 * 40), offsets.links);
    try testing.expectEqual(@as(u32, 128 + 2 * 40 + 1 * 40), offsets.tags);
    try testing.expectEqual(@as(u32, 128 + 2 * 40 + 1 * 40 + 3 * 24), offsets.block_ids);
    // code_spans section at block_ids end (0 code spans)
    try testing.expectEqual(offsets.block_ids + 1 * 28, offsets.code_spans);
}

test "computeSectionOffsets with code spans" {
    const header = ScanBlobHeader{
        .heading_count = 1,
        .link_count = 0,
        .tag_count = 0,
        .block_id_count = 0,
        .code_span_count = 2,
        .line_count = 1,
        .text_pool_size = 5,
    };
    const offsets = computeSectionOffsets(header).?;
    try testing.expectEqual(@as(u32, 128), offsets.headings);
    try testing.expectEqual(@as(u32, 128 + 1 * 40), offsets.links);
    try testing.expectEqual(@as(u32, 128 + 1 * 40), offsets.tags);
    try testing.expectEqual(@as(u32, 128 + 1 * 40), offsets.block_ids);
    try testing.expectEqual(@as(u32, 128 + 1 * 40), offsets.code_spans);
    // 2 code spans * 32 bytes each = 64
    try testing.expectEqual(@as(u32, 128 + 1 * 40 + 2 * 32), offsets.line_starts);
}

test "writeStruct and readStruct roundtrip" {
    var buf: [256]u8 = .{0} ** 256;
    const heading = BlobHeading{
        .text_off = 10,
        .text_len = 5,
        .slug_off = 15,
        .slug_len = 5,
        .source_offset = 0,
        .start_line = 0,
        .start_col = 0,
        .end_line = 0,
        .end_col = 7,
        .level = 1,
    };
    try writeStruct(BlobHeading, &buf, 128, heading);
    const read_back = try readStruct(BlobHeading, &buf, 128);
    try testing.expectEqual(heading.text_off, read_back.text_off);
    try testing.expectEqual(heading.text_len, read_back.text_len);
    try testing.expectEqual(heading.level, read_back.level);
    try testing.expectEqual(heading.end_col, read_back.end_col);
}

test "writeStruct returns error on out-of-bounds offset" {
    var buf: [10]u8 = .{0} ** 10;
    // BlobHeading is 40 bytes; offset 5 puts it at bytes 5..45, beyond buf.len=10
    try testing.expectError(error.OutOfRange, writeStruct(BlobHeading, &buf, 5, std.mem.zeroes(BlobHeading)));
}

test "readStruct returns error on out-of-bounds offset" {
    const buf: [10]u8 = .{0} ** 10;
    // BlobHeading is 40 bytes; offset 5 puts it at bytes 5..45, beyond buf.len=10
    try testing.expectError(error.OutOfRange, readStruct(BlobHeading, buf[0..], 5));
}
