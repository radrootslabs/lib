#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    comment::{RadrootsComment, RadrootsCommentEventIndex, RadrootsCommentEventMetadata},
    tags::{TAG_E_PREV, TAG_E_ROOT},
};

use crate::error::EventParseError;
use crate::event_ref::{find_event_ref_tag, parse_event_ref_tag};

const DEFAULT_KIND: u32 = 1;

pub fn comment_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsComment, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "1",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }

    let root_tag = find_event_ref_tag(tags, TAG_E_ROOT)
        .ok_or(EventParseError::MissingTag(TAG_E_ROOT))?;
    let root = parse_event_ref_tag(root_tag, TAG_E_ROOT)?;

    let parent = match find_event_ref_tag(tags, TAG_E_PREV) {
        Some(tag) => parse_event_ref_tag(tag, TAG_E_PREV)?,
        None => root.clone(),
    };

    Ok(RadrootsComment {
        root,
        parent,
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
) -> Result<RadrootsCommentEventMetadata, EventParseError> {
    let comment = comment_from_tags(kind, &tags, &content)?;
    Ok(RadrootsCommentEventMetadata {
        id,
        author,
        published_at,
        kind,
        comment,
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
) -> Result<RadrootsCommentEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsCommentEventIndex {
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
