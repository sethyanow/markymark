//! Binary index serialization FFI bindings.
//!
//! Low-level bindings to the Zig index_serde kernel. For mmap-backed usage,
//! the caller mmaps the file, passes the pointer to `index_deserialize`, and
//! must call `index_destroy` when done (frees the descriptor only; caller unmaps).

use std::ffi::c_void;

/// Opaque handle from deserialize. Caller owns the underlying buffer.
pub type IndexHandle = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IndexHeading {
    pub doc_id: u32,
    pub string_offset: u32,
    pub length: u16,
    pub level: u8,
    _pad: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IndexLink {
    pub doc_id: u32,
    pub text_offset: u32,
    pub text_length: u16,
    pub target_offset: u32,
    pub target_length: u16,
    pub link_type: u8,
    _pad: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IndexTag {
    pub doc_id: u32,
    pub string_offset: u32,
    pub length: u16,
    _pad: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IndexBlockId {
    pub doc_id: u32,
    pub string_offset: u32,
    pub length: u16,
    _pad: u16,
}

#[repr(C)]
pub struct IndexData {
    pub doc_count: u32,
    pub heading_count: u32,
    pub link_count: u32,
    pub tag_count: u32,
    pub block_id_count: u32,
    pub headings: *const IndexHeading,
    pub links: *const IndexLink,
    pub tags: *const IndexTag,
    pub block_ids: *const IndexBlockId,
    pub string_table: *const u8,
    pub string_table_size: u32,
}

extern "C" {
    pub fn marky_index_serialize(
        data: *const IndexData,
        output: *mut u8,
        cap: u32,
        written: *mut u32,
    ) -> i32;

    pub fn marky_index_deserialize(buf: *const u8, len: u32) -> IndexHandle;

    pub fn marky_index_destroy(handle: IndexHandle);

    pub fn marky_index_heading_count(handle: IndexHandle) -> u32;
    pub fn marky_index_link_count(handle: IndexHandle) -> u32;
    pub fn marky_index_tag_count(handle: IndexHandle) -> u32;
    pub fn marky_index_block_id_count(handle: IndexHandle) -> u32;
    pub fn marky_index_doc_count(handle: IndexHandle) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_serialize_empty() {
        let mut buf = [0u8; 128];
        let mut written = 0u32;
        let empty = IndexData {
            doc_count: 0,
            heading_count: 0,
            link_count: 0,
            tag_count: 0,
            block_id_count: 0,
            headings: std::ptr::null(),
            links: std::ptr::null(),
            tags: std::ptr::null(),
            block_ids: std::ptr::null(),
            string_table: std::ptr::null(),
            string_table_size: 0,
        };
        // SAFETY: All pointers are derived from live stack variables (`empty`, `buf`, `written`)
        // that outlive this call. `&empty` is a valid `*const IndexData` with all array pointers
        // set to null and all counts set to 0, which the Zig kernel accepts as a valid empty
        // dataset. `buf.as_mut_ptr()` is valid for 128 bytes. `&mut written` is a valid output
        // pointer. The kernel writes at most `cap` bytes to the output buffer.
        let rc = unsafe { marky_index_serialize(&empty, buf.as_mut_ptr(), 128, &mut written) };
        assert_eq!(rc, 0);
        assert_eq!(written, 36); // Header.SIZE (4-byte magic + 2-byte version + 2-byte flags + 5×u32 counts + u32 string_table_size + 4-byte _reserved)
    }

    #[test]
    fn test_index_round_trip() {
        let heading_text = b"Hello";
        let headings = [IndexHeading {
            doc_id: 0,
            string_offset: 0,
            length: 5,
            level: 1,
            _pad: 0,
        }];
        let data = IndexData {
            doc_count: 1,
            heading_count: 1,
            link_count: 0,
            tag_count: 0,
            block_id_count: 0,
            headings: headings.as_ptr(),
            links: std::ptr::null(),
            tags: std::ptr::null(),
            block_ids: std::ptr::null(),
            string_table: heading_text.as_ptr(),
            string_table_size: 5,
        };

        let mut buf = [0u8; 256];
        let mut written = 0u32;
        // SAFETY: `&data` is a valid `*const IndexData` whose `headings` pointer references the
        // live `headings` array (1 element, matching `heading_count=1`), and `string_table`
        // references the live `heading_text` byte array (5 bytes, matching `string_table_size=5`).
        // All other array pointers are null with corresponding counts of 0. `buf` provides 256
        // bytes of output capacity. `&mut written` is a valid output pointer. All referenced data
        // lives on the stack and outlives this FFI call.
        let rc = unsafe { marky_index_serialize(&data, buf.as_mut_ptr(), 256, &mut written) };
        assert_eq!(rc, 0);

        // SAFETY: `buf.as_ptr()` is a valid pointer to `written` bytes of data that was just
        // produced by a successful `marky_index_serialize` call (rc == 0), so the buffer contains
        // a well-formed binary index with valid magic number, version, and internal offsets.
        // The buffer lives on the stack and outlives this call. The returned handle owns a
        // heap-allocated descriptor (not the buffer itself); the caller must call
        // `marky_index_destroy` to free it.
        let h = unsafe { marky_index_deserialize(buf.as_ptr(), written) };
        assert!(!h.is_null());
        // SAFETY: `h` is a non-null handle returned by a successful `marky_index_deserialize`
        // call (verified by the `assert!(!h.is_null())` above). The handle has not been
        // destroyed yet, so the query functions (`marky_index_heading_count`,
        // `marky_index_doc_count`) read from a valid descriptor. `marky_index_destroy` is
        // called exactly once at the end, transferring ownership back to the Zig allocator
        // which frees the descriptor. No further use of `h` occurs after destruction.
        unsafe {
            assert_eq!(marky_index_heading_count(h), 1);
            assert_eq!(marky_index_doc_count(h), 1);
            marky_index_destroy(h);
        }
    }

    #[test]
    fn test_index_serialize_rejects_null_headings_with_count() {
        let mut buf = [0u8; 128];
        let mut written = 0u32;
        let bad = IndexData {
            doc_count: 0,
            heading_count: 1,
            link_count: 0,
            tag_count: 0,
            block_id_count: 0,
            headings: std::ptr::null(),
            links: std::ptr::null(),
            tags: std::ptr::null(),
            block_ids: std::ptr::null(),
            string_table: std::ptr::null(),
            string_table_size: 0,
        };

        // SAFETY: `&bad` is a valid `*const IndexData` pointer to a stack-allocated struct.
        // Although `heading_count=1` with `headings=null` is an invalid combination, the Zig
        // kernel is designed to detect this and return -1 (error) without dereferencing the
        // null pointer. `buf.as_mut_ptr()` is valid for 128 bytes and `&mut written` is a
        // valid output pointer. All data outlives this call.
        let rc = unsafe { marky_index_serialize(&bad, buf.as_mut_ptr(), 128, &mut written) };
        assert_eq!(rc, -1);
        assert_eq!(written, 0);
    }
}
