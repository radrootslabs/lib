#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::KIND_RESOURCE_HARVEST_CAP,
    resource_cap::{RadrootsResourceHarvestCap, RadrootsResourceHarvestCapEventIndex, RadrootsResourceHarvestCapEventMetadata},
    tags::TAG_D,
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;

const DEFAULT_KIND: u32 = KIND_RESOURCE_HARVEST_CAP;

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
    validate_d_tag_tag(&value, TAG_D)?;
    Ok(value)
}

pub fn resource_harvest_cap_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsResourceHarvestCap, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "30371",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = parse_d_tag(tags)?;
    let mut cap: RadrootsResourceHarvestCap =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;

    if cap.d_tag.trim().is_empty() {
        cap.d_tag = d_tag;
    } else if cap.d_tag != d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }

    Ok(cap)
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsResourceHarvestCapEventMetadata, EventParseError> {
    let cap = resource_harvest_cap_from_event(kind, &tags, &content)?;
    Ok(RadrootsResourceHarvestCapEventMetadata {
        id,
        author,
        published_at,
        kind,
        cap,
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
) -> Result<RadrootsResourceHarvestCapEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsResourceHarvestCapEventIndex {
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
