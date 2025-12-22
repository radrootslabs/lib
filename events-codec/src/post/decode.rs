#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    post::{RadrootsPost, RadrootsPostEventIndex, RadrootsPostEventMetadata},
};

use crate::error::EventParseError;

const DEFAULT_KIND: u32 = 1;

pub fn post_from_content(kind: u32, content: &str) -> Result<RadrootsPost, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "1",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }
    Ok(RadrootsPost {
        content: content.to_string(),
    })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    _tags: Vec<Vec<String>>,
) -> Result<RadrootsPostEventMetadata, EventParseError> {
    let post = post_from_content(kind, &content)?;
    Ok(RadrootsPostEventMetadata {
        id,
        author,
        published_at,
        kind,
        post,
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
) -> Result<RadrootsPostEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsPostEventIndex {
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
