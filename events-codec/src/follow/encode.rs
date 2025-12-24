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
    let relay = profile.relay_url.as_ref().filter(|v| !v.is_empty());
    let name = profile.contact_name.as_ref().filter(|v| !v.is_empty());
    let mut tag = Vec::with_capacity(2 + usize::from(relay.is_some()) + usize::from(name.is_some()));
    tag.push("p".to_string());
    tag.push(profile.public_key.clone());
    if let Some(relay) = relay {
        tag.push(relay.clone());
    }
    if let Some(name) = name {
        tag.push(name.clone());
    }
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
