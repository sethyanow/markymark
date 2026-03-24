//! CEngineResult FFI mirrors and conversion helpers.

use crate::scan::KernelError;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineHeading {
    pub text_offset: u32,
    pub text_length: u32,
    pub slug_offset: u32,
    pub slug_length: u32,
    pub source_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub level: u8,
    pub _pad: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineLink {
    pub text_offset: u32,
    pub text_length: u32,
    pub target_offset: u32,
    pub target_length: u32,
    pub source_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub is_wiki: u8,
    pub _pad: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineCodeSpan {
    pub text_offset: u32,
    pub text_length: u32,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineTag {
    pub name_offset: u32,
    pub name_length: u32,
    pub source_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineBlockId {
    pub id_offset: u32,
    pub id_length: u32,
    pub source_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineTask {
    pub text_offset: u32,
    pub text_length: u32,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub state: u8,
    pub _pad: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineEmbed {
    pub target_offset: u32,
    pub target_length: u32,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineCallout {
    pub type_offset: u32,
    pub type_length: u32,
    pub title_offset: u32,
    pub title_length: u32,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineBlockRef {
    pub uuid_offset: u32,
    pub uuid_length: u32,
    pub source_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineQueryBlock {
    pub query_offset: u32,
    pub query_length: u32,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineLinkDefinition {
    pub label_offset: u32,
    pub label_length: u32,
    pub url_offset: u32,
    pub url_length: u32,
    pub title_offset: u32,
    pub title_length: u32,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineProperty {
    pub key_offset: u32,
    pub key_length: u32,
    pub value_offset: u32,
    pub value_length: u32,
    pub value_type: u8,
    pub _pad: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CEngineXmlTag {
    pub tag_name_offset: u32,
    pub tag_name_length: u32,
    pub raw_html_offset: u32,
    pub raw_html_length: u32,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub is_self_closing: u8,
    pub is_unclosed: u8,
    pub is_inline: u8,
    pub _pad: [u8; 1],
}

#[repr(C)]
pub struct CEngineResult {
    pub headings: *mut CEngineHeading,
    pub links: *mut CEngineLink,
    pub code_spans: *mut CEngineCodeSpan,
    pub tags: *mut CEngineTag,
    pub block_ids: *mut CEngineBlockId,
    pub tasks: *mut CEngineTask,
    pub embeds: *mut CEngineEmbed,
    pub callouts: *mut CEngineCallout,
    pub block_refs: *mut CEngineBlockRef,
    pub query_blocks: *mut CEngineQueryBlock,
    pub link_definitions: *mut CEngineLinkDefinition,
    pub properties: *mut CEngineProperty,
    pub xml_tags: *mut CEngineXmlTag,
    pub line_starts: *mut u32,
    pub text_blob: *const u8,

    pub content_hash: u64,
    pub generation: u64,

    pub headings_count: u32,
    pub links_count: u32,
    pub code_spans_count: u32,
    pub tags_count: u32,
    pub block_ids_count: u32,
    pub tasks_count: u32,
    pub embeds_count: u32,
    pub callouts_count: u32,
    pub block_refs_count: u32,
    pub query_blocks_count: u32,
    pub link_definitions_count: u32,
    pub properties_count: u32,
    pub xml_tags_count: u32,
    pub line_starts_count: u32,
    pub text_blob_len: u32,
    pub token_estimate: u32,

    pub _reserved: [u8; 32],
}

const _: () = assert!(std::mem::size_of::<CEngineHeading>() == 40);
const _: () = assert!(std::mem::size_of::<CEngineLink>() == 40);
const _: () = assert!(std::mem::size_of::<CEngineCodeSpan>() == 32);
const _: () = assert!(std::mem::size_of::<CEngineTag>() == 20);
const _: () = assert!(std::mem::size_of::<CEngineBlockId>() == 28);
const _: () = assert!(std::mem::size_of::<CEngineTask>() == 36);
const _: () = assert!(std::mem::size_of::<CEngineEmbed>() == 32);
const _: () = assert!(std::mem::size_of::<CEngineCallout>() == 40);
const _: () = assert!(std::mem::size_of::<CEngineBlockRef>() == 28);
const _: () = assert!(std::mem::size_of::<CEngineQueryBlock>() == 32);
const _: () = assert!(std::mem::size_of::<CEngineLinkDefinition>() == 48);
const _: () = assert!(std::mem::size_of::<CEngineProperty>() == 20);
const _: () = assert!(std::mem::size_of::<CEngineXmlTag>() == 44);
const _: () = assert!(std::mem::size_of::<CEngineResult>() == 232);

extern "C" {
    pub(crate) fn marky_engine_get_result(
        handle: *mut std::ffi::c_void,
        out: *mut CEngineResult,
    ) -> i32;
    pub(crate) fn marky_engine_free_result(result: *mut CEngineResult);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineHeading {
    pub text: String,
    pub slug: String,
    pub source_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineWikiLink {
    pub target: String,
    pub alias: Option<String>,
    pub heading: Option<String>,
    pub source_offset: u32,
    pub text_len: u32,
    pub target_len: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineMarkdownLink {
    pub text: String,
    pub url: String,
    pub anchor: Option<String>,
    pub source_offset: u32,
    pub text_len: u32,
    pub target_len: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineTag {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineBlockId {
    pub id: String,
    pub source_offset: u32,
    pub id_len: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineCodeSpan {
    pub text: String,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineTask {
    pub state: String,
    pub text: String,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineEmbed {
    pub target: String,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineCallout {
    pub callout_type: String,
    pub title: Option<String>,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineBlockRef {
    pub uuid: String,
    pub source_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineQueryBlock {
    pub query: String,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineLinkDefinition {
    pub label: String,
    pub url: String,
    pub title: Option<String>,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineProperty {
    pub key: String,
    pub value: String,
    pub value_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineXmlTag {
    pub tag_name: String,
    pub raw_html: String,
    pub source_offset: u32,
    pub end_offset: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub is_self_closing: bool,
    pub is_unclosed: bool,
    pub is_inline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineExtraction {
    pub headings: Vec<EngineHeading>,
    pub wiki_links: Vec<EngineWikiLink>,
    pub markdown_links: Vec<EngineMarkdownLink>,
    pub tags: Vec<EngineTag>,
    pub block_ids: Vec<EngineBlockId>,
    pub code_spans: Vec<EngineCodeSpan>,
    pub tasks: Vec<EngineTask>,
    pub embeds: Vec<EngineEmbed>,
    pub callouts: Vec<EngineCallout>,
    pub block_refs: Vec<EngineBlockRef>,
    pub query_blocks: Vec<EngineQueryBlock>,
    pub link_definitions: Vec<EngineLinkDefinition>,
    pub properties: Vec<EngineProperty>,
    pub xml_tags: Vec<EngineXmlTag>,
    pub line_starts: Vec<u32>,
    pub token_estimate: u32,
    pub content_hash: u64,
    pub generation: u64,
}

pub struct EngineResult {
    raw: CEngineResult,
}

impl EngineResult {
    pub(crate) fn from_raw(raw: CEngineResult) -> Self {
        Self { raw }
    }

    pub fn as_raw(&self) -> &CEngineResult {
        &self.raw
    }

    pub fn to_extraction(&self) -> Result<EngineExtraction, KernelError> {
        convert_engine_result(&self.raw)
    }
}

impl Drop for EngineResult {
    fn drop(&mut self) {
        // SAFETY: `raw` was initialized by marky_engine_get_result on success,
        // and this drop is the unique owner responsible for releasing it.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        unsafe { marky_engine_free_result(&mut self.raw) };
    }
}

impl std::fmt::Debug for EngineResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineResult")
            .field("headings_count", &self.raw.headings_count)
            .field("links_count", &self.raw.links_count)
            .field("code_spans_count", &self.raw.code_spans_count)
            .field("text_blob_len", &self.raw.text_blob_len)
            .field("generation", &self.raw.generation)
            .finish()
    }
}

pub fn safe_text_blob_slice(blob: &[u8], offset: u32, length: u32) -> Result<&[u8], KernelError> {
    let start = usize::try_from(offset).map_err(|_| KernelError::InternalError(-101))?;
    let len = usize::try_from(length).map_err(|_| KernelError::InternalError(-101))?;
    let end = start
        .checked_add(len)
        .ok_or(KernelError::InternalError(-101))?;
    blob.get(start..end).ok_or(KernelError::InternalError(-101))
}

fn read_str(blob: &[u8], offset: u32, length: u32) -> Result<String, KernelError> {
    let bytes = safe_text_blob_slice(blob, offset, length)?;
    let s = std::str::from_utf8(bytes).map_err(|_| KernelError::InternalError(-100))?;
    Ok(s.to_owned())
}

fn ptr_slice<'a, T>(ptr: *const T, count: u32) -> Result<&'a [T], KernelError> {
    if count == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(KernelError::InternalError(-101));
    }
    let len = usize::try_from(count).map_err(|_| KernelError::InternalError(-101))?;
    // SAFETY: caller guarantees `ptr` originates from Zig allocation for `count` elements.
    // We validated non-null for non-zero count, and the returned slice is read-only.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

pub fn convert_engine_result(result: &CEngineResult) -> Result<EngineExtraction, KernelError> {
    let blob = if result.text_blob_len == 0 {
        &[][..]
    } else {
        if result.text_blob.is_null() {
            return Err(KernelError::InternalError(-101));
        }
        let len =
            usize::try_from(result.text_blob_len).map_err(|_| KernelError::InternalError(-101))?;
        // SAFETY: `text_blob` is allocated by Zig and valid for `text_blob_len` bytes
        // until marky_engine_free_result is called.
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage, semgrep.markymark.rust.unsafe-block
        unsafe { std::slice::from_raw_parts(result.text_blob, len) }
    };

    let mut headings = Vec::new();
    for h in ptr_slice(result.headings, result.headings_count)? {
        headings.push(EngineHeading {
            text: read_str(blob, h.text_offset, h.text_length)?,
            slug: read_str(blob, h.slug_offset, h.slug_length)?,
            source_offset: h.source_offset,
            start_line: h.start_line,
            start_col: h.start_col,
            end_line: h.end_line,
            end_col: h.end_col,
            level: h.level,
        });
    }

    let mut wiki_links = Vec::new();
    let mut markdown_links = Vec::new();
    for l in ptr_slice(result.links, result.links_count)? {
        let text = read_str(blob, l.text_offset, l.text_length)?;
        let target = read_str(blob, l.target_offset, l.target_length)?;
        if l.is_wiki != 0 {
            let (page, heading) = if let Some(hash_pos) = target.find('#') {
                (
                    target[..hash_pos].to_owned(),
                    Some(target[hash_pos + 1..].to_owned()),
                )
            } else {
                (target.clone(), None)
            };
            let alias = if text != target { Some(text) } else { None };
            wiki_links.push(EngineWikiLink {
                target: page,
                alias,
                heading,
                source_offset: l.source_offset,
                text_len: l.text_length,
                target_len: l.target_length,
                start_line: l.start_line,
                start_col: l.start_col,
                end_line: l.end_line,
                end_col: l.end_col,
            });
        } else {
            let (url, anchor) = if let Some(hash_pos) = target.find('#') {
                (
                    target[..hash_pos].to_owned(),
                    Some(target[hash_pos + 1..].to_owned()),
                )
            } else {
                (target, None)
            };
            markdown_links.push(EngineMarkdownLink {
                text,
                url,
                anchor,
                source_offset: l.source_offset,
                text_len: l.text_length,
                target_len: l.target_length,
                start_line: l.start_line,
                start_col: l.start_col,
                end_line: l.end_line,
                end_col: l.end_col,
            });
        }
    }

    let mut tags = Vec::new();
    for t in ptr_slice(result.tags, result.tags_count)? {
        tags.push(EngineTag {
            name: read_str(blob, t.name_offset, t.name_length)?,
        });
    }

    let mut block_ids = Vec::new();
    for b in ptr_slice(result.block_ids, result.block_ids_count)? {
        block_ids.push(EngineBlockId {
            id: read_str(blob, b.id_offset, b.id_length)?,
            source_offset: b.source_offset,
            id_len: b.id_length,
            start_line: b.start_line,
            start_col: b.start_col,
            end_line: b.end_line,
            end_col: b.end_col,
        });
    }

    let mut code_spans = Vec::new();
    for c in ptr_slice(result.code_spans, result.code_spans_count)? {
        code_spans.push(EngineCodeSpan {
            text: read_str(blob, c.text_offset, c.text_length)?,
            source_offset: c.source_offset,
            end_offset: c.end_offset,
            start_line: c.start_line,
            start_col: c.start_col,
            end_line: c.end_line,
            end_col: c.end_col,
        });
    }

    let mut tasks = Vec::new();
    for t in ptr_slice(result.tasks, result.tasks_count)? {
        let state = if t.state == b'x' || t.state == b'X' {
            "checked"
        } else {
            "unchecked"
        };
        tasks.push(EngineTask {
            state: state.to_owned(),
            text: read_str(blob, t.text_offset, t.text_length)?,
            source_offset: t.source_offset,
            end_offset: t.end_offset,
            start_line: t.start_line,
            start_col: t.start_col,
            end_line: t.end_line,
            end_col: t.end_col,
        });
    }

    let mut embeds = Vec::new();
    for e in ptr_slice(result.embeds, result.embeds_count)? {
        embeds.push(EngineEmbed {
            target: read_str(blob, e.target_offset, e.target_length)?,
            source_offset: e.source_offset,
            end_offset: e.end_offset,
            start_line: e.start_line,
            start_col: e.start_col,
            end_line: e.end_line,
            end_col: e.end_col,
        });
    }

    let mut callouts = Vec::new();
    for c in ptr_slice(result.callouts, result.callouts_count)? {
        let title = if c.title_length == 0 {
            None
        } else {
            Some(read_str(blob, c.title_offset, c.title_length)?)
        };
        callouts.push(EngineCallout {
            callout_type: read_str(blob, c.type_offset, c.type_length)?,
            title,
            source_offset: c.source_offset,
            end_offset: c.end_offset,
            start_line: c.start_line,
            start_col: c.start_col,
            end_line: c.end_line,
            end_col: c.end_col,
        });
    }

    let mut block_refs = Vec::new();
    for b in ptr_slice(result.block_refs, result.block_refs_count)? {
        block_refs.push(EngineBlockRef {
            uuid: read_str(blob, b.uuid_offset, b.uuid_length)?,
            source_offset: b.source_offset,
            start_line: b.start_line,
            start_col: b.start_col,
            end_line: b.end_line,
            end_col: b.end_col,
        });
    }

    let mut query_blocks = Vec::new();
    for q in ptr_slice(result.query_blocks, result.query_blocks_count)? {
        query_blocks.push(EngineQueryBlock {
            query: read_str(blob, q.query_offset, q.query_length)?,
            source_offset: q.source_offset,
            end_offset: q.end_offset,
            start_line: q.start_line,
            start_col: q.start_col,
            end_line: q.end_line,
            end_col: q.end_col,
        });
    }

    let mut link_definitions = Vec::new();
    for l in ptr_slice(result.link_definitions, result.link_definitions_count)? {
        let title = if l.title_length == 0 {
            None
        } else {
            Some(read_str(blob, l.title_offset, l.title_length)?)
        };
        link_definitions.push(EngineLinkDefinition {
            label: read_str(blob, l.label_offset, l.label_length)?,
            url: read_str(blob, l.url_offset, l.url_length)?,
            title,
            source_offset: l.source_offset,
            end_offset: l.end_offset,
            start_line: l.start_line,
            start_col: l.start_col,
            end_line: l.end_line,
            end_col: l.end_col,
        });
    }

    let mut properties = Vec::new();
    for p in ptr_slice(result.properties, result.properties_count)? {
        properties.push(EngineProperty {
            key: read_str(blob, p.key_offset, p.key_length)?,
            value: read_str(blob, p.value_offset, p.value_length)?,
            value_type: p.value_type,
        });
    }

    let mut xml_tags = Vec::new();
    for x in ptr_slice(result.xml_tags, result.xml_tags_count)? {
        xml_tags.push(EngineXmlTag {
            tag_name: read_str(blob, x.tag_name_offset, x.tag_name_length)?,
            raw_html: read_str(blob, x.raw_html_offset, x.raw_html_length)?,
            source_offset: x.source_offset,
            end_offset: x.end_offset,
            start_line: x.start_line,
            start_col: x.start_col,
            end_line: x.end_line,
            end_col: x.end_col,
            is_self_closing: x.is_self_closing != 0,
            is_unclosed: x.is_unclosed != 0,
            is_inline: x.is_inline != 0,
        });
    }

    let line_starts = ptr_slice(result.line_starts, result.line_starts_count)?.to_vec();

    Ok(EngineExtraction {
        headings,
        wiki_links,
        markdown_links,
        tags,
        block_ids,
        code_spans,
        tasks,
        embeds,
        callouts,
        block_refs,
        query_blocks,
        link_definitions,
        properties,
        xml_tags,
        line_starts,
        token_estimate: result.token_estimate,
        content_hash: result.content_hash,
        generation: result.generation,
    })
}
