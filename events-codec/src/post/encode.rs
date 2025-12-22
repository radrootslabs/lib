#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use radroots_events::post::RadrootsPost;

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = 1;

pub fn to_wire_parts(post: &RadrootsPost) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(post, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    post: &RadrootsPost,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if post.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    Ok(WireEventParts {
        kind,
        content: post.content.clone(),
        tags: Vec::new(),
    })
}
