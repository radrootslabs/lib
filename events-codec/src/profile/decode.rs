#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    profile::{RadrootsProfile, RadrootsProfileEventIndex, RadrootsProfileEventMetadata},
};

use crate::error::EventParseError;
use serde_json::Value;

const PROFILE_KIND: u32 = 0;

fn parse_optional_string(value: &Value, key: &'static str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn parse_bot(value: &Value) -> Option<String> {
    match value.get("bot") {
        Some(v) if v.is_string() => v.as_str().map(|s| s.to_string()),
        Some(v) if v.is_boolean() => v.as_bool().map(|b| b.to_string()),
        _ => None,
    }
}

pub fn profile_from_content(content: &str) -> Result<RadrootsProfile, EventParseError> {
    let value: Value =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;
    let obj = value
        .as_object()
        .ok_or(EventParseError::InvalidJson("content"))?;
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(EventParseError::InvalidJson("name"))?;

    Ok(RadrootsProfile {
        name: name.to_string(),
        display_name: parse_optional_string(&value, "display_name"),
        nip05: parse_optional_string(&value, "nip05"),
        about: parse_optional_string(&value, "about"),
        website: parse_optional_string(&value, "website"),
        picture: parse_optional_string(&value, "picture"),
        banner: parse_optional_string(&value, "banner"),
        lud06: parse_optional_string(&value, "lud06"),
        lud16: parse_optional_string(&value, "lud16"),
        bot: parse_bot(&value),
    })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    _tags: Vec<Vec<String>>,
) -> Result<RadrootsProfileEventMetadata, EventParseError> {
    if kind != PROFILE_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "0",
            got: kind,
        });
    }
    let profile = profile_from_content(&content)?;
    Ok(RadrootsProfileEventMetadata {
        id,
        author,
        published_at,
        kind,
        profile,
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
) -> Result<RadrootsProfileEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsProfileEventIndex {
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
