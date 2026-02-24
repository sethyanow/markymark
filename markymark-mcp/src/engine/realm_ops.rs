//! Realm management operation handlers: CreateRealm, DestroyRealm, AddRoot, RemoveRoot, RealmStats.

use std::collections::HashMap;
use std::path::PathBuf;

use markymark_core::engine::CoreOperationResult;
use markymark_core::CoreError;

use super::{helpers, index_root_into_realm, unindex_root_from_realm, RealmData, DEFAULT_REALM};

pub(crate) fn handle_create_realm(
    state: &mut HashMap<String, RealmData>,
    name: String,
) -> CoreOperationResult {
    if name.is_empty() {
        return CoreOperationResult::Error(CoreError::Message(
            "realm name must not be empty".to_string(),
        ));
    }

    if state.contains_key(&name) {
        return CoreOperationResult::Error(CoreError::Message(format!(
            "realm already exists: {name}"
        )));
    }

    state.insert(name.clone(), RealmData::new());
    CoreOperationResult::RealmInfo {
        name,
        root_count: 0,
        document_count: 0,
    }
}

pub(crate) fn handle_destroy_realm(
    state: &mut HashMap<String, RealmData>,
    name: String,
) -> CoreOperationResult {
    if name == DEFAULT_REALM {
        return CoreOperationResult::Error(CoreError::Message(
            "cannot destroy the default realm".to_string(),
        ));
    }

    if state.remove(&name).is_none() {
        return CoreOperationResult::Error(CoreError::Message(format!(
            "realm does not exist: {name}"
        )));
    }

    CoreOperationResult::Ok
}

pub(crate) fn handle_add_root(
    state: &mut HashMap<String, RealmData>,
    realm: String,
    root: PathBuf,
) -> CoreOperationResult {
    if let Err(msg) = helpers::validate_workspace_root(&root) {
        return CoreOperationResult::Error(CoreError::Message(msg.to_string()));
    }

    let realm_data = match state.get_mut(&realm) {
        Some(data) => data,
        None => {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "realm does not exist: {realm}"
            )));
        }
    };

    // Check for duplicate root (canonicalize for comparison).
    let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
    for existing in &realm_data.roots {
        let existing_canonical = existing.canonicalize().unwrap_or_else(|_| existing.clone());
        if canonical == existing_canonical {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "root already added to realm: {}",
                root.display()
            )));
        }
    }

    index_root_into_realm(&root, realm_data);
    realm_data.roots.push(root);

    CoreOperationResult::RealmInfo {
        name: realm.clone(),
        root_count: realm_data.roots.len(),
        document_count: realm_data.index.document_count(),
    }
}

pub(crate) fn handle_remove_root(
    state: &mut HashMap<String, RealmData>,
    realm: String,
    root: PathBuf,
) -> CoreOperationResult {
    let realm_data = match state.get_mut(&realm) {
        Some(data) => data,
        None => {
            return CoreOperationResult::Error(CoreError::Message(format!(
                "realm does not exist: {realm}"
            )));
        }
    };

    let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
    let pos = realm_data.roots.iter().position(|existing| {
        existing.canonicalize().unwrap_or_else(|_| existing.clone()) == canonical
    });

    match pos {
        Some(idx) => {
            let removed = realm_data.roots.remove(idx);
            unindex_root_from_realm(&removed, realm_data);

            CoreOperationResult::RealmInfo {
                name: realm.clone(),
                root_count: realm_data.roots.len(),
                document_count: realm_data.index.document_count(),
            }
        }
        None => CoreOperationResult::Error(CoreError::Message(format!(
            "root is not tracked in realm: {}",
            root.display()
        ))),
    }
}

pub(crate) fn handle_realm_stats(
    realm_data: &RealmData,
    realm: String,
    check_duplicates: bool,
    include_token_counts: bool,
) -> CoreOperationResult {
    let mut heading_count = 0;
    let mut xml_tag_count = 0;
    let mut wiki_link_count = 0;
    let mut markdown_link_count = 0;

    for (_uri, index) in realm_data.index.iter_documents() {
        heading_count += index.headings().len();
        xml_tag_count += index.xml_tags().len();
        wiki_link_count += index.wiki_links().len();
        markdown_link_count += index.markdown_links().len();
    }

    let duplicate_pairs = if check_duplicates {
        #[cfg(feature = "semantic-search")]
        {
            Some(realm_data.index.detect_semantic_duplicates(0.85).len())
        }
        #[cfg(not(feature = "semantic-search"))]
        {
            None
        }
    } else {
        None
    };

    let total_tokens = if include_token_counts {
        let (total, unreadable_docs) = helpers::total_tokens_for_realm(&realm_data.index);
        if unreadable_docs > 0 {
            log::warn!(
                "token count omitted for realm '{realm}' due to {unreadable_docs} unreadable documents"
            );
            None
        } else {
            Some(total)
        }
    } else {
        None
    };

    CoreOperationResult::RealmStats {
        name: realm,
        root_count: realm_data.roots.len(),
        document_count: realm_data.index.document_count(),
        heading_count,
        xml_tag_count,
        wiki_link_count,
        markdown_link_count,
        structured_doc_count: realm_data.index.structured_count(),
        key_path_count: realm_data.index.key_path_count(),
        duplicate_pairs,
        total_tokens,
    }
}
