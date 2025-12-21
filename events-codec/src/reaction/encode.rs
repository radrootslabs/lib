#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    reaction::RadrootsReaction,
    tags::TAG_E_ROOT,
    RadrootsNostrEventRef,
};

use crate::error::EventEncodeError;
use crate::event_ref::build_event_ref_tag;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = 7;

fn validate_ref(event: &RadrootsNostrEventRef) -> Result<(), EventEncodeError> {
    if event.id.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("root.id"));
    }
    if event.author.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("root.author"));
    }
    Ok(())
}

pub fn reaction_build_tags(
    reaction: &RadrootsReaction,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_ref(&reaction.root)?;
    let mut tags = Vec::with_capacity(1);
    tags.push(build_event_ref_tag(TAG_E_ROOT, &reaction.root));
    Ok(tags)
}

pub fn to_wire_parts(reaction: &RadrootsReaction) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(reaction, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    reaction: &RadrootsReaction,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if reaction.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    let tags = reaction_build_tags(reaction)?;
    Ok(WireEventParts {
        kind,
        content: reaction.content.clone(),
        tags,
    })
}
