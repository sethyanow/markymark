//! Incremental update helpers for XML-tag entries.

use markymark_index::XmlTagOwned;
use markymark_parser::InputEdit;

use super::{
    adjust_bytes_after_edit, adjust_range_after_edit, any_edit_in_entry_gap, range_intersects_edit,
    range_is_after_edit_end, range_within_neighbor_window, range_within_new_end_window,
};

/// Returns true if this XML tag is affected by any of the pending edits.
///
/// Entries that directly intersect the edit or are within the byte-level
/// neighbor window need re-extraction.
pub fn xml_tag_affected_by_edits(xt: &XmlTagOwned, pending_edits: &[InputEdit]) -> bool {
    pending_edits.iter().any(|edit| {
        range_intersects_edit(xt.range, edit)
            || range_within_neighbor_window(xt.start_byte, xt.end_byte, edit, 100)
    })
}

/// Returns true if any XML tag in the old index needs re-extraction.
pub fn xml_tags_need_update(old_xts: &[XmlTagOwned], pending_edits: &[InputEdit]) -> bool {
    if pending_edits.is_empty() {
        return false;
    }
    let byte_ranges: Vec<(usize, usize)> = old_xts
        .iter()
        .map(|xt| (xt.start_byte, xt.end_byte))
        .collect();
    old_xts
        .iter()
        .any(|xt| xml_tag_affected_by_edits(xt, pending_edits))
        || any_edit_starts_at_or_after_last_xml_tag(old_xts, pending_edits)
        || any_edit_in_entry_gap(&byte_ranges, pending_edits, 100)
}

/// Returns true if any edit starts at or after the last XML tag end.
pub fn any_edit_starts_at_or_after_last_xml_tag(
    old_xts: &[XmlTagOwned],
    pending_edits: &[InputEdit],
) -> bool {
    let Some(last_old_end) = old_xts
        .iter()
        .map(|xt| (xt.range.end.line, xt.range.end.character))
        .max()
    else {
        return false;
    };

    pending_edits.iter().any(|edit| {
        let edit_start = (
            edit.start_position.row as u32,
            edit.start_position.column as u32,
        );
        edit_start >= last_old_end
    })
}

/// Extract all XML tags from the AST as owned data with sorted attributes.
pub fn extract_xml_tags_owned(ast: &markymark_parser::Ast) -> Vec<XmlTagOwned> {
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
}

/// Merge old and new XML tags using selective purge-and-replace.
pub fn merge_incremental_xml_tags(
    old_xts: &[XmlTagOwned],
    new_xts: &[XmlTagOwned],
    pending_edits: &[InputEdit],
) -> Vec<XmlTagOwned> {
    let mut merged = Vec::new();
    for old in old_xts {
        if !xml_tag_affected_by_edits(old, pending_edits) {
            let mut adjusted = old.clone();
            for edit in pending_edits {
                if range_is_after_edit_end(adjusted.range, edit) {
                    adjust_range_after_edit(&mut adjusted.range, edit);
                    adjust_bytes_after_edit(&mut adjusted.start_byte, &mut adjusted.end_byte, edit);
                }
            }
            merged.push(adjusted);
        }
    }
    for new_xt in new_xts {
        if xml_tag_affected_by_edits(new_xt, pending_edits)
            || pending_edits.iter().any(|edit| {
                range_within_new_end_window(new_xt.start_byte, new_xt.end_byte, edit, 100)
            })
        {
            merged.push(new_xt.clone());
        }
    }
    merged.sort_by_key(|xt| (xt.range.start.line, xt.range.start.character));
    merged
}
