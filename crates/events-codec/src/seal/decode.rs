#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{RadrootsNostrEvent, kinds::KIND_SEAL, seal::RadrootsSeal};

use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const DEFAULT_KIND: u32 = KIND_SEAL;

pub fn seal_from_parts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsSeal, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "13",
            got: kind,
        });
    }
    if !tags.is_empty() {
        return Err(EventParseError::InvalidTag("tags"));
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }
    Ok(RadrootsSeal {
        content: content.to_string(),
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsSeal>, EventParseError> {
    let seal = seal_from_parts(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        seal,
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
) -> Result<RadrootsParsedEvent<RadrootsSeal>, EventParseError> {
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
