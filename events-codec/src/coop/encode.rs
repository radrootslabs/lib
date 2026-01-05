#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    coop::{RadrootsCoop, RadrootsCoopRef},
    kinds::KIND_COOP,
    tags::TAG_D,
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;

#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;

const TAG_T: &str = "t";
const TAG_G: &str = "g";

fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    let mut tag = Vec::with_capacity(2);
    tag.push(key.to_string());
    tag.push(value.to_string());
    tags.push(tag);
}

pub fn coop_build_tags(coop: &RadrootsCoop) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if coop.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    validate_d_tag(&coop.d_tag, "d_tag")?;
    if coop.name.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("name"));
    }
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, &coop.d_tag);
    if let Some(items) = coop.tags.as_ref() {
        for item in items.iter().filter(|v| !v.trim().is_empty()) {
            push_tag(&mut tags, TAG_T, item);
        }
    }
    if let Some(location) = coop.location.as_ref() {
        let geohash = location.gcs.geohash.trim();
        if geohash.is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("location.gcs.geohash"));
        }
        push_tag(&mut tags, TAG_G, geohash);
    }
    Ok(tags)
}

pub fn coop_ref_tags(coop: &RadrootsCoopRef) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if coop.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("coop.pubkey"));
    }
    if coop.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("coop.d_tag"));
    }
    validate_d_tag(&coop.d_tag, "coop.d_tag")?;
    let mut addr = String::new();
    addr.push_str(&KIND_COOP.to_string());
    addr.push(':');
    addr.push_str(&coop.pubkey);
    addr.push(':');
    addr.push_str(&coop.d_tag);
    let mut tags = Vec::with_capacity(2);
    push_tag(&mut tags, "p", &coop.pubkey);
    push_tag(&mut tags, "a", &addr);
    Ok(tags)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(coop: &RadrootsCoop) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(coop, KIND_COOP)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    coop: &RadrootsCoop,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_COOP {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = coop_build_tags(coop)?;
    let content = serde_json::to_string(coop).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts { kind, content, tags })
}
