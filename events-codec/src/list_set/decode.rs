#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::is_nip51_list_set_kind,
    list::{RadrootsListEntry},
    list_set::{RadrootsListSet, RadrootsListSetEventIndex, RadrootsListSetEventMetadata},
};

use crate::error::EventParseError;
#[cfg(feature = "serde_json")]
use crate::list::decode::list_entries_from_tags;

const TAG_D: &str = "d";
const TAG_TITLE: &str = "title";
const TAG_DESCRIPTION: &str = "description";
const TAG_IMAGE: &str = "image";

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

fn take_first_non_empty(tag: &[String]) -> Option<String> {
    tag.get(1)
        .filter(|v| !v.trim().is_empty())
        .cloned()
}

pub fn list_set_from_tags(
    kind: u32,
    content: String,
    tags: &[Vec<String>],
) -> Result<RadrootsListSet, EventParseError> {
    if !is_nip51_list_set_kind(kind) {
        return Err(EventParseError::InvalidKind {
            expected: "nip51 list set kind",
            got: kind,
        });
    }
    let mut d_tag: Option<String> = None;
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut image: Option<String> = None;
    let mut entries = Vec::new();

    for tag in tags.iter().filter(|t| t.len() >= 2) {
        let name = tag.get(0).ok_or(EventParseError::InvalidTag("tag"))?;
        if name.trim().is_empty() {
            return Err(EventParseError::InvalidTag("tag"));
        }
        match name.as_str() {
            TAG_D => {
                if d_tag.is_none() {
                    let value = tag.get(1).ok_or(EventParseError::InvalidTag("d"))?;
                    if value.trim().is_empty() {
                        return Err(EventParseError::InvalidTag("d"));
                    }
                    d_tag = Some(value.clone());
                }
            }
            TAG_TITLE => {
                if title.is_none() {
                    title = take_first_non_empty(tag);
                }
            }
            TAG_DESCRIPTION => {
                if description.is_none() {
                    description = take_first_non_empty(tag);
                }
            }
            TAG_IMAGE => {
                if image.is_none() {
                    image = take_first_non_empty(tag);
                }
            }
            _ => {
                entries.push(entry_from_tag(tag)?);
            }
        }
    }

    let d_tag = d_tag.ok_or(EventParseError::MissingTag("d"))?;
    Ok(RadrootsListSet {
        d_tag,
        content,
        entries,
        title,
        description,
        image,
    })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsListSetEventMetadata, EventParseError> {
    let list_set = list_set_from_tags(kind, content, &tags)?;
    Ok(RadrootsListSetEventMetadata {
        id,
        author,
        published_at,
        kind,
        list_set,
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
) -> Result<RadrootsListSetEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsListSetEventIndex {
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
pub fn list_set_private_entries_from_json(
    content: &str,
) -> Result<Vec<RadrootsListEntry>, EventParseError> {
    let tags: Vec<Vec<String>> =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;
    list_entries_from_tags(&tags)
}
