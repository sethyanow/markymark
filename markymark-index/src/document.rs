//! Document indexing: heading lookup, block lookup, TOC, outline tree.

use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;
use hashbrown::HashMap;
use markymark_core::prelude::*;
use markymark_parser::Ast;
use std::collections::HashMap as StdHashMap;
use std::sync::Mutex;

/// Allocate a string in the arena and return it as `&str`.
#[inline]
fn arena_alloc_str<'a>(arena: &'a Bump, s: &str) -> &'a str {
    let allocated: &mut str = arena.alloc_str(s);
    allocated
}

/// A heading entry in the document index.
#[derive(Debug, Clone)]
pub struct HeadingEntry<'arena> {
    /// The heading text.
    pub text: &'arena str,
    /// URL-safe slug derived from the heading text.
    pub slug: &'arena str,
    /// Heading level (1-6).
    pub level: u8,
    /// Source range of the heading.
    pub range: Range,
}

/// A block entry in the document index (Obsidian `^block-id`).
#[derive(Debug, Clone)]
pub struct BlockEntry<'arena> {
    /// The block identifier.
    pub id: &'arena str,
    /// Source range of the block.
    pub range: Range,
}

/// A table-of-contents entry.
#[derive(Debug, Clone)]
pub struct TocEntry<'arena> {
    /// Heading text.
    pub text: &'arena str,
    /// URL-safe slug.
    pub slug: &'arena str,
    /// Heading level (1-6).
    pub level: u8,
    /// Nesting depth relative to the root (0-based).
    pub depth: usize,
}

/// A node in the document outline tree.
#[derive(Debug, Clone)]
pub struct OutlineNode<'arena> {
    /// The heading at this node, if any (root node has `None`).
    pub heading: Option<HeadingEntry<'arena>>,
    /// Child outline nodes.
    pub children: &'arena [OutlineNode<'arena>],
}

/// A wiki link entry stored in the index.
#[derive(Debug, Clone)]
pub struct WikiLinkEntry<'arena> {
    /// Target page name.
    pub target: &'arena str,
    /// Optional alias text.
    pub alias: Option<&'arena str>,
    /// Optional heading anchor within the target.
    pub heading: Option<&'arena str>,
    /// Source range.
    pub range: Range,
}

/// A tag entry stored in the index.
#[derive(Debug, Clone)]
pub struct TagEntry<'arena> {
    /// Tag name (without leading `#`).
    pub name: &'arena str,
}

/// A markdown link entry stored in the index.
#[derive(Debug, Clone)]
pub struct MarkdownLinkEntry<'arena> {
    /// Link display text.
    pub text: &'arena str,
    /// Link URL.
    pub url: &'arena str,
    /// Optional anchor/fragment.
    pub anchor: Option<&'arena str>,
    /// Source range.
    pub range: Range,
}

/// An XML tag entry stored in the index.
#[derive(Debug, Clone)]
pub struct XmlTagEntry<'arena> {
    /// Tag name (e.g. "agent", "goal", "task").
    pub tag_name: &'arena str,
    /// Tag attributes as key-value pairs.
    pub attributes: HashMap<&'arena str, &'arena str>,
    /// Whether this is a self-closing tag (e.g. `<br/>`).
    pub is_self_closing: bool,
    /// Whether this tag has no matching closing tag.
    pub is_unclosed: bool,
    /// Source range of the entire tag.
    pub range: Range,
}

/// Convert heading text to a URL-safe slug.
pub fn slugify(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut slug = String::with_capacity(lower.len());

    for ch in lower.chars() {
        if ch.is_alphanumeric() || ch == '-' {
            slug.push(ch);
        } else if ch == ' ' {
            slug.push('-');
        }
        // Other non-alphanumeric chars are stripped entirely
    }

    // Collapse consecutive dashes
    let mut result = String::with_capacity(slug.len());
    let mut prev_dash = false;
    for ch in slug.chars() {
        if ch == '-' {
            if !prev_dash {
                result.push('-');
            }
            prev_dash = true;
        } else {
            result.push(ch);
            prev_dash = false;
        }
    }

    // Trim dashes from start/end
    result.trim_matches('-').to_string()
}

