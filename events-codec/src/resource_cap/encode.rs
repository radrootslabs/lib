#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    kinds::KIND_RESOURCE_AREA,
    resource_cap::RadrootsResourceHarvestCap,
    tags::TAG_D,
};

use crate::d_tag::validate_d_tag;
use crate::error::EventEncodeError;

#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;
#[cfg(feature = "serde_json")]
use radroots_events::kinds::KIND_RESOURCE_HARVEST_CAP;

const TAG_A: &str = "a";
const TAG_P: &str = "p";
const TAG_T: &str = "t";
const TAG_KEY: &str = "key";
const TAG_CATEGORY: &str = "category";
const TAG_START: &str = "start";
const TAG_END: &str = "end";

fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    let mut tag = Vec::with_capacity(2);
    tag.push(key.to_string());
    tag.push(value.to_string());
    tags.push(tag);
}

fn resource_area_address(cap: &RadrootsResourceHarvestCap) -> Result<String, EventEncodeError> {
    let area = &cap.resource_area;
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

pub fn resource_harvest_cap_build_tags(
    cap: &RadrootsResourceHarvestCap,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if cap.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    validate_d_tag(&cap.d_tag, "d_tag")?;
    if cap.product.key.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("product.key"));
    }
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, &cap.d_tag);
    let addr = resource_area_address(cap)?;
    push_tag(&mut tags, TAG_A, &addr);
    push_tag(&mut tags, TAG_P, &cap.resource_area.pubkey);
    push_tag(&mut tags, TAG_KEY, &cap.product.key);
    if let Some(category) = cap.product.category.as_deref() {
        if !category.trim().is_empty() {
            push_tag(&mut tags, TAG_CATEGORY, category);
        }
    }
    push_tag(&mut tags, TAG_START, &cap.start.to_string());
    push_tag(&mut tags, TAG_END, &cap.end.to_string());
    if let Some(items) = cap.tags.as_ref() {
        for item in items.iter().filter(|v| !v.trim().is_empty()) {
            push_tag(&mut tags, TAG_T, item);
        }
    }
    Ok(tags)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(
    cap: &RadrootsResourceHarvestCap,
) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(cap, KIND_RESOURCE_HARVEST_CAP)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    cap: &RadrootsResourceHarvestCap,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_RESOURCE_HARVEST_CAP {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = resource_harvest_cap_build_tags(cap)?;
    let content = serde_json::to_string(cap).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts { kind, content, tags })
}
