#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::KIND_MESSAGE_FILE,
    message_file::{
        RadrootsMessageFile, RadrootsMessageFileDimensions, RadrootsMessageFileEventIndex,
        RadrootsMessageFileEventMetadata,
    },
};

use crate::error::EventParseError;
use crate::message::tags::{parse_recipients, parse_reply_tag, parse_subject_tag};

const DEFAULT_KIND: u32 = KIND_MESSAGE_FILE;

fn required_tag_value(tags: &[Vec<String>], key: &'static str) -> Result<String, EventParseError> {
    let value = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(key))
        .and_then(|t| t.get(1));
    let value = value.ok_or(EventParseError::MissingTag(key))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(key));
    }
    Ok(value.clone())
}

fn optional_tag_value(tags: &[Vec<String>], key: &'static str) -> Result<Option<String>, EventParseError> {
    let value = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(key))
        .and_then(|t| t.get(1));
    match value {
        Some(value) if value.trim().is_empty() => Err(EventParseError::InvalidTag(key)),
        Some(value) => Ok(Some(value.clone())),
        None => Ok(None),
    }
}

fn parse_dimensions(value: &str) -> Result<RadrootsMessageFileDimensions, EventParseError> {
    let (w, h) = value.split_once('x').ok_or(EventParseError::InvalidTag("dim"))?;
    let w = w.parse::<u32>().map_err(|_| EventParseError::InvalidTag("dim"))?;
    let h = h.parse::<u32>().map_err(|_| EventParseError::InvalidTag("dim"))?;
    Ok(RadrootsMessageFileDimensions { w, h })
}

fn parse_size(tags: &[Vec<String>]) -> Result<Option<u64>, EventParseError> {
    let value = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("size"))
        .and_then(|t| t.get(1));
    let Some(value) = value else { return Ok(None); };
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag("size"));
    }
    let size = value
        .parse::<u64>()
        .map_err(|e| EventParseError::InvalidNumber("size", e))?;
    Ok(Some(size))
}

fn parse_dimensions_tag(tags: &[Vec<String>]) -> Result<Option<RadrootsMessageFileDimensions>, EventParseError> {
    let value = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("dim"))
        .and_then(|t| t.get(1));
    let Some(value) = value else { return Ok(None); };
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag("dim"));
    }
    Ok(Some(parse_dimensions(value)?))
}

fn parse_fallbacks(tags: &[Vec<String>]) -> Result<Vec<String>, EventParseError> {
    let mut fallbacks = Vec::new();
    for tag in tags.iter().filter(|t| t.get(0).map(|s| s.as_str()) == Some("fallback")) {
        let value = tag.get(1).ok_or(EventParseError::InvalidTag("fallback"))?;
        if value.trim().is_empty() {
            return Err(EventParseError::InvalidTag("fallback"));
        }
        fallbacks.push(value.clone());
    }
    Ok(fallbacks)
}

pub fn message_file_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsMessageFile, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "15",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }

    let recipients = parse_recipients(tags)?;
    let reply_to = parse_reply_tag(tags)?;
    let subject = parse_subject_tag(tags)?;
    let file_type = required_tag_value(tags, "file-type")?;
    let encryption_algorithm = required_tag_value(tags, "encryption-algorithm")?;
    let decryption_key = required_tag_value(tags, "decryption-key")?;
    let decryption_nonce = required_tag_value(tags, "decryption-nonce")?;
    let encrypted_hash = required_tag_value(tags, "x")?;
    let original_hash = optional_tag_value(tags, "ox")?;
    let size = parse_size(tags)?;
    let dimensions = parse_dimensions_tag(tags)?;
    let blurhash = optional_tag_value(tags, "blurhash")?;
    let thumb = optional_tag_value(tags, "thumb")?;
    let fallbacks = parse_fallbacks(tags)?;

    Ok(RadrootsMessageFile {
        recipients,
        file_url: content.to_string(),
        reply_to,
        subject,
        file_type,
        encryption_algorithm,
        decryption_key,
        decryption_nonce,
        encrypted_hash,
        original_hash,
        size,
        dimensions,
        blurhash,
        thumb,
        fallbacks,
    })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsMessageFileEventMetadata, EventParseError> {
    let message_file = message_file_from_tags(kind, &tags, &content)?;
    Ok(RadrootsMessageFileEventMetadata {
        id,
        author,
        published_at,
        kind,
        message_file,
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
) -> Result<RadrootsMessageFileEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsMessageFileEventIndex {
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
