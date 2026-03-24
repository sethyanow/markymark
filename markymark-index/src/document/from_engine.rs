//! [`DocumentIndex::from_engine_result`] — construct index from CEngineResult conversion.

use bumpalo::collections::Vec as BumpVec;
use hashbrown::HashMap;
use markymark_core::arena::{arena_alloc_str, DocumentArena};
use markymark_core::{Position, Range};
use markymark_kernels::engine::{DocumentEngine, EngineExtraction};

use super::{
    helpers, BlockKind, BlockRefEntry, CalloutEntry, CodeSpanEntry, ContentBlock,
    DocumentDependent, DocumentIndex, DocumentIndexCell, DocumentOwner, EmbedEntry,
    FrontmatterEntry, FrontmatterOwnedEntry, HeadingEntry, LinkDefinitionEntry, MarkdownLinkEntry,
    PropertyEntry, PropertyValueEntry, QueryBlockEntry, TagEntry, TaskEntry, WikiLinkEntry,
    XmlTagEntry,
};

/// Intermediate content block from tree-sitter block-tree parse.
struct RawBlock {
    kind: BlockKind,
    start_byte: usize,
    end_byte: usize,
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
}

/// Extract content blocks from source text via tree-sitter block-tree parsing.
///
/// Parses only the block grammar (no inline parsing). Blocks whose start_byte
/// falls within the frontmatter region are excluded.
fn extract_content_blocks(source: &str) -> Vec<RawBlock> {
    let mut parser = match markymark_parser::Parser::new() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let block_tree = match parser.parse_block_tree_only(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let fm_end = helpers::frontmatter_byte_end(source);
    let root = block_tree.root_node();
    let mut blocks = Vec::new();
    collect_blocks(root, source, fm_end, &mut blocks);
    blocks.sort_by_key(|b| b.start_byte);
    blocks
}

/// Recursively walk tree-sitter nodes collecting content blocks.
fn collect_blocks(
    node: tree_sitter::Node,
    source: &str,
    fm_end: usize,
    blocks: &mut Vec<RawBlock>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.start_byte() < fm_end {
            continue;
        }

        let push = |blocks: &mut Vec<RawBlock>, kind: BlockKind, node: tree_sitter::Node| {
            let sp = node.start_position();
            let ep = node.end_position();
            blocks.push(RawBlock {
                kind,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                start_row: sp.row as u32,
                start_col: sp.column as u32,
                end_row: ep.row as u32,
                end_col: ep.column as u32,
            });
        };

        match child.kind() {
            "section" | "document" => {
                collect_blocks(child, source, fm_end, blocks);
            }
            "paragraph" => push(blocks, BlockKind::Paragraph, child),
            "list" => {
                let mut list_cursor = child.walk();
                for list_child in child.children(&mut list_cursor) {
                    if list_child.kind() == "list_item" {
                        if is_logseq_heading(list_child, source) {
                            continue;
                        }
                        push(blocks, BlockKind::ListItem, list_child);
                    }
                }
            }
            "fenced_code_block" | "indented_code_block" => push(blocks, BlockKind::CodeBlock, child),
            "block_quote" => push(blocks, BlockKind::BlockQuote, child),
            "thematic_break" => push(blocks, BlockKind::ThematicBreak, child),
            "pipe_table" => push(blocks, BlockKind::Table, child),
            _ => {}
        }
    }
}

/// Check if a list_item is a Logseq-style heading (e.g., `- # Heading`).
fn is_logseq_heading(node: tree_sitter::Node, source: &str) -> bool {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let first_line = match text.lines().next() {
        Some(l) => l,
        None => return false,
    };
    let trimmed = first_line.trim_start();
    let after_marker = if let Some(rest) = trimmed.strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        rest
    } else {
        return false;
    };
    after_marker.starts_with('#')
}

impl DocumentIndex {
    /// Build a document index from raw markdown text via an ephemeral engine.
    ///
    /// This is a **test convenience** — it creates a temporary [`DocumentEngine`],
    /// extracts results, and drops the engine. Production code should use
    /// [`from_engine_result_with_frontmatter`] with a persistent engine.
    ///
    /// # Panics
    ///
    /// Panics if the engine fails to create, get results, or convert extraction.
    /// This is intentional — test code should surface failures immediately.
    pub fn from_text(text: &str) -> Self {
        let (fm, aliases) = helpers::parse_frontmatter_owned(text);
        let masked = helpers::mask_frontmatter(text);
        let engine = DocumentEngine::new(&masked).expect("from_text: engine create failed");
        let result = engine.get_result().expect("from_text: get_result failed");
        let extraction = result
            .to_extraction()
            .expect("from_text: to_extraction failed");
        let raw_blocks = extract_content_blocks(text);
        Self::from_engine_result_full(&extraction, fm, aliases, text.to_string(), raw_blocks)
    }

