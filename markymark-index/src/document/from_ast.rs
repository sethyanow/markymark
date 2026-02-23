//! [`DocumentIndex::from_ast`] family — construct index from a parsed AST.
//!
//! Extracts owned intermediate records from the parser AST, moves the parser
//! arena into the index, and allocates the final index entries in one
//! arena-backed pass via [`self_cell`].

use bumpalo::collections::Vec as BumpVec;
use hashbrown::HashMap;
use markymark_core::arena::arena_alloc_str;
use markymark_core::prelude::*;
use markymark_parser::Ast;
use std::collections::HashMap as StdHashMap;

use super::{
    helpers, BlockEntry, BlockOwned, BlockRefEntry, CalloutEntry, CalloutOwned, CodeSpanEntry,
    CodeSpanOwned, DocumentDependent, DocumentIndex, DocumentIndexCell, DocumentOwner, EmbedEntry,
    EmbedOwned, FrontmatterEntry, FrontmatterValueEntry, HeadingEntry, IncrementalOverrides,
    LinkDefinitionEntry, LinkDefinitionOwned, MarkdownLinkEntry, MarkdownLinkOwned, PropertyEntry,
    PropertyValueEntry, QueryBlockEntry, QueryBlockOwned, TagEntry, TagOwned, TaskEntry, TaskOwned,
    WikiLinkEntry, WikiLinkOwned, XmlTagEntry, XmlTagOwned,
};

impl DocumentIndex {
    /// Build a document index from a parsed AST.
    ///
    /// Extracts owned intermediate records, moves the parser arena into this
    /// index, and allocates the final index entries in one arena-backed pass.
    pub fn from_ast(ast: Ast) -> Self {
        Self::from_ast_with_overrides_opt(ast, IncrementalOverrides::default())
    }

    /// Build a document index from a parsed AST while overriding wiki-links.
    ///
    /// This is used by incremental reindexing paths that already computed
    /// a selective wiki-link merge and want to avoid full re-extraction.
    pub fn from_ast_with_wiki_links(ast: Ast, wiki_links: Vec<WikiLinkOwned>) -> Self {
        Self::from_ast_with_overrides_opt(
            ast,
            IncrementalOverrides {
                wiki_links: Some(wiki_links),
                ..Default::default()
            },
        )
    }

    /// Build a document index from a parsed AST while overriding blocks.
    ///
    /// This is used by incremental reindexing paths that already computed
    /// a selective block merge and want to avoid full re-extraction.
    pub fn from_ast_with_blocks(ast: Ast, blocks: Vec<BlockOwned>) -> Self {
        Self::from_ast_with_overrides_opt(
            ast,
            IncrementalOverrides {
                blocks: Some(blocks),
                ..Default::default()
            },
        )
    }

    /// Build a document index from a parsed AST while overriding both wiki-links and blocks.
    ///
    /// This is the primary incremental path when both extractors have been merged.
    pub fn from_ast_with_wiki_links_and_blocks(
        ast: Ast,
        wiki_links: Vec<WikiLinkOwned>,
        blocks: Vec<BlockOwned>,
    ) -> Self {
        Self::from_ast_with_overrides_opt(
            ast,
            IncrementalOverrides {
                wiki_links: Some(wiki_links),
                blocks: Some(blocks),
                ..Default::default()
            },
        )
    }

