#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::is_nip51_standard_list_kind,
    list::{RadrootsList, RadrootsListEntry, RadrootsListEventIndex, RadrootsListEventMetadata},
};

use crate::error::EventParseError;

fn entry_from_tag(tag: &[String]) -> Result<RadrootsListEntry, EventParseError> {
    let name = tag.get(0).ok_or(EventParseError::InvalidTag("tag"))?;
    if name.trim().is_empty() {
        return Err(EventParseError::InvalidTag("tag"));
    }
    let value = tag.get(1).ok_or(EventParseError::InvalidTag("tag"))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag("tag"));
    }
    Ok(RadrootsListEntry {
        tag: name.clone(),
        values: tag[1..].to_vec(),
    })
}

pub fn list_entries_from_tags(
    tags: &[Vec<String>],
) -> Result<Vec<RadrootsListEntry>, EventParseError> {
    let mut entries = Vec::with_capacity(tags.len());
    for tag in tags.iter().filter(|t| t.len() >= 2) {
        entries.push(entry_from_tag(tag)?);
    }
    Ok(entries)
}

pub fn list_from_tags(
    kind: u32,
    content: String,
    tags: &[Vec<String>],
) -> Result<RadrootsList, EventParseError> {
    if !is_nip51_standard_list_kind(kind) {
        return Err(EventParseError::InvalidKind {
            expected: "nip51 standard list kind",
            got: kind,
        });
    }
    let entries = list_entries_from_tags(tags)?;
    Ok(RadrootsList { content, entries })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsListEventMetadata, EventParseError> {
    let list = list_from_tags(kind, content, &tags)?;
    Ok(RadrootsListEventMetadata {
        id,
        author,
        published_at,
        kind,
        list,
    })
}

pub fn index_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsListEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsListEventIndex {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        metadata,
    })
}

#[cfg(feature = "serde_json")]
pub fn list_private_entries_from_json(
    content: &str,
) -> Result<Vec<RadrootsListEntry>, EventParseError> {
    let tags: Vec<Vec<String>> =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;
    list_entries_from_tags(&tags)
}