    /// Build a document index from an owned engine extraction.
    pub fn from_engine_result(data: &EngineExtraction) -> Self {
        Self::from_engine_result_inner(data, Vec::new(), Vec::new(), String::new(), Vec::new())
    }

    /// Build a document index from engine extraction with pre-parsed frontmatter.
    pub fn from_engine_result_with_frontmatter(
        data: &EngineExtraction,
        frontmatter: Vec<FrontmatterOwnedEntry>,
        aliases: Vec<String>,
    ) -> Self {
        Self::from_engine_result_inner(data, frontmatter, aliases, String::new(), Vec::new())
    }

    /// Full construction with source text and content blocks (used by `from_text`).
    fn from_engine_result_full(
        data: &EngineExtraction,
        frontmatter: Vec<FrontmatterOwnedEntry>,
        aliases: Vec<String>,
        source_text: String,
        raw_blocks: Vec<RawBlock>,
    ) -> Self {
        Self::from_engine_result_inner(data, frontmatter, aliases, source_text, raw_blocks)
    }

    fn from_engine_result_inner(
        data: &EngineExtraction,
        fm_owned: Vec<FrontmatterOwnedEntry>,
        aliases_owned: Vec<String>,
        source: String,
        raw_blocks: Vec<RawBlock>,
    ) -> Self {
        let owner = DocumentOwner {
            arena: DocumentArena::new(),
            source_text: source,
        };
        let cell = DocumentIndexCell::new(owner, move |owner| {
            let arena_ref = owner.arena.bump();

            // --- Headings ---
            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            for h in &data.headings {
                let text = arena_alloc_str(arena_ref, &h.text);
                let slug = arena_alloc_str(arena_ref, &h.slug);
                let start_pos = Position::new(h.start_line, h.start_col);
                let end_pos = Position::new(h.end_line, h.end_col);
                let idx = headings_builder.len();
                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text,
                    slug,
                    level: h.level,
                    range: Range::new(start_pos, end_pos),
                });
            }
            let headings = headings_builder.into_bump_slice();

            let toc = helpers::build_toc(arena_ref, headings);
            let outline = helpers::build_outline(arena_ref, headings);

            // --- Wiki links ---
            let mut wiki_builder = BumpVec::new_in(arena_ref);
            for wl in &data.wiki_links {
                let target = arena_alloc_str(arena_ref, &wl.target);
                let alias = wl.alias.as_deref().map(|a| arena_alloc_str(arena_ref, a));
                let start_pos = Position::new(wl.start_line, wl.start_col);
                let end_pos = Position::new(wl.end_line, wl.end_col);
                let start_byte = wl.source_offset as usize;
                let end_byte = if wl.alias.is_some() {
                    start_byte + wl.target_len as usize + wl.text_len as usize + 5
                } else {
                    start_byte + wl.target_len as usize + 4
                };
                let heading = wl.heading.as_deref().map(|h| arena_alloc_str(arena_ref, h));
                wiki_builder.push(WikiLinkEntry {
                    target,
                    alias,
                    heading,
                    range: Range::new(start_pos, end_pos),
                    start_byte,
                    end_byte,
                });
            }
            let wiki_links = wiki_builder.into_bump_slice();

            // --- Markdown links ---
            let mut ml_builder = BumpVec::new_in(arena_ref);
            for ml in &data.markdown_links {
                let text = arena_alloc_str(arena_ref, &ml.text);
                let url = arena_alloc_str(arena_ref, &ml.url);
                let anchor = ml.anchor.as_deref().map(|a| arena_alloc_str(arena_ref, a));
                let start_pos = Position::new(ml.start_line, ml.start_col);
                let end_pos = Position::new(ml.end_line, ml.end_col);
                let start_byte = ml.source_offset as usize;
                let end_byte = start_byte + ml.text_len as usize + ml.target_len as usize + 4;
                ml_builder.push(MarkdownLinkEntry {
                    text,
                    url,
                    anchor,
                    range: Range::new(start_pos, end_pos),
                    start_byte,
                    end_byte,
                });
            }
            let markdown_links = ml_builder.into_bump_slice();

            // --- Tags ---
            let mut tags_builder = BumpVec::new_in(arena_ref);
            for t in &data.tags {
                tags_builder.push(TagEntry {
                    name: arena_alloc_str(arena_ref, &t.name),
                });
            }
            let tags = tags_builder.into_bump_slice();

            // --- Block IDs (Obsidian ^block-id markers) ---
            let mut block_id_map: HashMap<&str, ContentBlock<'_>> = HashMap::new();
            for b in &data.block_ids {
                let id = arena_alloc_str(arena_ref, &b.id);
                let start_pos = Position::new(b.start_line, b.start_col);
                let end_pos = Position::new(b.end_line, b.end_col);
                let start_byte = b.source_offset as usize;
                let end_byte = start_byte + 1 + b.id_len as usize;
                block_id_map.insert(
                    id,
                    ContentBlock {
                        kind: BlockKind::Paragraph,
                        range: Range::new(start_pos, end_pos),
                        start_byte,
                        end_byte,
                        parent_heading: None,
                        block_id: Some(id),
                    },
                );
            }

            // --- Content blocks (from tree-sitter block-tree parse) ---
            let mut cb_builder = BumpVec::new_in(arena_ref);
            for rb in &raw_blocks {
                // Parent heading: last heading on a line before this block
                let parent_heading = headings
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, h)| h.range.start.line < rb.start_row)
                    .map(|(i, _)| i);