/// Deduplicate a slug given a set of already-used slugs.
fn dedup_slug(base: &str, used: &mut StdHashMap<String, usize>) -> String {
    let count = used.entry(base.to_string()).or_insert(0);
    let slug = if *count == 0 {
        base.to_string()
    } else {
        format!("{}-{}", base, count)
    };
    *count += 1;
    slug
}

/// Index of a single parsed markdown document.
///
/// Built from a [`markymark_parser::Ast`], provides fast lookups for
/// headings (by slug), block IDs, table of contents, and outline tree.
///
/// This struct owns its arena and stores arena-allocated data with a
/// `'static` lifetime marker. Like `markymark_parser::Ast`, this is a
/// self-referential pattern where `'static` is valid for the lifetime
/// of `self` because `self` owns the arena.
pub struct DocumentIndex {
    #[allow(dead_code)]
    _arena: Mutex<Bump>,
    headings: &'static [HeadingEntry<'static>],
    slug_to_heading: HashMap<&'static str, usize>,
    blocks: HashMap<&'static str, BlockEntry<'static>>,
    toc: &'static [TocEntry<'static>],
    outline: OutlineNode<'static>,
    wiki_links: &'static [WikiLinkEntry<'static>],
    tags: &'static [TagEntry<'static>],
    markdown_links: &'static [MarkdownLinkEntry<'static>],
    xml_tags: &'static [XmlTagEntry<'static>],
}

