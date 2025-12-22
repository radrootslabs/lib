#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::follow::{RadrootsFollow, RadrootsFollowProfile};

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = 3;

fn follow_tag(profile: &RadrootsFollowProfile) -> Result<Vec<String>, EventEncodeError> {
    if profile.public_key.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("follow.public_key"));
    }
    let mut tag = Vec::with_capacity(5);
    tag.push("p".to_string());
    tag.push(profile.public_key.clone());
    tag.push(profile.relay_url.clone().unwrap_or_default());
    tag.push(profile.contact_name.clone().unwrap_or_default());
    tag.push(profile.published_at.to_string());
    Ok(tag)
}

pub fn follow_build_tags(follow: &RadrootsFollow) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let mut tags = Vec::with_capacity(follow.list.len());
    for profile in &follow.list {
        tags.push(follow_tag(profile)?);
    }
    Ok(tags)
}

pub fn to_wire_parts(follow: &RadrootsFollow) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(follow, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    follow: &RadrootsFollow,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    let tags = follow_build_tags(follow)?;
    Ok(WireEventParts {
        kind,
        content: String::new(),
        tags,
    })
}
