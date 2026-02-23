use super::header::{
    pool_str, read_u32_le, read_u8, BlobError, BlobHeader, SectionOffsets, BLOCK_ID_SIZE,
    BLOCK_REF_SIZE, CALLOUT_SIZE, CODE_SPAN_SIZE, EMBED_SIZE, HEADING_SIZE, LINK_DEF_SIZE,
    LINK_SIZE, QUERY_BLOCK_SIZE, TAG_SIZE, TASK_SIZE,
};
use super::owned::{
    BlockData, BlockRefData, CalloutData, CodeSpanData, DecodedOwnedData, EmbedData, HeadingData,
    LinkDefinitionData, MarkdownData, QueryBlockData, TagData, TaskData, WikiData,
};

pub(super) fn decode_owned_data(
    data: &[u8],
    header: &BlobHeader,
    offsets: &SectionOffsets,
    text_pool: &[u8],
) -> Result<DecodedOwnedData, BlobError> {
    // Owned intermediate structs — collected before entering self_cell closure.
    let mut headings_owned: Vec<HeadingData> = Vec::with_capacity(header.heading_count as usize);
    let mut wiki_owned: Vec<WikiData> = Vec::with_capacity(header.link_count as usize);
    let mut markdown_owned: Vec<MarkdownData> = Vec::with_capacity(header.link_count as usize);
    let mut tags_owned: Vec<TagData> = Vec::with_capacity(header.tag_count as usize);
    let mut blocks_owned: Vec<BlockData> = Vec::with_capacity(header.block_id_count as usize);
    let mut code_spans_owned: Vec<CodeSpanData> =
        Vec::with_capacity(header.code_span_count as usize);
    let mut tasks_owned: Vec<TaskData> = Vec::with_capacity(header.task_count as usize);
    let mut embeds_owned: Vec<EmbedData> = Vec::with_capacity(header.embed_count as usize);

    // ── Headings ────────────────────────────────────────────────
    // BlobHeading layout (40 bytes):
    //   text_off(4@0) text_len(4@4) slug_off(4@8) slug_len(4@12)
    //   source_offset(4@16) start_line(4@20) start_col(4@24)
    //   end_line(4@28) end_col(4@32) level(1@36) _pad(3@37)
    for i in 0..header.heading_count as usize {
        let base = offsets.headings + i * HEADING_SIZE;
        let text_off = read_u32_le(data, base);
        let text_len = read_u32_le(data, base + 4);
        let slug_off = read_u32_le(data, base + 8);
        let slug_len = read_u32_le(data, base + 12);
        let start_line = read_u32_le(data, base + 20);
        let start_col = read_u32_le(data, base + 24);
        let end_line = read_u32_le(data, base + 28);
        let end_col = read_u32_le(data, base + 32);
        let level = read_u8(data, base + 36);

        let text = pool_str(text_pool, text_off, text_len)?.to_owned();
        let slug = pool_str(text_pool, slug_off, slug_len)?.to_owned();

        headings_owned.push(HeadingData {
            text,
            slug,
            start_line,
            start_col,
            end_line,
            end_col,
            level,
        });
    }

    // ── Links ───────────────────────────────────────────────────
    // BlobLink layout (40 bytes):
    //   text_off(4@0) text_len(4@4) target_off(4@8) target_len(4@12)
    //   source_offset(4@16) start_line(4@20) start_col(4@24)
    //   end_line(4@28) end_col(4@32) is_wiki(1@36) _pad(3@37)
    for i in 0..header.link_count as usize {
        let base = offsets.links + i * LINK_SIZE;
        let text_off = read_u32_le(data, base);
        let text_len = read_u32_le(data, base + 4);
        let target_off = read_u32_le(data, base + 8);
        let target_len = read_u32_le(data, base + 12);
        let source_offset = read_u32_le(data, base + 16);
        let start_line = read_u32_le(data, base + 20);
        let start_col = read_u32_le(data, base + 24);
        let end_line = read_u32_le(data, base + 28);
        let end_col = read_u32_le(data, base + 32);
        let is_wiki = read_u8(data, base + 36);

        let text = pool_str(text_pool, text_off, text_len)?;
        let target = pool_str(text_pool, target_off, target_len)?;

        if is_wiki != 0 {
            // Wiki link: text is the display/alias, target may contain
            // a heading anchor (e.g. "page#heading"). Split on '#'.
            let (page, heading) = if let Some(hash_pos) = target.find('#') {
                (&target[..hash_pos], Some(target[hash_pos + 1..].to_owned()))
            } else {
                (target, None)
            };
            // Alias is present only when text ≠ full target (before anchor strip).
            // Comparing against `page` (anchor-stripped) was wrong: [[p#h|p]] would
            // see text="p" == page="p" and produce alias=None. marky-d7hh.
            let alias = if text != target {
                Some(text.to_owned())
            } else {
                None
            };
            wiki_owned.push(WikiData {
                alias,
                heading,
                target: page.to_owned(),
                source_offset,
                text_len,
                target_len,
                start_line,
                start_col,
                end_line,
                end_col,
            });
        } else {
            // Markdown link: split target on '#' for url + anchor.
            let (url, anchor) = if let Some(hash_pos) = target.find('#') {
                (
                    target[..hash_pos].to_owned(),
                    Some(target[hash_pos + 1..].to_owned()),
                )
            } else {
                (target.to_owned(), None)
            };
            markdown_owned.push(MarkdownData {
                text: text.to_owned(),
                url,
                anchor,
                source_offset,
                text_len,
                target_len,
                start_line,
                start_col,
                end_line,
                end_col,
            });
        }
    }

    // ── Tags ────────────────────────────────────────────────────
    // BlobTag layout (24 bytes):
    //   name_off(4@0) name_len(4@4) source_offset(4@8)
    //   start_line(4@12) start_col(4@16) _pad(4@20)
    for i in 0..header.tag_count as usize {
        let base = offsets.tags + i * TAG_SIZE;
        let name_off = read_u32_le(data, base);
        let name_len = read_u32_le(data, base + 4);
        let name = pool_str(text_pool, name_off, name_len)?.to_owned();
        tags_owned.push(TagData { name });
    }

    // ── Block IDs ───────────────────────────────────────────────
    // BlobBlockId layout (28 bytes):
    //   id_off(4@0) id_len(4@4) source_offset(4@8)
    //   start_line(4@12) start_col(4@16) end_line(4@20) end_col(4@24)
    for i in 0..header.block_id_count as usize {
        let base = offsets.block_ids + i * BLOCK_ID_SIZE;
        let id_off = read_u32_le(data, base);
        let id_len = read_u32_le(data, base + 4);
        let source_offset = read_u32_le(data, base + 8);
        let start_line = read_u32_le(data, base + 12);
        let start_col = read_u32_le(data, base + 16);
        let end_line = read_u32_le(data, base + 20);
        let end_col = read_u32_le(data, base + 24);
        let id = pool_str(text_pool, id_off, id_len)?.to_owned();
        blocks_owned.push(BlockData {
            id,
            source_offset,
            id_len,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    // ── Code Spans ────────────────────────────────────────────────
    // BlobCodeSpan layout (32 bytes):
    //   text_off(4@0) text_len(4@4) source_offset(4@8) end_offset(4@12)
    //   start_line(4@16) start_col(4@20) end_line(4@24) end_col(4@28)
    for i in 0..header.code_span_count as usize {
        let base = offsets.code_spans + i * CODE_SPAN_SIZE;
        let text_off = read_u32_le(data, base);
        let text_len = read_u32_le(data, base + 4);
        let source_offset = read_u32_le(data, base + 8);
        let end_offset = read_u32_le(data, base + 12);
        let start_line = read_u32_le(data, base + 16);
        let start_col = read_u32_le(data, base + 20);
        let end_line = read_u32_le(data, base + 24);
        let end_col = read_u32_le(data, base + 28);
        let text = pool_str(text_pool, text_off, text_len)?.to_owned();
        code_spans_owned.push(CodeSpanData {
            text,
            source_offset,
            end_offset,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    // ── Tasks ──────────────────────────────────────────────────
    // BlobTask layout (36 bytes):
    //   text_off(4@0) text_len(4@4) source_offset(4@8) end_offset(4@12)
    //   start_line(4@16) start_col(4@20) end_line(4@24) end_col(4@28)
    //   state(1@32) _pad(3@33)
    for i in 0..header.task_count as usize {
        let base = offsets.tasks + i * TASK_SIZE;
        let text_off = read_u32_le(data, base);
        let text_len = read_u32_le(data, base + 4);
        let source_offset = read_u32_le(data, base + 8);
        let end_offset = read_u32_le(data, base + 12);
        let start_line = read_u32_le(data, base + 16);
        let start_col = read_u32_le(data, base + 20);
        let end_line = read_u32_le(data, base + 24);
        let end_col = read_u32_le(data, base + 28);
        let state_byte = read_u8(data, base + 32);
        let text = pool_str(text_pool, text_off, text_len)?.to_owned();
        let state = match state_byte {
            b'x' | b'X' => "checked",
            _ => "unchecked",
        }
        .to_owned();
        tasks_owned.push(TaskData {
            state,
            text,
            source_offset,
            end_offset,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    // ── Embeds ────────────────────────────────────────────────────
    // BlobEmbed layout (32 bytes):
    //   target_off(4@0) target_len(4@4) source_offset(4@8) end_offset(4@12)
    //   start_line(4@16) start_col(4@20) end_line(4@24) end_col(4@28)
    for i in 0..header.embed_count as usize {
        let base = offsets.embeds + i * EMBED_SIZE;
        let target_off = read_u32_le(data, base);
        let target_len = read_u32_le(data, base + 4);
        let source_offset = read_u32_le(data, base + 8);
        let end_offset = read_u32_le(data, base + 12);
        let start_line = read_u32_le(data, base + 16);
        let start_col = read_u32_le(data, base + 20);
        let end_line = read_u32_le(data, base + 24);
        let end_col = read_u32_le(data, base + 28);
        let target = pool_str(text_pool, target_off, target_len)?.to_owned();
        embeds_owned.push(EmbedData {
            target,
            source_offset,
            end_offset,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    // ── Callouts ──────────────────────────────────────────────────
    // BlobCallout layout (40 bytes):
    //   type_off(4@0) type_len(4@4) title_off(4@8) title_len(4@12)
    //   source_offset(4@16) end_offset(4@20) start_line(4@24) start_col(4@28)
    //   end_line(4@32) end_col(4@36)
    let mut callouts_owned: Vec<CalloutData> =
        Vec::with_capacity(header.callout_count as usize);
    for i in 0..header.callout_count as usize {
        let base = offsets.callouts + i * CALLOUT_SIZE;
        let type_off = read_u32_le(data, base);
        let type_len = read_u32_le(data, base + 4);
        let title_off = read_u32_le(data, base + 8);
        let title_len = read_u32_le(data, base + 12);
        let source_offset = read_u32_le(data, base + 16);
        let end_offset = read_u32_le(data, base + 20);
        let start_line = read_u32_le(data, base + 24);
        let start_col = read_u32_le(data, base + 28);
        let end_line = read_u32_le(data, base + 32);
        let end_col = read_u32_le(data, base + 36);
        let callout_type = pool_str(text_pool, type_off, type_len)?.to_owned();
        let title = if title_len == 0 {
            None
        } else {
            Some(pool_str(text_pool, title_off, title_len)?.to_owned())
        };
        callouts_owned.push(CalloutData {
            callout_type,
            title,
            source_offset,
            end_offset,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    // ── Block Refs ────────────────────────────────────────────────
    // BlobBlockRef layout (28 bytes):
    //   uuid_off(4@0) uuid_len(4@4) source_offset(4@8)
    //   start_line(4@12) start_col(4@16) end_line(4@20) end_col(4@24)
    let mut block_refs_owned: Vec<BlockRefData> =
        Vec::with_capacity(header.block_ref_count as usize);
    for i in 0..header.block_ref_count as usize {
        let base = offsets.block_refs + i * BLOCK_REF_SIZE;
        let uuid_off = read_u32_le(data, base);
        let uuid_len = read_u32_le(data, base + 4);
        let _source_offset = read_u32_le(data, base + 8);
        let start_line = read_u32_le(data, base + 12);
        let start_col = read_u32_le(data, base + 16);
        let end_line = read_u32_le(data, base + 20);
        let end_col = read_u32_le(data, base + 24);
        let uuid = pool_str(text_pool, uuid_off, uuid_len)?.to_owned();
        block_refs_owned.push(BlockRefData {
            uuid,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    // ── Query Blocks ──────────────────────────────────────────────
    // BlobQueryBlock layout (32 bytes):
    //   query_off(4@0) query_len(4@4) source_offset(4@8) end_offset(4@12)
    //   start_line(4@16) start_col(4@20) end_line(4@24) end_col(4@28)
    let mut query_blocks_owned: Vec<QueryBlockData> =
        Vec::with_capacity(header.query_block_count as usize);
    for i in 0..header.query_block_count as usize {
        let base = offsets.query_blocks + i * QUERY_BLOCK_SIZE;
        let query_off = read_u32_le(data, base);
        let query_len = read_u32_le(data, base + 4);
        let source_offset = read_u32_le(data, base + 8);
        let end_offset = read_u32_le(data, base + 12);
        let start_line = read_u32_le(data, base + 16);
        let start_col = read_u32_le(data, base + 20);
        let end_line = read_u32_le(data, base + 24);
        let end_col = read_u32_le(data, base + 28);
        let query = pool_str(text_pool, query_off, query_len)?.to_owned();
        query_blocks_owned.push(QueryBlockData {
            query,
            source_offset,
            end_offset,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    // ── Link Definitions ─────────────────────────────────────────
    // BlobLinkDefinition layout (48 bytes):
    //   label_off(4@0) label_len(4@4) url_off(4@8) url_len(4@12)
    //   title_off(4@16) title_len(4@20) source_offset(4@24) end_offset(4@28)
    //   start_line(4@32) start_col(4@36) end_line(4@40) end_col(4@44)
    let mut link_defs_owned: Vec<LinkDefinitionData> =
        Vec::with_capacity(header.link_def_count as usize);
    for i in 0..header.link_def_count as usize {
        let base = offsets.link_definitions + i * LINK_DEF_SIZE;
        let label_off = read_u32_le(data, base);
        let label_len = read_u32_le(data, base + 4);
        let url_off = read_u32_le(data, base + 8);
        let url_len = read_u32_le(data, base + 12);
        let title_off = read_u32_le(data, base + 16);
        let title_len = read_u32_le(data, base + 20);
        let source_offset = read_u32_le(data, base + 24);
        let end_offset = read_u32_le(data, base + 28);
        let start_line = read_u32_le(data, base + 32);
        let start_col = read_u32_le(data, base + 36);
        let end_line = read_u32_le(data, base + 40);
        let end_col = read_u32_le(data, base + 44);
        let label = pool_str(text_pool, label_off, label_len)?.to_owned();
        let url = pool_str(text_pool, url_off, url_len)?.to_owned();
        let title = if title_len == 0 {
            None
        } else {
            Some(pool_str(text_pool, title_off, title_len)?.to_owned())
        };
        link_defs_owned.push(LinkDefinitionData {
            label,
            url,
            title,
            source_offset,
            end_offset,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    Ok(DecodedOwnedData {
        headings: headings_owned,
        wiki_links: wiki_owned,
        markdown_links: markdown_owned,
        tags: tags_owned,
        blocks: blocks_owned,
        code_spans: code_spans_owned,
        tasks: tasks_owned,
        embeds: embeds_owned,
        callouts: callouts_owned,
        block_refs: block_refs_owned,
        query_blocks: query_blocks_owned,
        link_definitions: link_defs_owned,
    })
}
