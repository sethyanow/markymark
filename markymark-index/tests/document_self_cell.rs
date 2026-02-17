#[test]
fn document_index_internal_storage_no_longer_uses_static_or_ptr_read_transfer() {
    let source = include_str!("../src/document/mod.rs");

    assert!(
        !source.contains("headings: &'static [HeadingEntry<'static>]"),
        "DocumentIndex should not store headings as &'static internals",
    );
    assert!(
        !source.contains("slug_to_heading: HashMap<&'static str, usize>"),
        "DocumentIndex should not store slug index keys as &'static internals",
    );
    assert!(
        !source.contains("blocks: HashMap<&'static str, BlockEntry<'static>>"),
        "DocumentIndex should not store blocks as &'static internals",
    );
    assert!(
        !source.contains("std::ptr::read(doc_arena_ptr)"),
        "DocumentIndex::from_ast should not transfer ownership with ptr::read",
    );
    assert!(
        !source.contains("std::mem::forget(ast)"),
        "DocumentIndex::from_ast should not use mem::forget(ast)",
    );
}
