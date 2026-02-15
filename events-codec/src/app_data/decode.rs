#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    app_data::{
        KIND_APP_DATA, RadrootsAppData, RadrootsAppDataEventIndex, RadrootsAppDataEventMetadata,
    },
    tags::TAG_D,
};

use crate::error::EventParseError;

fn parse_d_tag(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_D))
        .ok_or(EventParseError::MissingTag(TAG_D))?;
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_D))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    Ok(value)
}

pub fn app_data_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsAppData, EventParseError> {
    if kind != KIND_APP_DATA {
        return Err(EventParseError::InvalidKind {
            expected: "30078",
            got: kind,
        });
    }
    let d_tag = parse_d_tag(tags)?;
    Ok(RadrootsAppData {
        d_tag,
        content: content.to_string(),
    })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsAppDataEventMetadata, EventParseError> {
    let app_data = app_data_from_tags(kind, &tags, &content)?;
    Ok(RadrootsAppDataEventMetadata {
        id,
        author,
        published_at,
        kind,
        app_data,
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
) -> Result<RadrootsAppDataEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsAppDataEventIndex {
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
