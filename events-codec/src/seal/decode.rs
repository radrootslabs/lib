#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::KIND_SEAL,
    seal::{RadrootsSeal, RadrootsSealEventIndex, RadrootsSealEventMetadata},
};

use crate::error::EventParseError;

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

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsSealEventMetadata, EventParseError> {
    let seal = seal_from_parts(kind, &tags, &content)?;
    Ok(RadrootsSealEventMetadata {
        id,
        author,
        published_at,
        kind,
        seal,
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
) -> Result<RadrootsSealEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsSealEventIndex {
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
