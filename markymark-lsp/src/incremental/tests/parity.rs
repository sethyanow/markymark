use super::super::*;
use markymark_parser::Point;

// ─── Full parity test: incremental must match full rebuild INCLUDING positions ─

#[test]
fn test_incremental_wiki_links_parity_with_positions() {
    // Build a document with wiki links, apply a single-char edit in prose
    // between links, verify incremental produces identical wiki link positions
    // to a full rebuild.
    use markymark_parser::Parser;

    let original = "# Title\n\nSee [[PageA]] for details.\n\nSome prose text here.\n\nAlso check [[PageB]] and [[PageC]].\n";
    let mut parser = Parser::new().unwrap();

    // Initial parse
    let ast0 = parser.parse(original).unwrap();
    let index0 = DocumentIndex::from_ast(ast0);

    // Extract old wiki links
    let old_wiki_links: Vec<WikiLinkOwned> = index0
        .wiki_links()
        .iter()
        .map(|wl| WikiLinkOwned {
            target: wl.target.to_string(),
            alias: wl.alias.map(str::to_string),
            heading: wl.heading.map(str::to_string),
            range: wl.range,
            start_byte: wl.start_byte,
            end_byte: wl.end_byte,
        })
        .collect();

    // Single-char insertion in prose (line 4, col 5: "Some Xprose text here.")
    let insert_byte = original.find("prose").unwrap();
    let modified = format!("{}X{}", &original[..insert_byte], &original[insert_byte..]);

    let edit = InputEdit {
        start_byte: insert_byte,
        old_end_byte: insert_byte,
        new_end_byte: insert_byte + 1,
        start_position: Point { row: 4, column: 5 },
        old_end_position: Point { row: 4, column: 5 },
        new_end_position: Point { row: 4, column: 6 },
    };

    // Full rebuild from modified text
    let ast_full = parser.parse(&modified).unwrap();
    let full_index = DocumentIndex::from_ast(ast_full);
    let full_wiki_links: Vec<_> = full_index
        .wiki_links()
        .iter()
        .map(|wl| (wl.target.to_string(), wl.range, wl.start_byte, wl.end_byte))
        .collect();

    // Incremental rebuild
    let ast_inc = parser.parse(&modified).unwrap();
    let inc_index =
        build_markdown_index_incremental(ast_inc, &[edit], Some(&old_wiki_links), None, None, None);
    let inc_wiki_links: Vec<_> = inc_index
        .wiki_links()
        .iter()
        .map(|wl| (wl.target.to_string(), wl.range, wl.start_byte, wl.end_byte))
        .collect();

    assert_eq!(
        full_wiki_links.len(),
        inc_wiki_links.len(),
        "same number of wiki links: full={} inc={}",
        full_wiki_links.len(),
        inc_wiki_links.len()
    );

    for (i, (full, inc)) in full_wiki_links
        .iter()
        .zip(inc_wiki_links.iter())
        .enumerate()
    {
        assert_eq!(
            full, inc,
            "wiki link {i} mismatch:\n  full: {full:?}\n  inc:  {inc:?}"
        );
    }
}

#[test]
fn test_incremental_blocks_parity_with_positions() {
    // Build a document with block IDs, apply an edit, verify positions match
    use markymark_parser::Parser;

    let original = "# Title\n\nBlock A ^block-a\n\nSome text here.\n\nBlock B ^block-b\n";
    let mut parser = Parser::new().unwrap();

    let ast0 = parser.parse(original).unwrap();
    let index0 = DocumentIndex::from_ast(ast0);

    let old_blocks: Vec<BlockOwned> = index0
        .block_ids()
        .filter_map(|id| index0.block_by_id(id))
        .map(|entry| BlockOwned {
            id: entry.id.to_string(),
            range: entry.range,
            start_byte: entry.start_byte,
            end_byte: entry.end_byte,
        })
        .collect();

    // Insert "X" in "Some text here." (line 4, col 5)
    let insert_byte = original.find("text here").unwrap();
    let modified = format!("{}X{}", &original[..insert_byte], &original[insert_byte..]);

    let edit = InputEdit {
        start_byte: insert_byte,
        old_end_byte: insert_byte,
        new_end_byte: insert_byte + 1,
        start_position: Point { row: 4, column: 5 },
        old_end_position: Point { row: 4, column: 5 },
        new_end_position: Point { row: 4, column: 6 },
    };

    // Full rebuild
    let ast_full = parser.parse(&modified).unwrap();
    let full_index = DocumentIndex::from_ast(ast_full);

    // Incremental
    let ast_inc = parser.parse(&modified).unwrap();
    let inc_index =
        build_markdown_index_incremental(ast_inc, &[edit], None, Some(&old_blocks), None, None);

    // Compare block IDs and positions (sort by ID for stable ordering)
    let mut full_blocks: Vec<_> = full_index
        .block_ids()
        .filter_map(|id| full_index.block_by_id(id))
        .map(|e| (e.id.to_string(), e.range, e.start_byte, e.end_byte))
        .collect();
    let mut inc_blocks: Vec<_> = inc_index
        .block_ids()
        .filter_map(|id| inc_index.block_by_id(id))
        .map(|e| (e.id.to_string(), e.range, e.start_byte, e.end_byte))
        .collect();
    full_blocks.sort_by(|a, b| a.0.cmp(&b.0));
    inc_blocks.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(full_blocks.len(), inc_blocks.len());
    for (full, inc) in full_blocks.iter().zip(inc_blocks.iter()) {
        assert_eq!(
            full, inc,
            "block mismatch:\n  full: {full:?}\n  inc:  {inc:?}"
        );
    }
}
