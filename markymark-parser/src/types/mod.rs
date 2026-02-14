//! Parser types for arena-allocated markdown AST.
//!
//! All types use `'arena` lifetime for borrowed data, enabling efficient
//! bulk deallocation via bumpalo arenas.

mod blocks;
mod elements;
mod frontmatter;
mod links;
mod xml;

pub use blocks::*;
pub use elements::*;
pub use frontmatter::*;
pub use links::*;
pub use xml::*;

/// Allocate a string in the arena and return it as `&'arena str`.
/// This helper is needed because `Bump::alloc_str` returns `&mut str`,
/// which doesn't automatically coerce in all contexts.
#[inline]
pub(crate) fn arena_alloc_str<'a>(arena: &'a bumpalo::Bump, s: &str) -> &'a str {
    let allocated: &mut str = arena.alloc_str(s);
    allocated
}

// ============================================================================
// ARENA ALLOCATION TESTS (GREEN phase)
// Tests verify arena-allocated types work correctly with 'arena lifetime.
// ============================================================================

#[cfg(test)]
mod arena_allocation_tests {
    use super::*;
    use bumpalo::Bump;
    use markymark_core::arena::new_arena_hashmap;
    use markymark_core::prelude::{Position, Range};

    // ========================================================================
    // PARSER TYPES: Arena Lifetime Tests
    // ========================================================================

    /// Heading uses arena-allocated text
    #[test]
    fn heading_uses_arena_lifetime() {
        let arena = Bump::new();
        let heading = Heading::new(
            1,
            arena.alloc_str("Test Heading"),
            Range::new(Position::new(0, 0), Position::new(0, 12)),
        );

        assert_eq!(heading.level(), 1);
        assert_eq!(heading.text(), "Test Heading");
    }

    /// Paragraph uses arena-allocated text
    #[test]
    fn paragraph_uses_arena_lifetime() {
        let arena = Bump::new();
        let paragraph = Paragraph::new(
            arena.alloc_str("Test paragraph content"),
            Range::new(Position::new(0, 0), Position::new(0, 21)),
        );

        assert_eq!(paragraph.text(), "Test paragraph content");
    }

    /// ListItem uses arena-allocated text and properties map
    #[test]
    fn list_item_uses_arena_lifetime() {
        let arena = Bump::new();
        let mut props = new_arena_hashmap(&arena);
        props.insert(
            arena_alloc_str(&arena, "key"),
            arena_alloc_str(&arena, "value"),
        );

        let item = ListItem::new(arena_alloc_str(&arena, "- test item"), props, &[]);

        assert_eq!(item.text(), "- test item");
        assert_eq!(item.properties().get("key"), Some(&"value"));
    }

    /// WikiLink uses arena-allocated strings
    #[test]
    fn wiki_link_uses_arena_lifetime() {
        let arena = Bump::new();
        let link = WikiLink::new(
            arena.alloc_str("target-page"),
            Some(arena.alloc_str("alias")),
            Some(arena.alloc_str("section")),
            None,
            Range::new(Position::new(0, 0), Position::new(0, 12)),
        );

        assert_eq!(link.target_page(), Some("target-page"));
        assert_eq!(link.alias(), Some("alias"));
        assert_eq!(link.target_heading(), Some("section"));
    }

    /// MarkdownLink uses arena-allocated strings
    #[test]
    fn markdown_link_uses_arena_lifetime() {
        let arena = Bump::new();
        let link = MarkdownLink::new(
            arena.alloc_str("link text"),
            arena.alloc_str("https://example.com"),
            Some(arena.alloc_str("anchor")),
            None,
            Range::new(Position::new(0, 0), Position::new(0, 9)),
        );

        assert_eq!(link.text(), "link text");
        assert_eq!(link.url(), "https://example.com");
        assert_eq!(link.anchor(), Some("anchor"));
    }

    /// LinkDefinition uses arena-allocated strings
    #[test]
    fn link_definition_uses_arena_lifetime() {
        let arena = Bump::new();
        let def = LinkDefinition::new(
            arena.alloc_str("ref-label"),
            arena.alloc_str("https://example.com"),
            Some(arena.alloc_str("Title")),
        );

        assert_eq!(def.label(), "ref-label");
        assert_eq!(def.url(), "https://example.com");
        assert_eq!(def.title(), Some("Title"));
    }

    /// BlockId uses arena-allocated id
    #[test]
    fn block_id_uses_arena_lifetime() {
        let arena = Bump::new();
        let id = BlockId::new(arena.alloc_str("abc123"));

        assert_eq!(id.id(), "abc123");
    }

    /// BlockRef uses arena-allocated uuid
    #[test]
    fn block_ref_uses_arena_lifetime() {
        let arena = Bump::new();
        let block_ref = BlockRef::new(arena.alloc_str("uuid-1234-5678"));

        assert_eq!(block_ref.uuid(), "uuid-1234-5678");
    }

