#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::KIND_MESSAGE,
    message::{RadrootsMessage, RadrootsMessageEventIndex, RadrootsMessageEventMetadata},
};

use crate::error::EventParseError;
use crate::message::tags::{parse_recipients, parse_reply_tag, parse_subject_tag};

const DEFAULT_KIND: u32 = KIND_MESSAGE;

pub fn message_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsMessage, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "14",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }

    let recipients = parse_recipients(tags)?;

    let reply_to = parse_reply_tag(tags)?;

    let subject = parse_subject_tag(tags)?;

    Ok(RadrootsMessage {
        recipients,
        content: content.to_string(),
        reply_to,
        subject,
    })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsMessageEventMetadata, EventParseError> {
    let message = message_from_tags(kind, &tags, &content)?;
    Ok(RadrootsMessageEventMetadata {
        id,
        author,
        published_at,
        kind,
        message,
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
) -> Result<RadrootsMessageEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsMessageEventIndex {
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