impl DocumentIndex {
    /// Build a document index from a parsed AST.
    pub fn from_ast(ast: &Ast) -> Self {
        let arena = Bump::new();

        // SAFETY: The arena is owned by `Self`, so this reference is valid
        // for the lifetime of `Self`.
        let arena_ref: &'static Bump = unsafe { &*(&arena as *const Bump) };

        let mut headings_builder: BumpVec<'static, HeadingEntry<'static>> =
            BumpVec::new_in(arena_ref);
        let mut slug_to_heading = HashMap::new();
        let mut slug_counts: StdHashMap<String, usize> = StdHashMap::new();

        // Extract headings
        for element in ast.root_elements() {
            if let Some(h) = element.as_heading() {
                let base_slug = slugify(h.text());
                let slug_owned = dedup_slug(&base_slug, &mut slug_counts);
                let text = arena_alloc_str(arena_ref, h.text());
                let slug = arena_alloc_str(arena_ref, &slug_owned);
                let idx = headings_builder.len();

                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text,
                    slug,
                    level: h.level(),
                    range: h.range(),
                });
            }
        }

        let headings = headings_builder.into_bump_slice();

        // Extract block IDs
        let mut blocks = HashMap::new();
        for block_id in ast.extract_block_ids() {
            let id = arena_alloc_str(arena_ref, block_id.id());
            blocks.insert(
                id,
                BlockEntry {
                    id,
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                },
            );
        }

        // Build TOC and outline tree
        let toc = build_toc(arena_ref, headings);
        let outline = build_outline(arena_ref, headings);

        // Extract wiki links
        let mut wiki_links_builder: BumpVec<'static, WikiLinkEntry<'static>> =
            BumpVec::new_in(arena_ref);
        for wl in ast.extract_wiki_links() {
            wiki_links_builder.push(WikiLinkEntry {
                target: arena_alloc_str(arena_ref, wl.target_page().unwrap_or("")),
                alias: wl.alias().map(|s| arena_alloc_str(arena_ref, s)),
                heading: wl.target_heading().map(|s| arena_alloc_str(arena_ref, s)),
                range: wl.range(),
            });
        }
        let wiki_links = wiki_links_builder.into_bump_slice();

        // Extract tags
        let mut tags_builder: BumpVec<'static, TagEntry<'static>> = BumpVec::new_in(arena_ref);
        for t in ast.extract_tags() {
            tags_builder.push(TagEntry {
                name: arena_alloc_str(arena_ref, t.name()),
            });
        }
        let tags = tags_builder.into_bump_slice();

        // Extract markdown links
        let mut markdown_links_builder: BumpVec<'static, MarkdownLinkEntry<'static>> =
            BumpVec::new_in(arena_ref);
        for ml in ast.extract_markdown_links() {
            let anchor = ml.anchor().map(|s| arena_alloc_str(arena_ref, s));
            let url_owned = match ml.anchor() {
                Some(raw_anchor) => format!("{}#{}", ml.url(), raw_anchor),
                None => ml.url().to_string(),
            };

            markdown_links_builder.push(MarkdownLinkEntry {
                text: arena_alloc_str(arena_ref, ml.text()),
                url: arena_alloc_str(arena_ref, &url_owned),
                anchor,
                range: ml.range(),
            });
        }
        let markdown_links = markdown_links_builder.into_bump_slice();

        // Extract XML tags
        let mut xml_tags_builder: BumpVec<'static, XmlTagEntry<'static>> =
            BumpVec::new_in(arena_ref);
        for xt in ast.extract_xml_tags() {
            let mut attributes = HashMap::new();
            for (k, v) in xt.attributes() {
                attributes.insert(arena_alloc_str(arena_ref, k), arena_alloc_str(arena_ref, v));
            }

            xml_tags_builder.push(XmlTagEntry {
                tag_name: arena_alloc_str(arena_ref, xt.tag_name()),
                attributes,
                is_self_closing: xt.is_self_closing(),
                is_unclosed: xt.is_unclosed(),
                range: xt.range(),
            });
        }
        let xml_tags = xml_tags_builder.into_bump_slice();

        Self {
            _arena: Mutex::new(arena),
            headings,
            slug_to_heading,
            blocks,
            toc,
            outline,
            wiki_links,
            tags,
            markdown_links,
            xml_tags,
        }
    }

    /// Look up a heading by its slug.
    pub fn heading_by_slug(&self, slug: &str) -> Option<&HeadingEntry<'static>> {
        self.slug_to_heading
            .get(slug)
            .map(|&idx| &self.headings[idx])
    }

    /// Look up a block by its ID.
    pub fn block_by_id(&self, id: &str) -> Option<&BlockEntry<'static>> {
        self.blocks.get(id)
    }

    /// Get the flat table of contents.
    pub fn toc(&self) -> &[TocEntry<'static>] {
        self.toc
    }

    /// Get the outline tree.
    pub fn outline(&self) -> &OutlineNode<'static> {
        &self.outline
    }

    /// Get all indexed headings.
    pub fn headings(&self) -> &[HeadingEntry<'static>] {
        self.headings
    }

    /// Get all indexed wiki links.
    pub fn wiki_links(&self) -> &[WikiLinkEntry<'static>] {
        self.wiki_links
    }

    /// Get all indexed tags.
    pub fn tags(&self) -> &[TagEntry<'static>] {
        self.tags
    }

    /// Get all indexed markdown links.
    pub fn markdown_links(&self) -> &[MarkdownLinkEntry<'static>] {
        self.markdown_links
    }

    /// Get all indexed XML tags.
    pub fn xml_tags(&self) -> &[XmlTagEntry<'static>] {
        self.xml_tags
    }

    /// Get all block IDs in this document.
    pub fn block_ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.blocks.keys().copied()
    }
}

/// Build flat TOC entries with depth calculation.
fn build_toc<'arena>(
    arena: &'arena Bump,
    headings: &[HeadingEntry<'arena>],
) -> &'arena [TocEntry<'arena>] {
    let mut toc = BumpVec::new_in(arena);
    let mut level_stack: Vec<u8> = Vec::new();

    for h in headings {
        while let Some(&top) = level_stack.last() {
            if top >= h.level {
                level_stack.pop();
            } else {
                break;
            }
        }

        let depth = level_stack.len();
        level_stack.push(h.level);

        toc.push(TocEntry {
            text: h.text,
            slug: h.slug,
            level: h.level,
            depth,
        });
    }

    toc.into_bump_slice()
}

#[derive(Debug, Clone)]
struct TempOutline<'arena> {
    heading: Option<HeadingEntry<'arena>>,
    children: Vec<TempOutline<'arena>>,
}

