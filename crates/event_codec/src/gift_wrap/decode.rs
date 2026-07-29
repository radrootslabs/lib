#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_event::{
    envelope::kind::KIND_GIFT_WRAP,
    social::gift_wrap::{GiftWrap, GiftWrapRecipient},
};

use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const DEFAULT_KIND: u32 = KIND_GIFT_WRAP;

fn parse_recipient(tags: &[Vec<String>]) -> Result<GiftWrapRecipient, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("p"))
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
    Ok(GiftWrapRecipient {
        public_key: public_key.clone(),
        relay_url,
    })
}

fn parse_expiration(tags: &[Vec<String>]) -> Result<Option<u32>, EventParseError> {
    let value = tags
        .iter()
        .find(|t| t.first().map(|s| s.as_str()) == Some("expiration"))
        .and_then(|t| t.get(1));
    let Some(value) = value else {
        return Ok(None);
    };
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
) -> Result<GiftWrap, EventParseError> {
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
    Ok(GiftWrap {
        recipient,
        content: content.to_string(),
        expiration,
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<GiftWrap>, EventParseError> {
    let gift_wrap = gift_wrap_from_tags(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        gift_wrap,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<GiftWrap>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    RadrootsParsedEvent::from_event_parts(id, author, published_at, kind, content, tags, sig, data)
}
