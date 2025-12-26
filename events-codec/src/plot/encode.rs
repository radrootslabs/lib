#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    farm::RadrootsFarmRef,
    kinds::KIND_FARM,
    plot::RadrootsPlot,
    tags::TAG_D,
};

#[cfg(feature = "serde_json")]
use radroots_events::kinds::KIND_PLOT;

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

fn farm_address(farm: &RadrootsFarmRef) -> String {
    let mut value = String::new();
    value.push_str(&KIND_FARM.to_string());
    value.push(':');
    value.push_str(&farm.pubkey);
    value.push(':');
    value.push_str(&farm.d_tag);
    value
}

pub fn plot_build_tags(plot: &RadrootsPlot) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if plot.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    if plot.name.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("name"));
    }
    if plot.farm.pubkey.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.pubkey"));
    }
    if plot.farm.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("farm.d_tag"));
    }
    let mut tags = Vec::new();
    push_tag(&mut tags, TAG_D, &plot.d_tag);
    push_tag(&mut tags, TAG_A, &farm_address(&plot.farm));
    push_tag(&mut tags, TAG_P, &plot.farm.pubkey);
    if let Some(items) = plot.tags.as_ref() {
        for item in items.iter().filter(|v| !v.trim().is_empty()) {
            push_tag(&mut tags, TAG_T, item);
        }
    }
    if let Some(location) = plot.location.as_ref() {
        if let Some(geohash) = location.geohash.as_ref().filter(|v| !v.trim().is_empty()) {
            push_tag(&mut tags, TAG_G, geohash);
        }
    }
    Ok(tags)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(plot: &RadrootsPlot) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(plot, KIND_PLOT)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    plot: &RadrootsPlot,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_PLOT {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = plot_build_tags(plot)?;
    let content = serde_json::to_string(plot).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts { kind, content, tags })
}
