#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    kinds::{KIND_LIST_READ_WRITE_RELAYS, is_nip51_list_set_kind, is_nip51_standard_list_kind},
    list::{RadrootsList, RadrootsListEntry},
    tags::TAG_R,
};

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

fn entry_tag(entry: &RadrootsListEntry) -> Result<Vec<String>, EventEncodeError> {
    if entry.tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("entry.tag"));
    }
    let first = entry
        .values
        .get(0)
        .ok_or(EventEncodeError::EmptyRequiredField("entry.values"))?;
    if first.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("entry.values"));
    }
    let mut tag = Vec::with_capacity(1 + entry.values.len());
    tag.push(entry.tag.clone());
    tag.extend(entry.values.iter().cloned());
    Ok(tag)
}

pub fn list_entries_to_tags(
    entries: &[RadrootsListEntry],
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::with_capacity(entries.len());
    for entry in entries {
        tags.push(entry_tag(entry)?);
    }
    Ok(tags)
}

pub fn list_build_tags(list: &RadrootsList) -> Result<Vec<Vec<String>>, EventEncodeError> {
    list_entries_to_tags(&list.entries)
}

pub fn to_wire_parts_with_kind(
    list: &RadrootsList,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if !is_supported_list_kind(kind) {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    if kind == KIND_LIST_READ_WRITE_RELAYS {
        validate_relay_entries(&list.entries)?;
    }
    let tags = list_build_tags(list)?;
    Ok(WireEventParts {
        kind,
        content: list.content.clone(),
        tags,
    })
}

fn is_supported_list_kind(kind: u32) -> bool {
    is_nip51_standard_list_kind(kind) || is_nip51_list_set_kind(kind)
}

fn validate_relay_entries(entries: &[RadrootsListEntry]) -> Result<(), EventEncodeError> {
    if entries.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("relay.entries"));
    }
    for entry in entries {
        if entry.tag != TAG_R {
            return Err(EventEncodeError::InvalidField("relay.tag"));
        }
        let Some(url) = entry.values.first() else {
            return Err(EventEncodeError::EmptyRequiredField("relay.url"));
        };
        if !is_ws_relay_url(url) {
            return Err(EventEncodeError::InvalidField("relay.url"));
        }
        if entry.values.len() > 2 {
            return Err(EventEncodeError::InvalidField("relay.marker"));
        }
        if let Some(marker) = entry.values.get(1) {
            if marker != "read" && marker != "write" {
                return Err(EventEncodeError::InvalidField("relay.marker"));
            }
        }
    }
    Ok(())
}

fn is_ws_relay_url(value: &str) -> bool {
    (value.starts_with("wss://") && value.len() > "wss://".len())
        || (value.starts_with("ws://") && value.len() > "ws://".len())
}

#[cfg(feature = "serde_json")]
pub fn list_private_entries_json(
    entries: &[RadrootsListEntry],
) -> Result<String, EventEncodeError> {
    let tags = list_entries_to_tags(entries)?;
    serde_json::to_string(&tags).map_err(|_| EventEncodeError::Json)
}