                // Merge block_id if a ^marker falls within this block's byte range
                let merged_block_id = block_id_map.iter().find_map(|(&id, bid)| {
                    if bid.start_byte >= rb.start_byte && bid.start_byte < rb.end_byte {
                        Some(id)
                    } else {
                        None
                    }
                });

                cb_builder.push(ContentBlock {
                    kind: rb.kind,
                    range: Range::new(
                        Position::new(rb.start_row, rb.start_col),
                        Position::new(rb.end_row, rb.end_col),
                    ),
                    start_byte: rb.start_byte,
                    end_byte: rb.end_byte,
                    parent_heading,
                    block_id: merged_block_id,
                });
            }
            let content_blocks = cb_builder.into_bump_slice();

            // Overwrite block_id_map entries with merged content blocks
            // so block_by_id() returns the full paragraph range, not just the ^marker.
            for cb in content_blocks {
                if let Some(id) = cb.block_id {
                    block_id_map.insert(id, *cb);
                }
            }

            // --- XML Tags ---
            let mut xt_builder = BumpVec::new_in(arena_ref);
            for xt in &data.xml_tags {
                let tag_name = arena_alloc_str(arena_ref, &xt.tag_name);
                let start_pos = Position::new(xt.start_line, xt.start_col);
                let end_pos = Position::new(xt.end_line, xt.end_col);
                xt_builder.push(XmlTagEntry {
                    tag_name,
                    attributes: hashbrown::HashMap::new(),
                    is_self_closing: xt.is_self_closing,
                    is_unclosed: xt.is_unclosed,
                    is_inline: xt.is_inline,
                    range: Range::new(start_pos, end_pos),
                    start_byte: xt.source_offset as usize,
                    end_byte: xt.end_offset as usize,
                });
            }
            let xml_tags = xt_builder.into_bump_slice();

            // --- Code spans ---
            let mut cs_builder = BumpVec::new_in(arena_ref);
            for cs in &data.code_spans {
                let text = arena_alloc_str(arena_ref, &cs.text);
                let start_pos = Position::new(cs.start_line, cs.start_col);
                let end_pos = Position::new(cs.end_line, cs.end_col);
                cs_builder.push(CodeSpanEntry {
                    text,
                    range: Range::new(start_pos, end_pos),
                    start_byte: cs.source_offset as usize,
                    end_byte: cs.end_offset as usize,
                    language_hint: None,
                    kind: None,
                });
            }
            let code_spans = cs_builder.into_bump_slice();

            let mut frontmatter_builder = BumpVec::new_in(arena_ref);
            for fm in fm_owned {
                let key = arena_alloc_str(arena_ref, &fm.key);
                let value = helpers::owned_value_to_arena(fm.value, arena_ref);
                frontmatter_builder.push(FrontmatterEntry { key, value });
            }
            let frontmatter = frontmatter_builder.into_bump_slice();

            let mut aliases_builder = BumpVec::new_in(arena_ref);
            for alias in aliases_owned {
                aliases_builder.push(arena_alloc_str(arena_ref, &alias));
            }
            let aliases = aliases_builder.into_bump_slice();

            // --- Properties ---
            let mut props_builder = BumpVec::new_in(arena_ref);
            for pd in &data.properties {
                let key = arena_alloc_str(arena_ref, &pd.key);
                let value = match pd.value_type {
                    1 => {
                        let items: Vec<&str> = pd.value.split(',').map(|s| s.trim()).collect();
                        let mut bump_items = BumpVec::new_in(arena_ref);
                        for item in items {
                            bump_items.push(arena_alloc_str(arena_ref, item));
                        }
                        PropertyValueEntry::List(bump_items.into_bump_slice())
                    }
                    2 => {
                        let inner = pd.value.trim_start_matches("[[").trim_end_matches("]]");
                        PropertyValueEntry::PageRef(arena_alloc_str(arena_ref, inner))
                    }
                    _ => PropertyValueEntry::String(arena_alloc_str(arena_ref, &pd.value)),
                };
                props_builder.push(PropertyEntry { key, value });
            }
            let properties = props_builder.into_bump_slice();

            // --- Tasks ---
            let mut tasks_builder = BumpVec::new_in(arena_ref);
            for td in &data.tasks {
                let state = arena_alloc_str(arena_ref, &td.state);
                let text = arena_alloc_str(arena_ref, &td.text);
                let start_pos = Position::new(td.start_line, td.start_col);
                let end_pos = Position::new(td.end_line, td.end_col);
                tasks_builder.push(TaskEntry {
                    state,
                    text,
                    range: Range::new(start_pos, end_pos),
                    start_byte: td.source_offset as usize,
                    end_byte: td.end_offset as usize,
                });
            }
            let tasks = tasks_builder.into_bump_slice();

            // --- Embeds ---
            let mut embeds_builder = BumpVec::new_in(arena_ref);
            for ed in &data.embeds {
                let target = arena_alloc_str(arena_ref, &ed.target);
                let start_pos = Position::new(ed.start_line, ed.start_col);
                let end_pos = Position::new(ed.end_line, ed.end_col);
                embeds_builder.push(EmbedEntry {
                    target,
                    range: Range::new(start_pos, end_pos),
                    start_byte: ed.source_offset as usize,
                    end_byte: ed.end_offset as usize,
                });
            }
            let embeds = embeds_builder.into_bump_slice();

            // --- Callouts ---
            let mut callouts_builder = BumpVec::new_in(arena_ref);
            for cd in &data.callouts {
                let callout_type = arena_alloc_str(arena_ref, &cd.callout_type);
                let title = cd.title.as_deref().map(|t| arena_alloc_str(arena_ref, t));
                let start_pos = Position::new(cd.start_line, cd.start_col);
                let end_pos = Position::new(cd.end_line, cd.end_col);
                callouts_builder.push(CalloutEntry {
                    callout_type,
                    title,
                    range: Range::new(start_pos, end_pos),
                    start_byte: cd.source_offset as usize,
                    end_byte: cd.end_offset as usize,
                });
            }
            let callouts = callouts_builder.into_bump_slice();

            // --- Block refs ---
            let mut block_refs_builder = BumpVec::new_in(arena_ref);
            for br in &data.block_refs {
                let uuid = arena_alloc_str(arena_ref, &br.uuid);
                let start_pos = Position::new(br.start_line, br.start_col);
                let end_pos = Position::new(br.end_line, br.end_col);
                block_refs_builder.push(BlockRefEntry {
                    uuid,
                    range: Range::new(start_pos, end_pos),
                });
            }
            let block_refs = block_refs_builder.into_bump_slice();

            // --- Query blocks ---
            let mut qb_builder = BumpVec::new_in(arena_ref);
            for qb in &data.query_blocks {
                let query = arena_alloc_str(arena_ref, &qb.query);
                let start_pos = Position::new(qb.start_line, qb.start_col);
                let end_pos = Position::new(qb.end_line, qb.end_col);
                qb_builder.push(QueryBlockEntry {
                    query,
                    range: Range::new(start_pos, end_pos),
                    start_byte: qb.source_offset as usize,
                    end_byte: qb.end_offset as usize,
                });
            }
            let query_blocks = qb_builder.into_bump_slice();

            // --- Link definitions ---
            let mut ld_builder = BumpVec::new_in(arena_ref);
            for ld in &data.link_definitions {
                let label = arena_alloc_str(arena_ref, &ld.label);
                let url = arena_alloc_str(arena_ref, &ld.url);
                let title = ld.title.as_deref().map(|t| arena_alloc_str(arena_ref, t));
                let start_pos = Position::new(ld.start_line, ld.start_col);
                let end_pos = Position::new(ld.end_line, ld.end_col);
                ld_builder.push(LinkDefinitionEntry {
                    label,
                    url,
                    title,
                    range: Range::new(start_pos, end_pos),
                    start_byte: ld.source_offset as usize,
                    end_byte: ld.end_offset as usize,
                });
            }
            let link_definitions = ld_builder.into_bump_slice();

            DocumentDependent {
                headings,
                slug_to_heading,
                content_blocks,
                block_id_map,
                toc,
                outline,
                wiki_links,
                tags,
                markdown_links,
                xml_tags,
                code_spans,
                frontmatter,
                aliases,
                properties,
                block_refs,
                embeds,
                tasks,
                callouts,
                query_blocks,
                link_definitions,
            }
        });

        Self { cell }
    }
}
