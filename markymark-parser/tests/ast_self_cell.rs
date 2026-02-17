#[test]
fn ast_no_longer_uses_static_root_element_storage() {
    let ast_source = include_str!("../src/ast.rs");
    assert!(
        !ast_source.contains("Vec<Element<'static>>"),
        "Ast should not store Element<'static> after self_cell migration"
    );
}
