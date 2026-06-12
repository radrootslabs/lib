#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{RadrootsNostrEvent, farm::RadrootsFarm, kinds::KIND_FARM, tags::TAG_D};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const DEFAULT_KIND: u32 = KIND_FARM;

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

pub fn farm_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsFarm, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "30340",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = parse_d_tag(tags)?;
    reject_private_farm_ops_content(content)?;
    let mut farm: RadrootsFarm =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;

    if farm.d_tag.trim().is_empty() {
        farm.d_tag = d_tag;
    } else if farm.d_tag != d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }

    Ok(farm)
}

fn reject_private_farm_ops_content(content: &str) -> Result<(), EventParseError> {
    let value: serde_json::Value =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;
    let Some(object) = value.as_object() else {
        return Err(EventParseError::InvalidJson("content"));
    };
    for key in [
        "workspace",
        "farm_group_id",
        "document_id",
        "document_kind",
        "crdt_backend",
        "encoded_change",
        "semantic_kind",
        "owner_document_kind",
        "owner_document_id",
        "relays",
        "media_servers",
        "supported_kinds",
        "protocol_version",
    ] {
        if object.contains_key(key) {
            return Err(EventParseError::InvalidJson("content"));
        }
    }
    Ok(())
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsFarm>, EventParseError> {
    let farm = farm_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        farm,
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
) -> Result<RadrootsParsedEvent<RadrootsFarm>, EventParseError> {
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
