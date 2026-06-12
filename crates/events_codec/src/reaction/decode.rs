#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent, kinds::KIND_REACTION, reaction::RadrootsReaction,
    social::RadrootsSocialTarget, tags::TAG_E_ROOT,
};

use crate::error::EventParseError;
use crate::field_helpers::{parse_address_tag, validate_lowercase_hex_64_tag};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};

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
    if tags
        .iter()
        .any(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_E_ROOT))
    {
        return Err(EventParseError::InvalidTag(TAG_E_ROOT));
    }
    let target = parse_reaction_target(tags)?;
    Ok(RadrootsReaction {
        target,
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
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        reaction,
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

fn parse_reaction_target(tags: &[Vec<String>]) -> Result<RadrootsSocialTarget, EventParseError> {
    let event_tag = find_tag(tags, "e");
    let address_tag = find_tag(tags, "a");
    match (event_tag, address_tag) {
        (Some(_), Some(_)) => Err(EventParseError::InvalidTag("e")),
        (None, None) => Err(EventParseError::MissingTag("e")),
        (Some(tag), None) => {
            let id = tag
                .get(1)
                .cloned()
                .ok_or(EventParseError::InvalidTag("e"))?;
            validate_lowercase_hex_64_tag(&id, "e")?;
            let relays = if tag.len() > 2 {
                Some(tag[2..].to_vec())
            } else {
                None
            };
            Ok(RadrootsSocialTarget::Event {
                id,
                author: optional_tag_value(tags, "p")?,
                event_kind: optional_numeric_tag(tags, "k")?,
                relays,
            })
        }
        (None, Some(tag)) => {
            let value = tag
                .get(1)
                .cloned()
                .ok_or(EventParseError::InvalidTag("a"))?;
            let address = parse_address_tag(&value, "a")?;
            let kind = optional_numeric_tag(tags, "k")?.unwrap_or(address.kind);
            if kind != address.kind {
                return Err(EventParseError::InvalidTag("k"));
            }
            let author = optional_tag_value(tags, "p")?.unwrap_or_else(|| address.pubkey.clone());
            if author != address.pubkey {
                return Err(EventParseError::InvalidTag("p"));
            }
            let relays = if tag.len() > 2 {
                Some(tag[2..].to_vec())
            } else {
                None
            };
            Ok(RadrootsSocialTarget::Address {
                address: value,
                author: Some(author),
                event_kind: Some(kind),
                relays,
            })
        }
    }
}

fn find_tag<'a>(tags: &'a [Vec<String>], key: &'static str) -> Option<&'a Vec<String>> {
    tags.iter()
        .find(|tag| tag.first().map(|value| value.as_str()) == Some(key))
}

fn optional_tag_value(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<String>, EventParseError> {
    let Some(tag) = find_tag(tags, key) else {
        return Ok(None);
    };
    let value = tag
        .get(1)
        .cloned()
        .ok_or(EventParseError::InvalidTag(key))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(key));
    }
    Ok(Some(value))
}

fn optional_numeric_tag(
    tags: &[Vec<String>],
    key: &'static str,
) -> Result<Option<u32>, EventParseError> {
    optional_tag_value(tags, key)?
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|err| EventParseError::InvalidNumber(key, err))
        })
        .transpose()
}
