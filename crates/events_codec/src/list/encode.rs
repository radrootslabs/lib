#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    kinds::is_nip51_standard_list_kind,
    list::{RadrootsList, RadrootsListEntry},
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
    if !is_nip51_standard_list_kind(kind) {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = list_build_tags(list)?;
    Ok(WireEventParts {
        kind,
        content: list.content.clone(),
        tags,
    })
}

#[cfg(feature = "serde_json")]
pub fn list_private_entries_json(
    entries: &[RadrootsListEntry],
) -> Result<String, EventEncodeError> {
    let tags = list_entries_to_tags(entries)?;
    serde_json::to_string(&tags).map_err(|_| EventEncodeError::Json)
}
