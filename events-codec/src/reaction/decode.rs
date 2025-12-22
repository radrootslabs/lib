#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    reaction::{RadrootsReaction, RadrootsReactionEventIndex, RadrootsReactionEventMetadata},
    tags::TAG_E_ROOT,
};

use crate::error::EventParseError;
use crate::event_ref::{find_event_ref_tag, parse_event_ref_tag};

const DEFAULT_KIND: u32 = 7;

pub fn reaction_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsReaction, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "7",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }
    let root_tag = find_event_ref_tag(tags, TAG_E_ROOT)
        .ok_or(EventParseError::MissingTag(TAG_E_ROOT))?;
    let root = parse_event_ref_tag(root_tag, TAG_E_ROOT)?;
    Ok(RadrootsReaction {
        root,
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
) -> Result<RadrootsReactionEventMetadata, EventParseError> {
    let reaction = reaction_from_tags(kind, &tags, &content)?;
    Ok(RadrootsReactionEventMetadata {
        id,
        author,
        published_at,
        kind,
        reaction,
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
) -> Result<RadrootsReactionEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsReactionEventIndex {
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
