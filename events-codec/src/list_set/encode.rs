#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    kinds::is_nip51_list_set_kind,
    list_set::RadrootsListSet,
};

use crate::error::EventEncodeError;
#[cfg(feature = "serde_json")]
use crate::list::encode::list_entries_to_tags;
use crate::wire::WireEventParts;

const TAG_D: &str = "d";
const TAG_TITLE: &str = "title";
const TAG_DESCRIPTION: &str = "description";
const TAG_IMAGE: &str = "image";

fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    let mut tag = Vec::with_capacity(2);
    tag.push(key.to_string());
    tag.push(value.to_string());
    tags.push(tag);
}

pub fn list_set_build_tags(list: &RadrootsListSet) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if list.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    let mut tags = Vec::with_capacity(1 + list.entries.len() + 3);
    push_tag(&mut tags, TAG_D, &list.d_tag);
    if let Some(title) = list.title.as_ref().filter(|v| !v.trim().is_empty()) {
        push_tag(&mut tags, TAG_TITLE, title);
    }
    if let Some(description) = list.description.as_ref().filter(|v| !v.trim().is_empty()) {
        push_tag(&mut tags, TAG_DESCRIPTION, description);
    }
    if let Some(image) = list.image.as_ref().filter(|v| !v.trim().is_empty()) {
        push_tag(&mut tags, TAG_IMAGE, image);
    }
    for entry in &list.entries {
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
        tags.push(tag);
    }
    Ok(tags)
}

pub fn to_wire_parts_with_kind(
    list: &RadrootsListSet,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if !is_nip51_list_set_kind(kind) {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = list_set_build_tags(list)?;
    Ok(WireEventParts {
        kind,
        content: list.content.clone(),
        tags,
    })
}

#[cfg(feature = "serde_json")]
pub fn list_set_private_entries_json(
    entries: &[radroots_events::list::RadrootsListEntry],
) -> Result<String, EventEncodeError> {
    let tags = list_entries_to_tags(entries)?;
    serde_json::to_string(&tags).map_err(|_| EventEncodeError::Json)
}
