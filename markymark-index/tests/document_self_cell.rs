#[test]
fn document_index_internal_storage_no_longer_uses_static_or_ptr_read_transfer() {
    // Read all document module files to check for banned patterns
    let mod_rs = include_str!("../src/document/mod.rs");
    let types_rs = include_str!("../src/document/types.rs");
    let helpers_rs = include_str!("../src/document/helpers.rs");
    let source = format!("{mod_rs}\n{types_rs}\n{helpers_rs}");

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
