#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    gift_wrap::{RadrootsGiftWrap, RadrootsGiftWrapEventIndex, RadrootsGiftWrapEventMetadata, RadrootsGiftWrapRecipient},
    kinds::KIND_GIFT_WRAP,
};

use crate::error::EventParseError;

const DEFAULT_KIND: u32 = KIND_GIFT_WRAP;

fn parse_recipient(tags: &[Vec<String>]) -> Result<RadrootsGiftWrapRecipient, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("p"))
        .ok_or(EventParseError::MissingTag("p"))?;
    let public_key = tag.get(1).ok_or(EventParseError::InvalidTag("p"))?;
    if public_key.trim().is_empty() {
        return Err(EventParseError::InvalidTag("p"));
    }
    let relay_url = match tag.get(2) {
        Some(value) if value.trim().is_empty() => return Err(EventParseError::InvalidTag("p")),
        Some(value) => Some(value.clone()),
        None => None,
    };
    Ok(RadrootsGiftWrapRecipient {
        public_key: public_key.clone(),
        relay_url,
    })
}

fn parse_expiration(tags: &[Vec<String>]) -> Result<Option<u32>, EventParseError> {
    let value = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some("expiration"))
        .and_then(|t| t.get(1));
    let Some(value) = value else { return Ok(None); };
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag("expiration"));
    }
    let expiration = value
        .parse::<u32>()
        .map_err(|e| EventParseError::InvalidNumber("expiration", e))?;
    Ok(Some(expiration))
}

pub fn gift_wrap_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsGiftWrap, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "1059",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }
    let recipient = parse_recipient(tags)?;
    let expiration = parse_expiration(tags)?;
    Ok(RadrootsGiftWrap {
        recipient,
        content: content.to_string(),
        expiration,
    })
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsGiftWrapEventMetadata, EventParseError> {
    let gift_wrap = gift_wrap_from_tags(kind, &tags, &content)?;
    Ok(RadrootsGiftWrapEventMetadata {
        id,
        author,
        published_at,
        kind,
        gift_wrap,
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
) -> Result<RadrootsGiftWrapEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsGiftWrapEventIndex {
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
