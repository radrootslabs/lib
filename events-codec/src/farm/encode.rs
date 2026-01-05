#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    farm::{RadrootsFarm, RadrootsFarmRef},
    kinds::KIND_FARM,
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

pub fn farm_build_tags(farm: &RadrootsFarm) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if farm.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    validate_d_tag(&farm.d_tag, "d_tag")?;
    if farm.name.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("name"));
    }
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, &farm.d_tag);
    if let Some(items) = farm.tags.as_ref() {
        for item in items.iter().filter(|v| !v.trim().is_empty()) {
            push_tag(&mut tags, TAG_T, item);
        }
    }
    if let Some(location) = farm.location.as_ref() {
        let geohash = location.gcs.geohash.trim();
        if geohash.is_empty() {
            return Err(EventEncodeError::EmptyRequiredField("location.gcs.geohash"));
        }
        push_tag(&mut tags, TAG_G, geohash);
    }
    Ok(tags)
}

pub fn farm_ref_tags(farm: &RadrootsFarmRef) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if farm.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.pubkey"));
    }
    if farm.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.d_tag"));
    }
    validate_d_tag(&farm.d_tag, "farm.d_tag")?;
    let mut addr = String::new();
    addr.push_str(&KIND_FARM.to_string());
    addr.push(':');
    addr.push_str(&farm.pubkey);
    addr.push(':');
    addr.push_str(&farm.d_tag);
    let mut tags = Vec::with_capacity(2);
    push_tag(&mut tags, "p", &farm.pubkey);
    push_tag(&mut tags, "a", &addr);
    Ok(tags)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(farm: &RadrootsFarm) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(farm, KIND_FARM)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    farm: &RadrootsFarm,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_FARM {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = farm_build_tags(farm)?;
    let content = serde_json::to_string(farm).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts { kind, content, tags })
}
