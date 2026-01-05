#![cfg(feature = "serde_json")]
#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    document::{RadrootsDocument, RadrootsDocumentEventIndex, RadrootsDocumentEventMetadata},
    kinds::KIND_DOCUMENT,
    tags::TAG_D,
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;

const TAG_A: &str = "a";
const TAG_P: &str = "p";
const DEFAULT_KIND: u32 = KIND_DOCUMENT;

fn parse_d_tag(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_D))
        .ok_or(EventParseError::MissingTag(TAG_D))?;
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_D))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    validate_d_tag_tag(&value, TAG_D)?;
    Ok(value)
}

fn parse_subject_pubkey(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_P))
        .ok_or(EventParseError::MissingTag(TAG_P))?;
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_P))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_P));
    }
    Ok(value)
}

fn parse_subject_address(tags: &[Vec<String>]) -> Result<Option<String>, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_A));
    let Some(tag) = tag else { return Ok(None) };
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_A))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    Ok(Some(value))
}

pub fn document_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsDocument, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "30361",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = parse_d_tag(tags)?;
    let subject_pubkey = parse_subject_pubkey(tags)?;
    let subject_address = parse_subject_address(tags)?;
    let mut document: RadrootsDocument =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;

    if document.d_tag.trim().is_empty() {
        document.d_tag = d_tag;
    } else if document.d_tag != d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }

    if document.subject.pubkey.trim().is_empty() {
        document.subject.pubkey = subject_pubkey;
    } else if document.subject.pubkey != subject_pubkey {
        return Err(EventParseError::InvalidTag(TAG_P));
    }

    if let Some(address) = document.subject.address.as_ref() {
        if address.trim().is_empty() {
            return Err(EventParseError::InvalidTag(TAG_A));
        }
    }

    if let Some(tag_address) = subject_address {
        match document.subject.address.as_ref() {
            None => {
                document.subject.address = Some(tag_address);
            }
            Some(existing) => {
                if existing != &tag_address {
                    return Err(EventParseError::InvalidTag(TAG_A));
                }
            }
        }
    } else if document.subject.address.is_some() {
        return Err(EventParseError::MissingTag(TAG_A));
    }

    Ok(document)
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsDocumentEventMetadata, EventParseError> {
    let document = document_from_event(kind, &tags, &content)?;
    Ok(RadrootsDocumentEventMetadata {
        id,
        author,
        published_at,
        kind,
        document,
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
) -> Result<RadrootsDocumentEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsDocumentEventIndex {
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
