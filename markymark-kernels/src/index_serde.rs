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
        let rc = unsafe { marky_index_serialize(&empty, buf.as_mut_ptr(), 128, &mut written) };
        assert_eq!(rc, 0);
        assert_eq!(written, 36); // Header.SIZE (4-byte magic + 2-byte version + 2-byte flags + 5*u32 counts + u32 string_table_size + 4-byte reserved)
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
        let rc = unsafe { marky_index_serialize(&data, buf.as_mut_ptr(), 256, &mut written) };
        assert_eq!(rc, 0);

        let h = unsafe { marky_index_deserialize(buf.as_ptr(), written) };
        assert!(!h.is_null());
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

        let rc = unsafe { marky_index_serialize(&bad, buf.as_mut_ptr(), 128, &mut written) };
        assert_eq!(rc, -1);
        assert_eq!(written, 0);
    }
}
