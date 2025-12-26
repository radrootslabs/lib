#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    message::{
        RadrootsMessage, RadrootsMessageEventIndex, RadrootsMessageEventMetadata,
        RadrootsMessageRecipient,
    },
};

use crate::error::EventParseError;

const DEFAULT_KIND: u32 = 14;

fn parse_recipient_tag(tag: &[String]) -> Result<RadrootsMessageRecipient, EventParseError> {
    if tag.get(0).map(|s| s.as_str()) != Some("p") {
        return Err(EventParseError::InvalidTag("p"));
    }
    let public_key = tag.get(1).ok_or(EventParseError::InvalidTag("p"))?;
    if public_key.trim().is_empty() {
        return Err(EventParseError::InvalidTag("p"));
    }
    let relay_url = match tag.get(2) {
        Some(value) if value.trim().is_empty() => return Err(EventParseError::InvalidTag("p")),
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(RadrootsMessageRecipient {
        public_key: public_key.clone(),
        relay_url,
    })
}

fn parse_reply_tag(tag: &[String]) -> Result<RadrootsNostrEventPtr, EventParseError> {
    if tag.get(0).map(|s| s.as_str()) != Some("e") {
        return Err(EventParseError::InvalidTag("e"));
    }
    let id = tag.get(1).ok_or(EventParseError::InvalidTag("e"))?;
    if id.trim().is_empty() {
        return Err(EventParseError::InvalidTag("e"));
    }
    let relay = match tag.get(2) {
        Some(value) if value.trim().is_empty() => return Err(EventParseError::InvalidTag("e")),
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(RadrootsNostrEventPtr {
        id: id.clone(),
        relays: relay,
    })
}

fn parse_subject_tag(tag: &[String]) -> Result<String, EventParseError> {
    if tag.get(0).map(|s| s.as_str()) != Some("subject") {
        return Err(EventParseError::InvalidTag("subject"));
    }
    let subject = tag.get(1).ok_or(EventParseError::InvalidTag("subject"))?;
    if subject.trim().is_empty() {
        return Err(EventParseError::InvalidTag("subject"));
    }
    Ok(subject.clone())
}

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

    let mut recipients = Vec::new();
    for tag in tags.iter().filter(|t| t.get(0).map(|s| s.as_str()) == Some("p")) {
        recipients.push(parse_recipient_tag(tag)?);
    }
    if recipients.is_empty() {
        return Err(EventParseError::MissingTag("p"));
    }

    let reply_to = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("e"))
        .map(|tag| parse_reply_tag(tag))
        .transpose()?;

    let subject = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("subject"))
        .map(|tag| parse_subject_tag(tag))
        .transpose()?;

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