    /// Build a document index from a parsed AST with selective extractor overrides.
    ///
    /// This is the primary construction path used by incremental reindexing. For each
    /// extractor, a `Some` override skips re-extraction and uses the provided data;
    /// `None` extracts fresh from the AST. Always use [`IncrementalOverrides`] rather
    /// than calling the convenience functions when multiple extractors need overrides.
    pub fn from_ast_with_overrides_opt(ast: Ast, overrides: IncrementalOverrides) -> Self {
        #[derive(Debug)]
        struct HeadingOwned {
            text: String,
            level: u8,
            range: Range,
        }

        let mut headings_owned = Vec::new();
        for element in ast.root_elements() {
            if let Some(h) = element.as_heading() {
                headings_owned.push(HeadingOwned {
                    text: h.text().to_string(),
                    level: h.level(),
                    range: h.range(),
                });
            }
        }

        let blocks_owned = if let Some(blocks_override) = overrides.blocks {
            blocks_override
        } else {
            let mut blocks_owned = Vec::new();
            for block_id in ast.extract_block_ids() {
                blocks_owned.push(BlockOwned {
                    id: block_id.id().to_string(),
                    range: block_id.range(),
                    start_byte: block_id.start_byte(),
                    end_byte: block_id.end_byte(),
                });
            }
            blocks_owned
        };

        let wiki_links_owned = if let Some(wiki_links_override) = overrides.wiki_links {
            wiki_links_override
        } else {
            let mut wiki_links_owned = Vec::new();
            for wl in ast.extract_wiki_links() {
                if wl.target_page().is_none()
                    && wl.target_heading().is_none()
                    && wl.target_block_id().is_none()
                {
                    continue;
                }

                let (start_byte, end_byte) = wl.byte_range();
                wiki_links_owned.push(WikiLinkOwned {
                    target: wl.target_page().unwrap_or("").to_string(),
                    alias: wl.alias().map(str::to_string),
                    heading: wl.target_heading().map(str::to_string),
                    range: wl.range(),
                    start_byte,
                    end_byte,
                });
            }
            wiki_links_owned
        };

        // Tags have no source range in the parser — always re-extract.
        // The `overrides.tags` field is present for API completeness but is always `None`.
        let tags_owned: Vec<TagOwned> = ast
            .extract_tags()
            .into_iter()
            .map(|tag| TagOwned {
                name: tag.name().to_string(),
            })
            .collect();

        let markdown_links_owned = if let Some(ml_override) = overrides.markdown_links {
            ml_override
        } else {
            ast.extract_markdown_links()
                .into_iter()
                .map(|ml| {
                    let (start_byte, end_byte) = ml.byte_range();
                    MarkdownLinkOwned {
                        text: ml.text().to_string(),
                        url: ml.url().to_string(),
                        anchor: ml.anchor().map(str::to_string),
                        range: ml.range(),
                        start_byte,
                        end_byte,
                    }
                })
                .collect()
        };

        let xml_tags_owned = if let Some(xt_override) = overrides.xml_tags {
            xt_override
        } else {
            ast.extract_xml_tags()
                .into_iter()
                .map(|xt| {
                    let mut attributes: Vec<(String, String)> = xt
                        .attributes()
                        .iter()
                        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                        .collect();
                    attributes.sort_by(|a, b| a.0.cmp(&b.0));
                    let (start_byte, end_byte) = xt.byte_range();
                    XmlTagOwned {
                        tag_name: xt.tag_name().to_string(),
                        attributes,
                        is_self_closing: xt.is_self_closing(),
                        is_unclosed: xt.is_unclosed(),
                        range: xt.range(),
                        start_byte,
                        end_byte,
                    }
                })
                .collect()
        };

        // Code spans: use overrides if available, otherwise empty (from_ast doesn't
        // extract code spans yet — that's Phase A-3/B).
        let code_spans_owned: Vec<CodeSpanOwned> = overrides.code_spans.unwrap_or_default();

        // Extract frontmatter and properties as owned data BEFORE arena move.
        #[derive(Debug)]
        enum FrontmatterValueOwned {
            String(String),
            List(Vec<String>),
        }
        #[derive(Debug)]
        struct FrontmatterOwned {
            key: String,
            value: FrontmatterValueOwned,
        }
        #[derive(Debug)]
        enum PropertyValueOwned {
            String(String),
            List(Vec<String>),
            PageRef(String),
        }
        #[derive(Debug)]
        struct PropertyOwned {
            key: String,
            value: PropertyValueOwned,
        }

        let mut frontmatter_owned: Vec<FrontmatterOwned> = Vec::new();
        let mut aliases_owned: Vec<String> = Vec::new();

        if let Some(fm) = ast.frontmatter() {
            use markymark_parser::FrontmatterValue;
            for (key, value) in fm.iter() {
                let key_str = (*key).to_string();
                let value_owned = match value {
                    FrontmatterValue::String(s) => FrontmatterValueOwned::String((*s).to_string()),
                    FrontmatterValue::List(items) => {
                        FrontmatterValueOwned::List(items.iter().map(|s| s.to_string()).collect())
                    }
                };
                // Extract aliases separately for the dedicated accessor.
                if key_str == "aliases" {
                    match &value_owned {
                        FrontmatterValueOwned::String(s) => {
                            if !s.is_empty() {
                                aliases_owned.push(s.clone());
                            }
                        }
                        FrontmatterValueOwned::List(items) => {
                            aliases_owned.extend(items.iter().cloned());
                        }
                    }
                }
                frontmatter_owned.push(FrontmatterOwned {
                    key: key_str,
                    value: value_owned,
                });
            }
        }

        let mut properties_owned: Vec<PropertyOwned> = Vec::new();
        if let Some(props) = ast.page_properties() {
            use markymark_parser::PropertyValue;
            for (key, value) in props.iter() {
                let key_str = (*key).to_string();
                let value_owned = match value {
                    PropertyValue::String(s) => PropertyValueOwned::String((*s).to_string()),
                    PropertyValue::List(items) => {
                        PropertyValueOwned::List(items.iter().map(|s| s.to_string()).collect())
                    }
                    PropertyValue::PageRef(s) => PropertyValueOwned::PageRef((*s).to_string()),
                };
                properties_owned.push(PropertyOwned {
                    key: key_str,
                    value: value_owned,
                });
            }
        }

        // Extract block refs as owned data BEFORE arena move.
        #[derive(Debug)]
        struct BlockRefOwned {
            uuid: String,
            range: markymark_core::Range,
        }
        let block_refs_owned: Vec<BlockRefOwned> = ast
            .extract_block_refs()
            .into_iter()
            .map(|r| BlockRefOwned {
                uuid: r.uuid().to_string(),
                range: r.range(),
            })
            .collect();

        // Extract 5 new types as owned data BEFORE arena move.
        // Positional fields default to zero — Zig extractors (Phase B-3+) will
        // supply real ranges.
        let embeds_owned: Vec<EmbedOwned> = if let Some(ov) = overrides.embeds {
            ov
        } else {
            ast.extract_embeds()
                .into_iter()
                .map(|e| EmbedOwned {
                    target: e.target().to_string(),
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    start_byte: 0,
                    end_byte: 0,
                })
                .collect()
        };

        let tasks_owned: Vec<TaskOwned> = if let Some(ov) = overrides.tasks {
            ov
        } else {
            ast.extract_tasks()
                .into_iter()
                .map(|t| TaskOwned {
                    state: t.state().as_str().to_string(),
                    text: String::new(),
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    start_byte: 0,
                    end_byte: 0,
                })
                .collect()
        };

        let callouts_owned: Vec<CalloutOwned> = if let Some(ov) = overrides.callouts {
            ov
        } else {
            ast.extract_callouts()
                .into_iter()
                .map(|c| CalloutOwned {
                    callout_type: c.callout_type().to_string(),
                    title: c.title().map(str::to_string),
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    start_byte: 0,
                    end_byte: 0,
                })
                .collect()
        };

        let query_blocks_owned: Vec<QueryBlockOwned> = if let Some(ov) = overrides.query_blocks {
            ov
        } else {
            ast.extract_query_blocks()
                .into_iter()
                .map(|q| QueryBlockOwned {
                    query: q.query_text().to_string(),
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    start_byte: 0,
                    end_byte: 0,
                })
                .collect()
        };

        let link_defs_owned: Vec<LinkDefinitionOwned> = if let Some(ov) = overrides.link_definitions
        {
            ov
        } else {
            ast.extract_link_definitions()
                .into_iter()
                .map(|ld| LinkDefinitionOwned {
                    label: ld.label().to_string(),
                    url: ld.url().to_string(),
                    title: ld.title().map(str::to_string),
                    range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                    start_byte: 0,
                    end_byte: 0,
                })
                .collect()
        };

        let owner = DocumentOwner {
            arena: ast.into_arena(),
        };
        let cell = DocumentIndexCell::new(owner, move |owner| {
            let arena_ref = owner.arena.bump();

            let mut headings_builder = BumpVec::new_in(arena_ref);
            let mut slug_to_heading = HashMap::new();
            let mut slug_counts: StdHashMap<String, usize> = StdHashMap::new();
            for h in headings_owned {
                let base_slug = super::slugify(&h.text);
                let slug_owned = helpers::dedup_slug(&base_slug, &mut slug_counts);
                let text = arena_alloc_str(arena_ref, &h.text);
                let slug = arena_alloc_str(arena_ref, &slug_owned);
                let idx = headings_builder.len();
                slug_to_heading.insert(slug, idx);
                headings_builder.push(HeadingEntry {
                    text,
                    slug,
                    level: h.level,
                    range: h.range,
                });
            }
            let headings = headings_builder.into_bump_slice();

            let mut blocks = HashMap::new();
            for block in blocks_owned {
                let id = arena_alloc_str(arena_ref, &block.id);
                blocks.insert(
                    id,
                    BlockEntry {
                        id,
                        range: block.range,
                        start_byte: block.start_byte,
                        end_byte: block.end_byte,
                    },
                );
            }

            let toc = helpers::build_toc(arena_ref, headings);
            let outline = helpers::build_outline(arena_ref, headings);

            let mut wiki_links_builder = BumpVec::new_in(arena_ref);
            for wl in &wiki_links_owned {
                wiki_links_builder.push(WikiLinkEntry {
                    target: arena_alloc_str(arena_ref, &wl.target),
                    alias: wl.alias.as_deref().map(|a| arena_alloc_str(arena_ref, a)),
                    heading: wl.heading.as_deref().map(|h| arena_alloc_str(arena_ref, h)),
                    range: wl.range,
                    start_byte: wl.start_byte,
                    end_byte: wl.end_byte,
                });
            }
            let wiki_links = wiki_links_builder.into_bump_slice();

            let mut tags_builder = BumpVec::new_in(arena_ref);
            for tag in tags_owned {
                tags_builder.push(TagEntry {
                    name: arena_alloc_str(arena_ref, &tag.name),
                });
            }
            let tags = tags_builder.into_bump_slice();

            let mut markdown_links_builder = BumpVec::new_in(arena_ref);
            for ml in markdown_links_owned {
                markdown_links_builder.push(MarkdownLinkEntry {
                    text: arena_alloc_str(arena_ref, &ml.text),
                    url: arena_alloc_str(arena_ref, &ml.url),
                    anchor: ml.anchor.as_deref().map(|a| arena_alloc_str(arena_ref, a)),
                    range: ml.range,
                    start_byte: ml.start_byte,
                    end_byte: ml.end_byte,
                });
            }
            let markdown_links = markdown_links_builder.into_bump_slice();

            let mut xml_tags_builder = BumpVec::new_in(arena_ref);
            for xt in xml_tags_owned {
                let mut attributes = HashMap::new();
                for (k, v) in xt.attributes {
                    let k_ref = arena_alloc_str(arena_ref, &k);
                    let v_ref = arena_alloc_str(arena_ref, &v);
                    attributes.insert(k_ref, v_ref);
                }
                xml_tags_builder.push(XmlTagEntry {
                    tag_name: arena_alloc_str(arena_ref, &xt.tag_name),
                    attributes,
                    is_self_closing: xt.is_self_closing,
                    is_unclosed: xt.is_unclosed,
                    range: xt.range,
                    start_byte: xt.start_byte,
                    end_byte: xt.end_byte,
                });
            }
            let xml_tags = xml_tags_builder.into_bump_slice();

            // Arena-allocate frontmatter entries.
            let mut frontmatter_builder = BumpVec::new_in(arena_ref);
            for fm in frontmatter_owned {
                let key = arena_alloc_str(arena_ref, &fm.key);
                let value = match fm.value {
                    FrontmatterValueOwned::String(s) => {
                        FrontmatterValueEntry::String(arena_alloc_str(arena_ref, &s))
                    }
                    FrontmatterValueOwned::List(items) => {
                        let mut list = BumpVec::new_in(arena_ref);
                        for item in items {
                            list.push(arena_alloc_str(arena_ref, &item));
                        }
                        FrontmatterValueEntry::List(list.into_bump_slice())
                    }
                };
                frontmatter_builder.push(FrontmatterEntry { key, value });
            }
            let frontmatter = frontmatter_builder.into_bump_slice();

            // Arena-allocate aliases (from frontmatter "aliases" key).
            let mut aliases_builder = BumpVec::new_in(arena_ref);
            for alias in aliases_owned {
                aliases_builder.push(arena_alloc_str(arena_ref, &alias));
            }
            let aliases = aliases_builder.into_bump_slice();

            // Arena-allocate properties entries.
            let mut properties_builder = BumpVec::new_in(arena_ref);
            for prop in properties_owned {
                let key = arena_alloc_str(arena_ref, &prop.key);
                let value = match prop.value {
                    PropertyValueOwned::String(s) => {
                        PropertyValueEntry::String(arena_alloc_str(arena_ref, &s))
                    }
                    PropertyValueOwned::List(items) => {
                        let mut list = BumpVec::new_in(arena_ref);
                        for item in items {
                            list.push(arena_alloc_str(arena_ref, &item));
                        }
                        PropertyValueEntry::List(list.into_bump_slice())
                    }
                    PropertyValueOwned::PageRef(s) => {
                        PropertyValueEntry::PageRef(arena_alloc_str(arena_ref, &s))
                    }
                };
                properties_builder.push(PropertyEntry { key, value });
            }
            let properties = properties_builder.into_bump_slice();

            // Arena-allocate block ref entries.
            let mut block_refs_builder = BumpVec::new_in(arena_ref);
            for br in block_refs_owned {
                block_refs_builder.push(BlockRefEntry {
                    uuid: arena_alloc_str(arena_ref, &br.uuid),
                    range: br.range,
                });
            }
            let block_refs = block_refs_builder.into_bump_slice();

            // Arena-allocate code span entries.
            let mut cs_builder = BumpVec::new_in(arena_ref);
            for cs in &code_spans_owned {
                cs_builder.push(CodeSpanEntry {
                    text: arena_alloc_str(arena_ref, &cs.text),
                    range: cs.range,
                    start_byte: cs.start_byte,
                    end_byte: cs.end_byte,
                    language_hint: None,
                    kind: None,
                });
            }
            let code_spans = cs_builder.into_bump_slice();

            // Arena-allocate embed entries.
            let mut embeds_builder = BumpVec::new_in(arena_ref);
            for e in &embeds_owned {
                embeds_builder.push(EmbedEntry {
                    target: arena_alloc_str(arena_ref, &e.target),
                    range: e.range,
                    start_byte: e.start_byte,
                    end_byte: e.end_byte,
                });
            }
            let embeds = embeds_builder.into_bump_slice();

            // Arena-allocate task entries.
            let mut tasks_builder = BumpVec::new_in(arena_ref);
            for t in &tasks_owned {
                tasks_builder.push(TaskEntry {
                    state: arena_alloc_str(arena_ref, &t.state),
                    text: arena_alloc_str(arena_ref, &t.text),
                    range: t.range,
                    start_byte: t.start_byte,
                    end_byte: t.end_byte,
                });
            }
            let tasks = tasks_builder.into_bump_slice();

            // Arena-allocate callout entries.
            let mut callouts_builder = BumpVec::new_in(arena_ref);
            for c in &callouts_owned {
                callouts_builder.push(CalloutEntry {
                    callout_type: arena_alloc_str(arena_ref, &c.callout_type),
                    title: c.title.as_deref().map(|t| arena_alloc_str(arena_ref, t)),
                    range: c.range,
                    start_byte: c.start_byte,
                    end_byte: c.end_byte,
                });
            }
            let callouts = callouts_builder.into_bump_slice();

            // Arena-allocate query block entries.
            let mut qb_builder = BumpVec::new_in(arena_ref);
            for q in &query_blocks_owned {
                qb_builder.push(QueryBlockEntry {
                    query: arena_alloc_str(arena_ref, &q.query),
                    range: q.range,
                    start_byte: q.start_byte,
                    end_byte: q.end_byte,
                });
            }
            let query_blocks = qb_builder.into_bump_slice();

            // Arena-allocate link definition entries.
            let mut ld_builder = BumpVec::new_in(arena_ref);
            for ld in &link_defs_owned {
                ld_builder.push(LinkDefinitionEntry {
                    label: arena_alloc_str(arena_ref, &ld.label),
                    url: arena_alloc_str(arena_ref, &ld.url),
                    title: ld.title.as_deref().map(|t| arena_alloc_str(arena_ref, t)),
                    range: ld.range,
                    start_byte: ld.start_byte,
                    end_byte: ld.end_byte,
                });
            }
            let link_definitions = ld_builder.into_bump_slice();

            DocumentDependent {
                headings,
                slug_to_heading,
                blocks,
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
