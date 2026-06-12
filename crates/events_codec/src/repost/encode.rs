#[cfg(not(feature = "std"))]
use alloc::{format, string::ToString, vec, vec::Vec};

use radroots_events::{
    kinds::{KIND_GENERIC_REPOST, KIND_POST, KIND_REPOST},
    repost::{RadrootsGenericRepost, RadrootsRepost},
    social::RadrootsSocialTarget,
    tags::{TAG_A, TAG_E, TAG_K, TAG_P},
};

use crate::error::EventEncodeError;
use crate::field_helpers::{
    parse_address_tag, push_tag, validate_lowercase_hex_64, validate_non_empty_field,
};
use crate::wire::WireEventParts;

pub fn repost_build_tags(repost: &RadrootsRepost) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::new();
    push_event_target(&mut tags, &repost.target, KIND_POST, "target")?;
    Ok(tags)
}

pub fn generic_repost_build_tags(
    repost: &RadrootsGenericRepost,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_generic_target_kind(repost.target_kind)?;
    let mut tags = Vec::new();
    push_generic_target(&mut tags, &repost.target, repost.target_kind)?;
    push_tag(&mut tags, TAG_K, repost.target_kind.to_string());
    Ok(tags)
}

pub fn repost_to_wire_parts(repost: &RadrootsRepost) -> Result<WireEventParts, EventEncodeError> {
    repost_to_wire_parts_with_kind(repost, KIND_REPOST)
}

pub fn generic_repost_to_wire_parts(
    repost: &RadrootsGenericRepost,
) -> Result<WireEventParts, EventEncodeError> {
    generic_repost_to_wire_parts_with_kind(repost, KIND_GENERIC_REPOST)
}

pub fn repost_to_wire_parts_with_kind(
    repost: &RadrootsRepost,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_REPOST {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: repost.content.clone().unwrap_or_default(),
        tags: repost_build_tags(repost)?,
    })
}

pub fn generic_repost_to_wire_parts_with_kind(
    repost: &RadrootsGenericRepost,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_GENERIC_REPOST {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    Ok(WireEventParts {
        kind,
        content: repost.content.clone().unwrap_or_default(),
        tags: generic_repost_build_tags(repost)?,
    })
}

fn push_event_target(
    tags: &mut Vec<Vec<String>>,
    target: &RadrootsSocialTarget,
    expected_kind: u32,
    field: &'static str,
) -> Result<(), EventEncodeError> {
    let RadrootsSocialTarget::Event {
        id,
        author,
        event_kind,
        relays,
    } = target
    else {
        return Err(EventEncodeError::InvalidField(field));
    };
    validate_lowercase_hex_64(id, "target.id")?;
    validate_event_kind(*event_kind, expected_kind)?;
    let mut event_tag = vec![TAG_E.to_string(), id.clone()];
    if let Some(relays) = relays.as_ref() {
        event_tag.extend(
            relays
                .iter()
                .filter(|relay| !relay.trim().is_empty())
                .cloned(),
        );
    }
    tags.push(event_tag);
    if let Some(author) = author.as_deref() {
        validate_non_empty_field(author, "target.author")?;
        push_tag(tags, TAG_P, author);
    }
    Ok(())
}

fn push_generic_target(
    tags: &mut Vec<Vec<String>>,
    target: &RadrootsSocialTarget,
    expected_kind: u32,
) -> Result<(), EventEncodeError> {
    match target {
        RadrootsSocialTarget::Event {
            id,
            author,
            event_kind,
            relays,
        } => {
            validate_lowercase_hex_64(id, "target.id")?;
            validate_event_kind(*event_kind, expected_kind)?;
            let mut event_tag = vec![TAG_E.to_string(), id.clone()];
            if let Some(relays) = relays.as_ref() {
                event_tag.extend(
                    relays
                        .iter()
                        .filter(|relay| !relay.trim().is_empty())
                        .cloned(),
                );
            }
            tags.push(event_tag);
            if let Some(author) = author.as_deref() {
                validate_non_empty_field(author, "target.author")?;
                push_tag(tags, TAG_P, author);
            }
            Ok(())
        }
        RadrootsSocialTarget::Address {
            address,
            author,
            event_kind,
            relays,
        } => {
            let address = parse_address_tag(address, "target.address")
                .map_err(|_| EventEncodeError::InvalidField("target.address"))?;
            if address.kind != expected_kind {
                return Err(EventEncodeError::InvalidField("target_kind"));
            }
            validate_event_kind(*event_kind, expected_kind)?;
            let mut tag = vec![
                TAG_A.to_string(),
                format!("{}:{}:{}", address.kind, address.pubkey, address.d_tag),
            ];
            if let Some(relays) = relays.as_ref() {
                tag.extend(
                    relays
                        .iter()
                        .filter(|relay| !relay.trim().is_empty())
                        .cloned(),
                );
            }
            tags.push(tag);
            if let Some(author) = author.as_deref() {
                validate_non_empty_field(author, "target.author")?;
            }
            Ok(())
        }
        RadrootsSocialTarget::External { .. } => Err(EventEncodeError::InvalidField("target")),
    }
}

fn validate_event_kind(
    event_kind: Option<u32>,
    expected_kind: u32,
) -> Result<(), EventEncodeError> {
    if event_kind == Some(expected_kind) {
        Ok(())
    } else {
        Err(EventEncodeError::InvalidField("target_kind"))
    }
}

fn validate_generic_target_kind(target_kind: u32) -> Result<(), EventEncodeError> {
    if target_kind == KIND_POST {
        Err(EventEncodeError::InvalidField("target_kind"))
    } else {
        Ok(())
    }
}
