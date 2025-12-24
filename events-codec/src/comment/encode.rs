#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::{
    comment::RadrootsComment,
    RadrootsNostrEventRef,
};

use crate::error::EventEncodeError;
use crate::event_ref::push_nip10_ref_tags;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = 1;

fn validate_ref(
    event: &RadrootsNostrEventRef,
    id_label: &'static str,
    author_label: &'static str,
) -> Result<(), EventEncodeError> {
    if event.id.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField(id_label));
    }
    if event.author.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField(author_label));
    }
    Ok(())
}

pub fn comment_build_tags(comment: &RadrootsComment) -> Result<Vec<Vec<String>>, EventEncodeError> {
    validate_ref(&comment.root, "root.id", "root.author")?;
    validate_ref(&comment.parent, "parent.id", "parent.author")?;

    let root_has_addr = comment
        .root
        .d_tag
        .as_deref()
        .map_or(false, |v| !v.is_empty());
    let parent_has_addr = comment
        .parent
        .d_tag
        .as_deref()
        .map_or(false, |v| !v.is_empty());
    let mut tags = Vec::with_capacity(6 + usize::from(root_has_addr) + usize::from(parent_has_addr));
    push_nip10_ref_tags(&mut tags, &comment.root, "E", "P", "K", "A");
    push_nip10_ref_tags(&mut tags, &comment.parent, "e", "p", "k", "a");
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
