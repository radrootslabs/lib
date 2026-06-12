#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    comment::RadrootsComment,
    kinds::{KIND_COMMENT, KIND_POST},
    social::RadrootsSocialTarget,
};

use crate::error::EventEncodeError;
use crate::field_helpers::{
    parse_address_tag, validate_lowercase_hex_64, validate_non_empty_field,
};
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_COMMENT;

pub fn comment_build_tags(comment: &RadrootsComment) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::with_capacity(8);
    push_comment_target(&mut tags, &comment.root, CommentTargetTags::root())?;
    push_comment_target(&mut tags, &comment.parent, CommentTargetTags::parent())?;
    Ok(tags)
}

pub fn to_wire_parts(comment: &RadrootsComment) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(comment, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    comment: &RadrootsComment,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if comment.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    let tags = comment_build_tags(comment)?;
    Ok(WireEventParts {
        kind,
        content: comment.content.clone(),
        tags,
    })
}

struct CommentTargetTags {
    event: &'static str,
    address: &'static str,
    external: &'static str,
    author: &'static str,
    kind: &'static str,
    field: &'static str,
}

impl CommentTargetTags {
    fn root() -> Self {
        Self {
            event: "E",
            address: "A",
            external: "I",
            author: "P",
            kind: "K",
            field: "root",
        }
    }

    fn parent() -> Self {
        Self {
            event: "e",
            address: "a",
            external: "i",
            author: "p",
            kind: "k",
            field: "parent",
        }
    }
}

fn push_comment_target(
    tags: &mut Vec<Vec<String>>,
    target: &RadrootsSocialTarget,
    keys: CommentTargetTags,
) -> Result<(), EventEncodeError> {
    match target {
        RadrootsSocialTarget::Event {
            id,
            author,
            event_kind,
            relays,
        } => {
            validate_lowercase_hex_64(id, keys.field)?;
            let author = author
                .as_deref()
                .ok_or(EventEncodeError::EmptyRequiredField(keys.field))?;
            validate_non_empty_field(author, keys.field)?;
            let kind = event_kind.ok_or(EventEncodeError::EmptyRequiredField(keys.field))?;
            validate_comment_target_kind(kind, keys.field)?;
            let mut event_tag = Vec::with_capacity(2 + relays.as_ref().map_or(0, Vec::len));
            event_tag.push(keys.event.to_string());
            event_tag.push(id.clone());
            if let Some(relays) = relays {
                event_tag.extend(relays.iter().cloned());
            }
            tags.push(event_tag);
            tags.push(vec![keys.author.to_string(), author.to_string()]);
            tags.push(vec![keys.kind.to_string(), kind.to_string()]);
        }
        RadrootsSocialTarget::Address {
            address,
            author,
            event_kind,
            relays,
        } => {
            let parsed = parse_address_tag(address, keys.field)
                .map_err(|_| EventEncodeError::InvalidField(keys.field))?;
            validate_comment_target_kind(parsed.kind, keys.field)?;
            if let Some(kind) = event_kind {
                if *kind != parsed.kind {
                    return Err(EventEncodeError::InvalidField(keys.field));
                }
            }
            if let Some(author) = author.as_deref() {
                if author != parsed.pubkey {
                    return Err(EventEncodeError::InvalidField(keys.field));
                }
            }
            let mut address_tag = Vec::with_capacity(2 + relays.as_ref().map_or(0, Vec::len));
            address_tag.push(keys.address.to_string());
            address_tag.push(format!(
                "{}:{}:{}",
                parsed.kind, parsed.pubkey, parsed.d_tag
            ));
            if let Some(relays) = relays {
                address_tag.extend(relays.iter().cloned());
            }
            tags.push(address_tag);
            tags.push(vec![keys.author.to_string(), parsed.pubkey]);
            tags.push(vec![keys.kind.to_string(), parsed.kind.to_string()]);
        }
        RadrootsSocialTarget::External {
            id,
            external_kind,
            hint,
        } => {
            validate_non_empty_field(id, keys.field)?;
            validate_non_empty_field(external_kind, keys.field)?;
            let mut external_tag = Vec::with_capacity(3);
            external_tag.push(keys.external.to_string());
            external_tag.push(id.clone());
            if let Some(hint) = hint.as_deref().filter(|value| !value.trim().is_empty()) {
                external_tag.push(hint.to_string());
            }
            tags.push(external_tag);
            tags.push(vec![keys.kind.to_string(), external_kind.clone()]);
        }
    }
    Ok(())
}

fn validate_comment_target_kind(kind: u32, field: &'static str) -> Result<(), EventEncodeError> {
    if kind == KIND_POST {
        Err(EventEncodeError::InvalidField(field))
    } else {
        Ok(())
    }
}
