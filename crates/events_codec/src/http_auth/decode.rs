#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    http_auth::{KIND_HTTP_AUTH, RadrootsHttpAuth},
    tags::{TAG_METHOD, TAG_PAYLOAD, TAG_URL_AUTH},
};

use crate::error::EventParseError;
use crate::field_helpers::{
    optional_tag_value, require_empty_content, required_tag_value, validate_lowercase_hex_64_tag,
};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const EXPECTED_KIND: &str = "27235";

pub fn http_auth_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsHttpAuth, EventParseError> {
    if kind != KIND_HTTP_AUTH {
        return Err(EventParseError::InvalidKind {
            expected: EXPECTED_KIND,
            got: kind,
        });
    }
    require_empty_content(content, "content")?;
    let payload_sha256 = optional_tag_value(tags, TAG_PAYLOAD)?;
    if let Some(payload) = payload_sha256.as_deref() {
        validate_lowercase_hex_64_tag(payload, TAG_PAYLOAD)?;
    }
    Ok(RadrootsHttpAuth {
        url: required_tag_value(tags, TAG_URL_AUTH)?,
        method: required_tag_value(tags, TAG_METHOD)?,
        payload_sha256,
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsHttpAuth>, EventParseError> {
    let auth = http_auth_from_event(kind, &tags, &content)?;
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
) -> Result<RadrootsParsedEvent<RadrootsHttpAuth>, EventParseError> {
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
