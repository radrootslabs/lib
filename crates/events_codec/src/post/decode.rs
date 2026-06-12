#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    RadrootsNostrEvent,
    farm::RadrootsFarmRef,
    kinds::{KIND_FARM, KIND_POST},
    post::RadrootsPost,
    social::{RadrootsSocialFarmAnchor, RadrootsSocialMediaMetadata, RadrootsSocialTarget},
    tags::{TAG_A, TAG_IMETA, TAG_Q, TAG_T},
};

use crate::error::EventParseError;
use crate::field_helpers::{parse_address_tag, tag_values, validate_lowercase_hex_64_tag};
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use crate::social_helpers::{location_from_tags, parse_dimensions_tag};

const DEFAULT_KIND: u32 = KIND_POST;

pub fn post_from_content(kind: u32, content: &str) -> Result<RadrootsPost, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "1",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidTag("content"));
    }
    Ok(RadrootsPost {
        content: content.to_string(),
        farm: None,
        address_refs: None,
        location: None,
        topics: None,
        quote_refs: None,
        media: None,
    })
}

pub fn post_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsPost, EventParseError> {
    let mut post = post_from_content(kind, content)?;
    post.farm = farm_anchor_from_tags(tags)?;
    post.address_refs = address_refs_from_tags(tags)?;
    post.location = location_from_tags(tags);
    post.topics = non_empty_vec(tag_values(tags, TAG_T)?);
    post.quote_refs = quote_refs_from_tags(tags)?;
    post.media = media_from_tags(tags)?;
    Ok(post)
}

pub fn data_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsPost>, EventParseError> {
    let post = post_from_event(kind, &tags, &content)?;
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        post,
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
) -> Result<RadrootsParsedEvent<RadrootsPost>, EventParseError> {
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

fn farm_anchor_from_tags(
    tags: &[Vec<String>],
) -> Result<Option<RadrootsSocialFarmAnchor>, EventParseError> {
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_A))
    {
        let value = tag.get(1).ok_or(EventParseError::InvalidTag(TAG_A))?;
        let address = parse_address_tag(value, TAG_A)?;
        if address.kind == KIND_FARM {
            let relays = if tag.len() > 2 {
                Some(tag[2..].to_vec())
            } else {
                None
            };
            return Ok(Some(RadrootsSocialFarmAnchor {
                farm: RadrootsFarmRef {
                    pubkey: address.pubkey,
                    d_tag: address.d_tag,
                },
                relays,
            }));
        }
    }
    Ok(None)
}

fn address_refs_from_tags(
    tags: &[Vec<String>],
) -> Result<Option<Vec<RadrootsSocialTarget>>, EventParseError> {
    let mut refs = Vec::new();
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_A))
    {
        let value = tag.get(1).ok_or(EventParseError::InvalidTag(TAG_A))?;
        let address = parse_address_tag(value, TAG_A)?;
        if address.kind == KIND_FARM {
            continue;
        }
        let relays = if tag.len() > 2 {
            Some(tag[2..].to_vec())
        } else {
            None
        };
        refs.push(RadrootsSocialTarget::Address {
            address: value.clone(),
            author: Some(address.pubkey),
            event_kind: Some(address.kind),
            relays,
        });
    }
    Ok(non_empty_vec(refs))
}

fn quote_refs_from_tags(
    tags: &[Vec<String>],
) -> Result<Option<Vec<RadrootsSocialTarget>>, EventParseError> {
    let mut refs = Vec::new();
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_Q))
    {
        let value = tag.get(1).ok_or(EventParseError::InvalidTag(TAG_Q))?;
        let relays = if tag.len() > 2 {
            Some(tag[2..].to_vec())
        } else {
            None
        };
        match parse_address_tag(value, TAG_Q) {
            Ok(address) => refs.push(RadrootsSocialTarget::Address {
                address: value.clone(),
                author: Some(address.pubkey),
                event_kind: Some(address.kind),
                relays,
            }),
            Err(_) => {
                validate_lowercase_hex_64_tag(value, TAG_Q)?;
                refs.push(RadrootsSocialTarget::Event {
                    id: value.clone(),
                    author: None,
                    event_kind: None,
                    relays,
                });
            }
        }
    }
    Ok(non_empty_vec(refs))
}

fn media_from_tags(
    tags: &[Vec<String>],
) -> Result<Option<Vec<RadrootsSocialMediaMetadata>>, EventParseError> {
    let mut media = Vec::new();
    for tag in tags
        .iter()
        .filter(|tag| tag.first().map(|value| value.as_str()) == Some(TAG_IMETA))
    {
        if tag.len() < 2 {
            return Err(EventParseError::InvalidTag(TAG_IMETA));
        }
        let raw = tag[1..].to_vec();
        if raw.iter().any(|value| value.trim().is_empty()) {
            return Err(EventParseError::InvalidTag(TAG_IMETA));
        }
        let mut item = RadrootsSocialMediaMetadata {
            imeta: Some(vec![raw.clone()]),
            ..RadrootsSocialMediaMetadata::default()
        };
        for entry in raw {
            parse_imeta_entry(&mut item, &entry)?;
        }
        media.push(item);
    }
    Ok(non_empty_vec(media))
}

fn parse_imeta_entry(
    item: &mut RadrootsSocialMediaMetadata,
    entry: &str,
) -> Result<(), EventParseError> {
    let Some((key, value)) = entry.split_once(' ') else {
        return Err(EventParseError::InvalidTag(TAG_IMETA));
    };
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_IMETA));
    }
    match key {
        "url" => item.url = Some(value.to_string()),
        "m" => item.mime_type = Some(value.to_string()),
        "x" => item.sha256 = Some(value.to_string()),
        "ox" => item.original_sha256 = Some(value.to_string()),
        "size" => {
            item.size = Some(
                value
                    .parse::<u64>()
                    .map_err(|err| EventParseError::InvalidNumber(TAG_IMETA, err))?,
            );
        }
        "dim" => item.dimensions = Some(parse_dimensions_tag(value, TAG_IMETA)?),
        "blurhash" => item.blurhash = Some(value.to_string()),
        "image" => item.image = Some(value.to_string()),
        "summary" => item.summary = Some(value.to_string()),
        "alt" => item.alt = Some(value.to_string()),
        "fallback" => item.fallback = Some(value.to_string()),
        "magnet" => item.magnet = Some(value.to_string()),
        "i" => push_repeated_value(&mut item.content_hashes, value),
        "service" => push_repeated_value(&mut item.services, value),
        "thumb" => {}
        _ => {}
    }
    Ok(())
}

fn push_repeated_value(values: &mut Option<Vec<String>>, value: &str) {
    values.get_or_insert_with(Vec::new).push(value.to_string());
}

fn non_empty_vec<T>(values: Vec<T>) -> Option<Vec<T>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}
