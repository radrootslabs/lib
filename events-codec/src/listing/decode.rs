#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    listing::{RadrootsListing, RadrootsListingEventIndex, RadrootsListingEventMetadata},
    tags::TAG_D,
};

use crate::error::EventParseError;

const DEFAULT_KIND: u32 = 30402;

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
    Ok(value)
}

pub fn listing_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsListing, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "30402",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = parse_d_tag(tags)?;
    let mut listing: RadrootsListing =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;

    if listing.d_tag.trim().is_empty() {
        listing.d_tag = d_tag;
    } else if listing.d_tag != d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }

    Ok(listing)
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsListingEventMetadata, EventParseError> {
    let listing = listing_from_event(kind, &tags, &content)?;
    Ok(RadrootsListingEventMetadata {
        id,
        author,
        published_at,
        kind,
        listing,
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
) -> Result<RadrootsListingEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsListingEventIndex {
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
