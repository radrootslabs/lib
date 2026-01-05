#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    kinds::KIND_RESOURCE_AREA,
    resource_area::{RadrootsResourceArea, RadrootsResourceAreaRef},
    tags::TAG_D,
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;

#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;

const TAG_T: &str = "t";
const TAG_G: &str = "g";
const TAG_A: &str = "a";
const TAG_P: &str = "p";

fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    let mut tag = Vec::with_capacity(2);
    tag.push(key.to_string());
    tag.push(value.to_string());
    tags.push(tag);
}

fn resource_area_address(area: &RadrootsResourceAreaRef) -> Result<String, EventEncodeError> {
    if area.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("resource_area.pubkey"));
    }
    if area.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("resource_area.d_tag"));
    }
    validate_d_tag(&area.d_tag, "resource_area.d_tag")?;
    let mut addr = String::new();
    addr.push_str(&KIND_RESOURCE_AREA.to_string());
    addr.push(':');
    addr.push_str(&area.pubkey);
    addr.push(':');
    addr.push_str(&area.d_tag);
    Ok(addr)
}

pub fn resource_area_build_tags(
    area: &RadrootsResourceArea,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if area.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    validate_d_tag(&area.d_tag, "d_tag")?;
    if area.name.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("name"));
    }
    let geohash = area.location.gcs.geohash.trim();
    if geohash.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("location.gcs.geohash"));
    }
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, &area.d_tag);
    if let Some(items) = area.tags.as_ref() {
        for item in items.iter().filter(|v| !v.trim().is_empty()) {
            push_tag(&mut tags, TAG_T, item);
        }
    }
    push_tag(&mut tags, TAG_G, geohash);
    Ok(tags)
}

pub fn resource_area_ref_tags(
    area: &RadrootsResourceAreaRef,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let addr = resource_area_address(area)?;
    let mut tags = Vec::with_capacity(2);
    push_tag(&mut tags, TAG_P, &area.pubkey);
    push_tag(&mut tags, TAG_A, &addr);
    Ok(tags)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(area: &RadrootsResourceArea) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(area, KIND_RESOURCE_AREA)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    area: &RadrootsResourceArea,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_RESOURCE_AREA {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = resource_area_build_tags(area)?;
    let content = serde_json::to_string(area).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts { kind, content, tags })
}