    /// Tag uses arena-allocated name
    #[test]
    fn tag_uses_arena_lifetime() {
        let arena = Bump::new();
        let tag = Tag::new(arena.alloc_str("project/feature"));

        assert_eq!(tag.name(), "project/feature");
        assert_eq!(tag.segments(), vec!["project", "feature"]);
    }

    /// Embed uses arena-allocated target
    #[test]
    fn embed_uses_arena_lifetime() {
        let arena = Bump::new();
        let embed = Embed::new(arena.alloc_str("embedded-page"));

        assert_eq!(embed.target(), "embedded-page");
        assert!(embed.is_embed());
    }

    /// Task and TaskState use arena-allocated strings
    #[test]
    fn task_uses_arena_lifetime() {
        let arena = Bump::new();
        let task = Task::new(TaskState::new(arena.alloc_str("TODO")));

        assert_eq!(task.state().as_str(), "TODO");
    }

    /// Callout uses arena-allocated strings
    #[test]
    fn callout_uses_arena_lifetime() {
        let arena = Bump::new();
        let callout = Callout::new(arena.alloc_str("note"), Some(arena.alloc_str("Pro Tip")));

        assert_eq!(callout.callout_type(), "note");
        assert_eq!(callout.title(), Some("Pro Tip"));
    }

    /// QueryBlock uses arena-allocated query
    #[test]
    fn query_block_uses_arena_lifetime() {
        let arena = Bump::new();
        let query = QueryBlock::new(arena.alloc_str("{{query todo}}"));

        assert_eq!(query.query_text(), "{{query todo}}");
    }

    /// Frontmatter uses arena-allocated HashMap
    #[test]
    fn frontmatter_uses_arena_lifetime() {
        let arena = Bump::new();
        let mut data = new_arena_hashmap(&arena);
        data.insert(
            arena_alloc_str(&arena, "title"),
            FrontmatterValue::String(arena_alloc_str(&arena, "My Page")),
        );

        let fm = Frontmatter::new(data);

        assert_eq!(fm.get_string("title"), Some("My Page"));
    }

    /// Properties uses arena-allocated HashMap
    #[test]
    fn properties_uses_arena_lifetime() {
        let arena = Bump::new();
        let mut data = new_arena_hashmap(&arena);
        data.insert(
            arena_alloc_str(&arena, "type"),
            PropertyValue::String(arena_alloc_str(&arena, "project")),
        );

        let props = Properties::new(data);

        assert_eq!(props.get("type").unwrap().as_str(), Some("project"));
    }

    /// XmlTag uses arena-allocated strings and HashMap
    #[test]
    fn xml_tag_uses_arena_lifetime() {
        let arena = Bump::new();
        let mut attrs = new_arena_hashmap(&arena);
        attrs.insert(
            arena_alloc_str(&arena, "id"),
            arena_alloc_str(&arena, "main"),
        );

        let tag = XmlTag::new(
            arena_alloc_str(&arena, "agent"),
            attrs,
            false,
            Some(arena_alloc_str(&arena, "content")),
            Range::new(Position::new(0, 0), Position::new(0, 10)),
        );

        assert_eq!(tag.tag_name(), "agent");
        assert_eq!(tag.attributes().get("id"), Some(&"main"));
        assert_eq!(tag.content(), Some("content"));
        assert!(!tag.is_self_closing());
    }

    /// Element enum variants contain arena-allocated types
    #[test]
    fn element_uses_arena_lifetime() {
        let arena = Bump::new();

        let heading = Heading::new(
            2,
            arena.alloc_str("Section"),
            Range::new(Position::new(0, 0), Position::new(0, 7)),
        );
        let element = Element::Heading(heading);

        assert!(matches!(element, Element::Heading(_)));
        assert_eq!(element.as_heading().unwrap().text(), "Section");
    }

    // ========================================================================
    // ARENA STRING STORAGE TESTS
    // ========================================================================

    /// Heading text is &str borrowed from arena
    #[test]
    fn heading_text_is_arena_str() {
        let arena = Bump::new();
        let text: &str = arena.alloc_str("Test Heading");
        let heading = Heading::new(
            1,
            text,
            Range::new(Position::new(0, 0), Position::new(0, 12)),
        );

        // Verify text is borrowed from arena (same pointer)
        assert_eq!(heading.text().as_ptr(), text.as_ptr());
    }

    /// ListItem properties use arena-allocated HashMap
    #[test]
    fn list_item_properties_arena_map() {
        let arena = Bump::new();

        let mut map = new_arena_hashmap(&arena);
        let key = arena_alloc_str(&arena, "property");
        let value = arena_alloc_str(&arena, "value");
        map.insert(key, value);

        let item = ListItem::new(arena_alloc_str(&arena, "test"), map, &[]);

        assert_eq!(item.properties().get("property"), Some(&"value"));
    }

    /// Vec fields are arena slices
    #[test]
    fn vec_fields_become_arena_slices() {
        let arena = Bump::new();

        let item = ListItem::new(
            arena_alloc_str(&arena, "parent"),
            new_arena_hashmap(&arena),
            &[], // Empty arena slice
        );

        assert!(item.children().is_none() || item.children().unwrap().is_empty());
    }
}
