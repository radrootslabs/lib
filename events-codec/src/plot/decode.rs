#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::{KIND_FARM, KIND_PLOT},
    farm::RadrootsFarmRef,
    plot::{RadrootsPlot, RadrootsPlotEventIndex, RadrootsPlotEventMetadata},
    tags::TAG_D,
};

use crate::error::EventParseError;

const TAG_A: &str = "a";
const TAG_P: &str = "p";
const DEFAULT_KIND: u32 = KIND_PLOT;

fn parse_d_tag(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_D))
        .ok_or(EventParseError::MissingTag(TAG_D))?;
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_D))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_D));
    }
    Ok(value)
}

fn parse_farm_ref(tags: &[Vec<String>]) -> Result<RadrootsFarmRef, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_A))
        .ok_or(EventParseError::MissingTag(TAG_A))?;
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_A))?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or(EventParseError::InvalidTag(TAG_A))?;
    if kind != KIND_FARM {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    let pubkey = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_A))?
        .to_string();
    let d_tag = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_A))?
        .to_string();
    if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    Ok(RadrootsFarmRef { pubkey, d_tag })
}

fn parse_farm_pubkey(tags: &[Vec<String>]) -> Result<String, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_P))
        .ok_or(EventParseError::MissingTag(TAG_P))?;
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_P))?;
    if value.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_P));
    }
    Ok(value)
}

pub fn plot_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsPlot, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "30350",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = parse_d_tag(tags)?;
    let farm_ref = parse_farm_ref(tags)?;
    let farm_pubkey = parse_farm_pubkey(tags)?;
    let mut plot: RadrootsPlot =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;

    if plot.d_tag.trim().is_empty() {
        plot.d_tag = d_tag;
    } else if plot.d_tag != d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }

    if plot.farm.pubkey.trim().is_empty() || plot.farm.d_tag.trim().is_empty() {
        plot.farm = farm_ref;
    } else if plot.farm.pubkey != farm_ref.pubkey || plot.farm.d_tag != farm_ref.d_tag {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    if plot.farm.pubkey != farm_pubkey {
        return Err(EventParseError::InvalidTag(TAG_P));
    }

    Ok(plot)
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsPlotEventMetadata, EventParseError> {
    let plot = plot_from_event(kind, &tags, &content)?;
    Ok(RadrootsPlotEventMetadata {
        id,
        author,
        published_at,
        kind,
        plot,
    })
}

pub fn index_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsPlotEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsPlotEventIndex {
        event: RadrootsNostrEvent {
            id,
            author,
            created_at: published_at,
            kind,
            content,
            tags,
            sig,
        },
        metadata,
    })
}
