#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{RadrootsNostrEvent, kinds::KIND_POST, post::RadrootsPost};

use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const DEFAULT_KIND: u32 = KIND_POST;

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

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    _tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsPost>, EventParseError> {
    let post = post_from_content(kind, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        post,
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
) -> Result<RadrootsParsedEvent<RadrootsPost>, EventParseError> {
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
