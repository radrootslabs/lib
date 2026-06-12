#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use radroots_events::{
    kinds::KIND_REACTION, reaction::RadrootsReaction, social::RadrootsSocialTarget,
};

use crate::error::EventEncodeError;
use crate::field_helpers::{
    parse_address_tag, validate_lowercase_hex_64, validate_non_empty_field,
};
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_REACTION;

pub fn reaction_build_tags(
    reaction: &RadrootsReaction,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::with_capacity(4);
    push_reaction_target(&mut tags, &reaction.target)?;
    Ok(tags)
}

pub fn to_wire_parts(reaction: &RadrootsReaction) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(reaction, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    reaction: &RadrootsReaction,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    let tags = reaction_build_tags(reaction)?;
    Ok(WireEventParts {
        kind,
        content: reaction.content.clone(),
        tags,
    })
}

fn push_reaction_target(
    tags: &mut Vec<Vec<String>>,
    target: &RadrootsSocialTarget,
) -> Result<(), EventEncodeError> {
    match target {
        RadrootsSocialTarget::Event {
            id,
            author,
            event_kind,
            relays,
        } => {
            validate_lowercase_hex_64(id, "target.id")?;
            let mut event_tag = Vec::with_capacity(2 + relays.as_ref().map_or(0, Vec::len));
            event_tag.push("e".to_string());
            event_tag.push(id.clone());
            if let Some(relays) = relays {
                event_tag.extend(relays.iter().cloned());
            }
            tags.push(event_tag);
            if let Some(author) = author.as_deref() {
                validate_non_empty_field(author, "target.author")?;
                tags.push(vec!["p".to_string(), author.to_string()]);
            }
            if let Some(kind) = event_kind {
                tags.push(vec!["k".to_string(), kind.to_string()]);
            }
        }
        RadrootsSocialTarget::Address {
            address,
            author,
            event_kind,
            relays,
        } => {
            let parsed = parse_address_tag(address, "target.address")
                .map_err(|_| EventEncodeError::InvalidField("target.address"))?;
            if let Some(kind) = event_kind {
                if *kind != parsed.kind {
                    return Err(EventEncodeError::InvalidField("target.kind"));
                }
            }
            if let Some(author) = author.as_deref() {
                if author != parsed.pubkey {
                    return Err(EventEncodeError::InvalidField("target.author"));
                }
            }
            let mut address_tag = Vec::with_capacity(2 + relays.as_ref().map_or(0, Vec::len));
            address_tag.push("a".to_string());
            address_tag.push(format!(
                "{}:{}:{}",
                parsed.kind, parsed.pubkey, parsed.d_tag
            ));
            if let Some(relays) = relays {
                address_tag.extend(relays.iter().cloned());
            }
            tags.push(address_tag);
            tags.push(vec!["p".to_string(), parsed.pubkey]);
            tags.push(vec!["k".to_string(), parsed.kind.to_string()]);
        }
        RadrootsSocialTarget::External { .. } => {
            return Err(EventEncodeError::InvalidField("target"));
        }
    }
    Ok(())
}