fn get_temp_node_mut<'tree, 'arena>(
    root: &'tree mut TempOutline<'arena>,
    path: &[usize],
) -> &'tree mut TempOutline<'arena> {
    let mut current = root;
    for &idx in path {
        current = &mut current.children[idx];
    }
    current
}

fn freeze_outline<'arena>(arena: &'arena Bump, node: TempOutline<'arena>) -> OutlineNode<'arena> {
    let mut children = BumpVec::new_in(arena);
    for child in node.children {
        children.push(freeze_outline(arena, child));
    }

    OutlineNode {
        heading: node.heading,
        children: children.into_bump_slice(),
    }
}

/// Build outline tree from heading entries.
fn build_outline<'arena>(
    arena: &'arena Bump,
    headings: &[HeadingEntry<'arena>],
) -> OutlineNode<'arena> {
    let mut root = TempOutline {
        heading: None,
        children: Vec::new(),
    };

    // Stack entries are (heading level, path of child indices from root).
    let mut stack: Vec<(u8, Vec<usize>)> = Vec::new();

    for h in headings {
        let node = TempOutline {
            heading: Some(h.clone()),
            children: Vec::new(),
        };

        while let Some((lvl, _)) = stack.last() {
            if *lvl >= h.level {
                stack.pop();
            } else {
                break;
            }
        }

        if stack.is_empty() {
            root.children.push(node);
            let idx = root.children.len() - 1;
            stack.push((h.level, vec![idx]));
        } else {
            let parent_path = stack.last().expect("stack not empty").1.clone();
            let parent = get_temp_node_mut(&mut root, &parent_path);
            parent.children.push(node);
            let child_idx = parent.children.len() - 1;

            let mut child_path = parent_path;
            child_path.push(child_idx);
            stack.push((h.level, child_path));
        }
    }

    freeze_outline(arena, root)
}

#[cfg(test)]
mod arena_allocation_tests {
    use super::*;
    use markymark_parser::Parser;

    #[test]
    fn heading_entry_uses_borrowed_str() {
        let arena = Bump::new();
        let text = arena.alloc_str("Introduction");
        let slug = arena.alloc_str("introduction");

        let entry = HeadingEntry {
            text,
            slug,
            level: 1,
            range: Range::new(Position::new(0, 0), Position::new(0, 13)),
        };

        assert_eq!(entry.text, "Introduction");
        assert_eq!(entry.slug, "introduction");
        assert_eq!(entry.level, 1);
    }

    #[test]
    fn document_index_uses_arena_allocated_types() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("# Root\n\n## Child\n\nA block ^block-1\n\n[[Page#intro]]\n")
            .unwrap();

        let index = DocumentIndex::from_ast(&ast);

        assert_eq!(index.headings().len(), 2);
        assert_eq!(index.headings()[0].text, "Root");
        assert_eq!(index.headings()[1].slug, "child");
        assert!(index.block_by_id("block-1").is_some());
    }

    #[test]
    fn toc_is_arena_slice() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("# A\n\n## B\n\n### C\n").unwrap();
        let index = DocumentIndex::from_ast(&ast);

        let toc = index.toc();
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].depth, 0);
        assert_eq!(toc[1].depth, 1);
        assert_eq!(toc[2].depth, 2);
    }

    #[test]
    fn outline_children_are_arena_slices() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("# Root\n\n## Child\n\n### Grandchild\n")
            .unwrap();
        let index = DocumentIndex::from_ast(&ast);

        let outline = index.outline();
        assert_eq!(outline.children.len(), 1);
        assert_eq!(outline.children[0].heading.as_ref().unwrap().text, "Root");
        assert_eq!(outline.children[0].children.len(), 1);
    }

    #[test]
    fn xml_attributes_use_borrowed_arena_keys_values() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("<goal priority=\"high\" status=\"open\">Ship</goal>\n")
            .unwrap();
        let index = DocumentIndex::from_ast(&ast);

        let tags = index.xml_tags();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].attributes.get("priority"), Some(&"high"));
        assert_eq!(tags[0].attributes.get("status"), Some(&"open"));
    }
}
