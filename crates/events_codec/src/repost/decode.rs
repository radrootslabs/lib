#[cfg(not(feature = "std"))]
use alloc::{string::ToString, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::{KIND_GENERIC_REPOST, KIND_POST, KIND_REPOST},
    repost::{RadrootsGenericRepost, RadrootsRepost},
    social::RadrootsSocialTarget,
    tags::{TAG_A, TAG_E, TAG_K, TAG_P},
};

use crate::error::EventParseError;
use crate::field_helpers::{parse_address_tag, required_tag_value, validate_lowercase_hex_64_tag};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use crate::social_helpers::first_tag_value;

pub fn repost_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsRepost, EventParseError> {
    if kind != KIND_REPOST {
        return Err(EventParseError::InvalidKind {
            expected: "6",
            got: kind,
        });
    }
    let event_tag = find_tag(tags, TAG_E).ok_or(EventParseError::MissingTag(TAG_E))?;
    let id = event_tag
        .get(1)
        .cloned()
        .ok_or(EventParseError::InvalidTag(TAG_E))?;
    validate_lowercase_hex_64_tag(&id, TAG_E)?;
    Ok(RadrootsRepost {
        target: RadrootsSocialTarget::Event {
            id,
            author: first_tag_value(tags, TAG_P),
            event_kind: Some(KIND_POST),
            relays: relays_from_tag(event_tag, 2),
        },
        content: optional_content(content),
    })
}

pub fn generic_repost_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsGenericRepost, EventParseError> {
    if kind != KIND_GENERIC_REPOST {
        return Err(EventParseError::InvalidKind {
            expected: "16",
            got: kind,
        });
    }
    let target_kind = required_tag_value(tags, TAG_K)?
        .parse::<u32>()
        .map_err(|err| EventParseError::InvalidNumber(TAG_K, err))?;
    if target_kind == KIND_POST {
        return Err(EventParseError::InvalidTag(TAG_K));
    }
    let target = if let Some(tag) = find_tag(tags, TAG_A) {
        let value = tag
            .get(1)
            .cloned()
            .ok_or(EventParseError::InvalidTag(TAG_A))?;
        let address = parse_address_tag(&value, TAG_A)?;
        if address.kind != target_kind {
            return Err(EventParseError::InvalidTag(TAG_A));
        }
        RadrootsSocialTarget::Address {
            address: value,
            author: Some(address.pubkey),
            event_kind: Some(target_kind),
            relays: relays_from_tag(tag, 2),
        }
    } else if let Some(tag) = find_tag(tags, TAG_E) {
        let id = tag
            .get(1)
            .cloned()
            .ok_or(EventParseError::InvalidTag(TAG_E))?;
        validate_lowercase_hex_64_tag(&id, TAG_E)?;
        RadrootsSocialTarget::Event {
            id,
            author: first_tag_value(tags, TAG_P),
            event_kind: Some(target_kind),
            relays: relays_from_tag(tag, 2),
        }
    } else {
        return Err(EventParseError::MissingTag(TAG_E));
    };
    Ok(RadrootsGenericRepost {
        target,
        target_kind,
        content: optional_content(content),
    })
}

pub fn repost_data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsRepost>, EventParseError> {
    let repost = repost_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        repost,
    ))
}

pub fn generic_repost_data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsGenericRepost>, EventParseError> {
    let repost = generic_repost_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        repost,
    ))
}

pub fn repost_parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsRepost>, EventParseError> {
    let data = repost_data_from_event(
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

pub fn generic_repost_parsed_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsGenericRepost>, EventParseError> {
    let data = generic_repost_data_from_event(
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

fn find_tag<'a>(tags: &'a [Vec<String>], key: &'static str) -> Option<&'a Vec<String>> {
    tags.iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
}

fn relays_from_tag(tag: &[String], start: usize) -> Option<Vec<String>> {
    let relays = tag
        .iter()
        .skip(start)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    if relays.is_empty() {
        None
    } else {
        Some(relays)
    }
}

fn optional_content(content: &str) -> Option<String> {
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}
