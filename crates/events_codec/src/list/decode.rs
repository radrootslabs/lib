#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::{KIND_LIST_READ_WRITE_RELAYS, is_nip51_list_set_kind, is_nip51_standard_list_kind},
    list::{RadrootsList, RadrootsListEntry},
    tags::TAG_R,
};

use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

fn entry_from_tag(tag: &[String]) -> Result<RadrootsListEntry, EventParseError> {
    let name = &tag[0];
    if name.trim().is_empty() {
        return Err(EventParseError::InvalidTag("tag"));
    }
    let value = &tag[1];
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
    if !is_supported_list_kind(kind) {
        return Err(EventParseError::InvalidKind {
            expected: "nip51 standard or list-set kind",
            got: kind,
        });
    }
    if kind == KIND_LIST_READ_WRITE_RELAYS {
        validate_relay_tags(tags)?;
    }
    let entries = list_entries_from_tags(tags)?;
    Ok(RadrootsList { content, entries })
}

fn is_supported_list_kind(kind: u32) -> bool {
    is_nip51_standard_list_kind(kind) || is_nip51_list_set_kind(kind)
}

fn validate_relay_tags(tags: &[Vec<String>]) -> Result<(), EventParseError> {
    if tags.is_empty() {
        return Err(EventParseError::MissingTag(TAG_R));
    }
    for tag in tags {
        if tag.first().map(|value| value.as_str()) != Some(TAG_R) {
            return Err(EventParseError::InvalidTag(TAG_R));
        }
        let Some(url) = tag.get(1) else {
            return Err(EventParseError::InvalidTag(TAG_R));
        };
        if !is_ws_relay_url(url) {
            return Err(EventParseError::InvalidTag(TAG_R));
        }
        if tag.len() > 3 {
            return Err(EventParseError::InvalidTag(TAG_R));
        }
        if let Some(marker) = tag.get(2) {
            if marker != "read" && marker != "write" {
                return Err(EventParseError::InvalidTag(TAG_R));
            }
        }
    }
    Ok(())
}

fn is_ws_relay_url(value: &str) -> bool {
    (value.starts_with("wss://") && value.len() > "wss://".len())
        || (value.starts_with("ws://") && value.len() > "ws://".len())
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsList>, EventParseError> {
    let list = list_from_tags(kind, content, &tags)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        list,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsList>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsParsedEvent {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        data,
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
