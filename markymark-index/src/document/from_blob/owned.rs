#[derive(Clone)]
pub(super) struct HeadingData {
    pub(super) text: String,
    pub(super) slug: String,
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
    pub(super) level: u8,
}

#[derive(Clone)]
pub(super) struct WikiData {
    pub(super) target: String,
    pub(super) alias: Option<String>,
    pub(super) heading: Option<String>,
    pub(super) source_offset: u32,
    pub(super) text_len: u32,   // display/alias text length (for end_byte)
    pub(super) target_len: u32, // page name length (for end_byte)
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

#[derive(Clone)]
pub(super) struct MarkdownData {
    pub(super) text: String,
    pub(super) url: String,
    pub(super) anchor: Option<String>,
    pub(super) source_offset: u32,
    pub(super) text_len: u32,   // link text length (for end_byte)
    pub(super) target_len: u32, // full target length incl. #frag (for end_byte)
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

#[derive(Clone)]
pub(super) struct TagData {
    pub(super) name: String,
}

#[derive(Clone)]
pub(super) struct BlockData {
    pub(super) id: String,
    pub(super) source_offset: u32,
    pub(super) id_len: u32,
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

#[derive(Clone)]
pub(super) struct CodeSpanData {
    pub(super) text: String,
    pub(super) source_offset: u32,
    pub(super) end_offset: u32,
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

#[derive(Clone)]
pub(super) struct TaskData {
    pub(super) state: String,
    pub(super) text: String,
    pub(super) source_offset: u32,
    pub(super) end_offset: u32,
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

#[derive(Clone)]
pub(super) struct EmbedData {
    pub(super) target: String,
    pub(super) source_offset: u32,
    pub(super) end_offset: u32,
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

#[derive(Clone)]
pub(super) struct CalloutData {
    pub(super) callout_type: String,
    pub(super) title: Option<String>,
    pub(super) source_offset: u32,
    pub(super) end_offset: u32,
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

#[derive(Clone)]
pub(super) struct BlockRefData {
    pub(super) uuid: String,
    pub(super) start_line: u32,
    pub(super) start_col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

pub(super) struct DecodedOwnedData {
    pub(super) headings: Vec<HeadingData>,
    pub(super) wiki_links: Vec<WikiData>,
    pub(super) markdown_links: Vec<MarkdownData>,
    pub(super) tags: Vec<TagData>,
    pub(super) blocks: Vec<BlockData>,
    pub(super) code_spans: Vec<CodeSpanData>,
    pub(super) tasks: Vec<TaskData>,
    pub(super) embeds: Vec<EmbedData>,
    pub(super) callouts: Vec<CalloutData>,
    pub(super) block_refs: Vec<BlockRefData>,
}

