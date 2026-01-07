#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};
#[cfg(not(feature = "std"))]
use alloc::vec;

use radroots_events::geochat::RadrootsGeoChat;
use radroots_events::kinds::KIND_GEOCHAT;

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_GEOCHAT;
const TAG_G: &str = "g";
const TAG_N: &str = "n";
const TAG_T: &str = "t";
const TAG_T_TELEPORT: &str = "teleport";

fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    tags.push(vec![key.to_string(), value.to_string()]);
}

pub fn geochat_build_tags(
    geochat: &RadrootsGeoChat,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let geohash = geochat.geohash.trim();
    if geohash.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("geohash"));
    }

    let mut tags = Vec::with_capacity(
        1 + usize::from(geochat.nickname.is_some()) + usize::from(geochat.teleported),
    );
    push_tag(&mut tags, TAG_G, geohash);

    if let Some(nickname) = geochat.nickname.as_ref() {
        let nickname = nickname.trim();
        if nickname.is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("nickname"));
        }
        push_tag(&mut tags, TAG_N, nickname);
    }

    if geochat.teleported {
        push_tag(&mut tags, TAG_T, TAG_T_TELEPORT);
    }

    Ok(tags)
}

pub fn to_wire_parts(geochat: &RadrootsGeoChat) -> Result<WireEventParts, EventEncodeError> {
    if geochat.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    let tags = geochat_build_tags(geochat)?;
    Ok(WireEventParts {
        kind: DEFAULT_KIND,
        content: geochat.content.clone(),
        tags,
    })
}
