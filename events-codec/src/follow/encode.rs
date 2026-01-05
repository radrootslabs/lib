#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::follow::{RadrootsFollow, RadrootsFollowProfile};

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;
use radroots_events::kinds::KIND_FOLLOW;

const DEFAULT_KIND: u32 = KIND_FOLLOW;

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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FollowMutation {
    Follow {
        public_key: String,
        relay_url: Option<String>,
        contact_name: Option<String>,
    },
    Unfollow {
        public_key: String,
    },
    Toggle {
        public_key: String,
        relay_url: Option<String>,
        contact_name: Option<String>,
    },
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

pub fn follow_apply(
    follow: &RadrootsFollow,
    mutation: FollowMutation,
) -> Result<RadrootsFollow, EventEncodeError> {
    let mut list = normalize_list(&follow.list)?;

    match mutation {
        FollowMutation::Follow {
            public_key,
            relay_url,
            contact_name,
        } => {
            let public_key = normalize_public_key(&public_key)?;
            let relay_url = normalize_optional(relay_url);
            let contact_name = normalize_optional(contact_name);
            apply_follow(&mut list, public_key, relay_url, contact_name);
        }
        FollowMutation::Unfollow { public_key } => {
            let public_key = normalize_public_key(&public_key)?;
            list.retain(|entry| entry.public_key != public_key);
        }
        FollowMutation::Toggle {
            public_key,
            relay_url,
            contact_name,
        } => {
            let public_key = normalize_public_key(&public_key)?;
            if list.iter().any(|entry| entry.public_key == public_key) {
                list.retain(|entry| entry.public_key != public_key);
            } else {
                let relay_url = normalize_optional(relay_url);
                let contact_name = normalize_optional(contact_name);
                list.push(RadrootsFollowProfile {
                    published_at: 0,
                    public_key,
                    relay_url,
                    contact_name,
                });
            }
        }
    }

    Ok(RadrootsFollow { list })
}

pub fn follow_to_wire_parts_after(
    follow: &RadrootsFollow,
    mutation: FollowMutation,
) -> Result<WireEventParts, EventEncodeError> {
    let updated = follow_apply(follow, mutation)?;
    to_wire_parts(&updated)
}

fn normalize_public_key(value: &str) -> Result<String, EventEncodeError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("follow.public_key"));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_list(
    list: &[RadrootsFollowProfile],
) -> Result<Vec<RadrootsFollowProfile>, EventEncodeError> {
    let mut out = Vec::with_capacity(list.len());
    for entry in list {
        let public_key = normalize_public_key(&entry.public_key)?;
        if out
            .iter()
            .any(|item: &RadrootsFollowProfile| item.public_key == public_key)
        {
            continue;
        }
        let mut normalized = entry.clone();
        normalized.public_key = public_key;
        normalized.relay_url = normalize_optional(normalized.relay_url);
        normalized.contact_name = normalize_optional(normalized.contact_name);
        out.push(normalized);
    }
    Ok(out)
}

fn apply_follow(
    list: &mut Vec<RadrootsFollowProfile>,
    public_key: String,
    relay_url: Option<String>,
    contact_name: Option<String>,
) {
    if let Some(pos) = list.iter().position(|entry| entry.public_key == public_key) {
        let mut entry = list[pos].clone();
        if let Some(relay_url) = relay_url {
            entry.relay_url = Some(relay_url);
        }
        if let Some(contact_name) = contact_name {
            entry.contact_name = Some(contact_name);
        }
        list[pos] = entry;
    } else {
        list.push(RadrootsFollowProfile {
            published_at: 0,
            public_key,
            relay_url,
            contact_name,
        });
    }
}
