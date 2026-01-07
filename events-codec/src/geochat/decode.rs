#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    geochat::{RadrootsGeoChat, RadrootsGeoChatEventIndex, RadrootsGeoChatEventMetadata},
    kinds::KIND_GEOCHAT,
};

use crate::error::EventParseError;

const DEFAULT_KIND: u32 = KIND_GEOCHAT;
const TAG_G: &str = "g";
const TAG_N: &str = "n";
const TAG_T: &str = "t";
const TAG_T_TELEPORT: &str = "teleport";

fn parse_geohash_tag(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_G))
        .ok_or(EventParseError::MissingTag("g"))?;
    let geohash = tag.get(1).ok_or(EventParseError::InvalidTag("g"))?;
    if geohash.trim().is_empty() {
        return Err(EventParseError::InvalidTag("g"));
    }
    Ok(geohash.to_string())
}

fn parse_nickname_tag(tags: &[Vec<String>]) -> Result<Option<String>, EventParseError> {
    let tag = match tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_N))
    {
        Some(tag) => tag,
        None => return Ok(None),
    };
    let nickname = tag.get(1).ok_or(EventParseError::InvalidTag("n"))?;
    if nickname.trim().is_empty() {
        return Err(EventParseError::InvalidTag("n"));
    }
    Ok(Some(nickname.to_string()))
}

fn parse_teleport_tag(tags: &[Vec<String>]) -> Result<bool, EventParseError> {
    for tag in tags
        .iter()
        .filter(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_T))
    {
        let value = tag.get(1).ok_or(EventParseError::InvalidTag("t"))?;
        if value.trim().is_empty() {
            return Err(EventParseError::InvalidTag("t"));
        }
        if value.eq_ignore_ascii_case(TAG_T_TELEPORT) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn geochat_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsGeoChat, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "20000",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }

    let geohash = parse_geohash_tag(tags)?;
    let nickname = parse_nickname_tag(tags)?;
    let teleported = parse_teleport_tag(tags)?;

    Ok(RadrootsGeoChat {
        geohash,
        content: content.to_string(),
        nickname,
        teleported,
    })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsGeoChatEventMetadata, EventParseError> {
    let geochat = geochat_from_tags(kind, &tags, &content)?;
    Ok(RadrootsGeoChatEventMetadata {
        id,
        author,
        published_at,
        kind,
        geochat,
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
) -> Result<RadrootsGeoChatEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsGeoChatEventIndex {
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
