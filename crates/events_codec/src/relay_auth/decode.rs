#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    relay_auth::{KIND_RELAY_AUTH, RadrootsRelayAuth},
    tags::{TAG_CHALLENGE, TAG_RELAY},
};

use crate::error::EventParseError;
use crate::field_helpers::{require_empty_content, required_tag_value};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const EXPECTED_KIND: &str = "22242";

pub fn relay_auth_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsRelayAuth, EventParseError> {
    if kind != KIND_RELAY_AUTH {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        });
    }
    require_empty_content(content, "content")?;
    Ok(RadrootsRelayAuth {
        relay: required_tag_value(tags, TAG_RELAY)?,
        challenge: required_tag_value(tags, TAG_CHALLENGE)?,
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsRelayAuth>, EventParseError> {
    let auth = relay_auth_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        auth,
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
) -> Result<RadrootsParsedEvent<RadrootsRelayAuth>, EventParseError> {
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
