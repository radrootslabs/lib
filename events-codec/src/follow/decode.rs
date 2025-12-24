#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    follow::{RadrootsFollow, RadrootsFollowEventIndex, RadrootsFollowEventMetadata, RadrootsFollowProfile},
};

use crate::error::EventParseError;

const DEFAULT_KIND: u32 = 3;

fn looks_like_ws_relay(s: &str) -> bool {
    s.starts_with("ws://") || s.starts_with("wss://")
}

fn parse_follow_tag(
    tag: &[String],
    published_at: u32,
) -> Result<RadrootsFollowProfile, EventParseError> {
    if tag.get(0).map(|s| s.as_str()) != Some("p") {
        return Err(EventParseError::InvalidTag("p"));
    }
    let public_key = tag.get(1).ok_or(EventParseError::InvalidTag("p"))?;
    let (relay_url, contact_name) = match tag.get(2).filter(|s| !s.is_empty()) {
        Some(value) if looks_like_ws_relay(value) => (
            Some(value.clone()),
            tag.get(3).filter(|s| !s.is_empty()).cloned(),
        ),
        Some(value) => (None, Some(value.clone())),
        None => (None, tag.get(3).filter(|s| !s.is_empty()).cloned()),
    };

    let published_at = match tag.get(4) {
        Some(v) => v
            .parse()
            .map_err(|e| EventParseError::InvalidNumber("p", e))?,
        None => published_at,
    };

    Ok(RadrootsFollowProfile {
        published_at,
        public_key: public_key.clone(),
        relay_url,
        contact_name,
    })
}

pub fn follow_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    published_at: u32,
) -> Result<RadrootsFollow, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "3",
            got: kind,
        });
    }
    let mut list = Vec::new();
    for tag in tags.iter().filter(|t| t.get(0).map(|s| s.as_str()) == Some("p")) {
        list.push(parse_follow_tag(tag, published_at)?);
    }
    Ok(RadrootsFollow { list })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    _content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsFollowEventMetadata, EventParseError> {
    let follow = follow_from_tags(kind, &tags, published_at)?;
    Ok(RadrootsFollowEventMetadata {
        id,
        author,
        published_at,
        kind,
        follow,
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
) -> Result<RadrootsFollowEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsFollowEventIndex {
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
