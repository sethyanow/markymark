//! Extraction functions for arena-allocated markdown types.

mod blocks;
pub use blocks::{extract_block_ids, extract_block_refs, extract_callouts, extract_query_blocks};
mod frontmatter;
pub use frontmatter::{extract_frontmatter, extract_page_properties};
mod links;
pub use links::{
    extract_embeds, extract_link_definitions, extract_markdown_links, extract_wiki_links,
};
mod tags;
pub use tags::{extract_tags, extract_xml_tags};
mod tasks;
pub use tasks::extract_tasks;
