#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::KIND_REACTION,
    reaction::{RadrootsReaction},
    tags::TAG_E_ROOT,
};

use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use crate::event_ref::{find_event_ref_tag, parse_event_ref_tag, parse_nip10_ref_tags};

const DEFAULT_KIND: u32 = KIND_REACTION;

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
    let root = if find_event_ref_tag(tags, "e").is_some() {
        parse_nip10_ref_tags(tags, "e", "p", "k", "a")?
    } else if let Some(root_tag) = find_event_ref_tag(tags, TAG_E_ROOT) {
        parse_event_ref_tag(root_tag, TAG_E_ROOT)?
    } else {
        return Err(EventParseError::MissingTag("e"));
    };
    Ok(RadrootsReaction {
        root,
        content: content.to_string(),
    })
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsReaction>, EventParseError> {
    let reaction = reaction_from_tags(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(id, author, published_at, kind, reaction))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsReaction>, EventParseError> {
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
