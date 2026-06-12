#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    comment::RadrootsComment,
    kinds::{KIND_COMMENT, KIND_POST},
    social::RadrootsSocialTarget,
    tags::{TAG_E_PREV, TAG_E_ROOT},
};

use crate::error::EventParseError;
use crate::field_helpers::{parse_address_tag, validate_lowercase_hex_64_tag};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

const DEFAULT_KIND: u32 = KIND_COMMENT;

pub fn comment_from_tags(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsComment, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "1111",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }
    if tags
        .iter()
        .any(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_E_ROOT))
    {
        return Err(EventParseError::InvalidTag(TAG_E_ROOT));
    }
    if tags
        .iter()
        .any(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_E_PREV))
    {
        return Err(EventParseError::InvalidTag(TAG_E_PREV));
    }

    let root = parse_comment_target(tags, CommentTargetTags::root())?;
    let parent = parse_comment_target(tags, CommentTargetTags::parent())?;

    Ok(RadrootsComment {
        root,
        parent,
        content: content.to_string(),
    })
}

struct CommentTargetTags {
    event: &'static str,
    address: &'static str,
    external: &'static str,
    author: &'static str,
    kind: &'static str,
}

impl CommentTargetTags {
    fn root() -> Self {
        Self {
            event: "E",
            address: "A",
            external: "I",
            author: "P",
            kind: "K",
        }
    }

    fn parent() -> Self {
        Self {
            event: "e",
            address: "a",
            external: "i",
            author: "p",
            kind: "k",
        }
    }
}

fn parse_comment_target(
    tags: &[Vec<String>],
    keys: CommentTargetTags,
) -> Result<RadrootsSocialTarget, EventParseError> {
    let event_tag = find_tag(tags, keys.event);
    let address_tag = find_tag(tags, keys.address);
    let external_tag = find_tag(tags, keys.external);
    let target_count = usize::from(event_tag.is_some())
        + usize::from(address_tag.is_some())
        + usize::from(external_tag.is_some());
    if target_count == 0 {
        return Err(EventParseError::MissingTag(keys.event));
    }
    if target_count > 1 {
        return Err(EventParseError::InvalidTag(keys.event));
    }

    if let Some(tag) = event_tag {
        let id = tag
            .get(1)
            .cloned()
            .ok_or(EventParseError::InvalidTag(keys.event))?;
        validate_lowercase_hex_64_tag(&id, keys.event)?;
        let kind = required_numeric_kind(tags, keys.kind)?;
        validate_comment_target_kind(kind, keys.kind)?;
        let author = required_author(tags, keys.author)?;
        let relays = if tag.len() > 2 {
            Some(tag[2..].to_vec())
        } else {
            None
        };
        return Ok(RadrootsSocialTarget::Event {
            id,
            author: Some(author),
            event_kind: Some(kind),
            relays,
        });
    }

    if let Some(tag) = address_tag {
        let value = tag
            .get(1)
            .cloned()
            .ok_or(EventParseError::InvalidTag(keys.address))?;
        let address = parse_address_tag(&value, keys.address)?;
        let kind = required_numeric_kind(tags, keys.kind)?;
        validate_comment_target_kind(kind, keys.kind)?;
        if kind != address.kind {
            return Err(EventParseError::InvalidTag(keys.kind));
        }
        let author = required_author(tags, keys.author)?;
        if author != address.pubkey {
            return Err(EventParseError::InvalidTag(keys.author));
        }
        let relays = if tag.len() > 2 {
            Some(tag[2..].to_vec())
        } else {
            None
        };
        return Ok(RadrootsSocialTarget::Address {
            address: value,
            author: Some(author),
            event_kind: Some(kind),
            relays,
        });
    }

    let Some(tag) = external_tag else {
        return Err(EventParseError::MissingTag(keys.external));
    };
    let id = tag
        .get(1)
        .cloned()
        .ok_or(EventParseError::InvalidTag(keys.external))?;
    if id.trim().is_empty() {
        return Err(EventParseError::InvalidTag(keys.external));
    }
    let external_kind = required_kind_value(tags, keys.kind)?;
    if external_kind == "1" {
        return Err(EventParseError::InvalidTag(keys.kind));
    }
    let hint = tag.get(2).filter(|value| !value.trim().is_empty()).cloned();
    Ok(RadrootsSocialTarget::External {
        id,
        external_kind,
        hint,
    })
}

fn validate_comment_target_kind(kind: u32, key: &'static str) -> Result<(), EventParseError> {
    if kind == KIND_POST {
        Err(EventParseError::InvalidTag(key))
    } else {
        Ok(())
    }
}

fn find_tag<'a>(tags: &'a [Vec<String>], key: &'static str) -> Option<&'a Vec<String>> {
    tags.iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
}

fn required_author(tags: &[Vec<String>], key: &'static str) -> Result<String, EventParseError> {
    let value = find_tag(tags, key)
        .and_then(|tag| tag.get(1))
        .cloned()
        .ok_or(EventParseError::MissingTag(key))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(key));
    }
    Ok(value)
}

fn required_kind_value(tags: &[Vec<String>], key: &'static str) -> Result<String, EventParseError> {
    let value = find_tag(tags, key)
        .and_then(|tag| tag.get(1))
        .cloned()
        .ok_or(EventParseError::MissingTag(key))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(key));
    }
    Ok(value)
}

fn required_numeric_kind(tags: &[Vec<String>], key: &'static str) -> Result<u32, EventParseError> {
    required_kind_value(tags, key)?
        .parse::<u32>()
        .map_err(|err| EventParseError::InvalidNumber(key, err))
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsComment>, EventParseError> {
    let comment = comment_from_tags(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        comment,
    ))
}

pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsComment>, EventParseError> {
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
