#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use radroots_events::{
    RadrootsNostrEvent,
    kinds::{KIND_FARM, KIND_PLOT, KIND_RESOURCE_AREA},
    kinds::KIND_LISTING,
    listing::{RadrootsListing, RadrootsListingEventIndex, RadrootsListingEventMetadata, RadrootsListingFarmRef},
    plot::RadrootsPlotRef,
    resource_area::RadrootsResourceAreaRef,
    tags::TAG_D,
};

use crate::d_tag::validate_d_tag_tag;
use crate::error::EventParseError;

const DEFAULT_KIND: u32 = KIND_LISTING;
const TAG_A: &str = "a";
const TAG_P: &str = "p";
const TAG_RADROOTS_RESOURCE_AREA: &str = "radroots:resource_area";
const TAG_RADROOTS_PLOT: &str = "radroots:plot";

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
    validate_d_tag_tag(&value, TAG_D)?;
    Ok(value)
}

fn parse_farm_ref(tags: &[Vec<String>]) -> Result<RadrootsListingFarmRef, EventParseError> {
    for tag in tags.iter().filter(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_A)) {
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
            continue;
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
        validate_d_tag_tag(&d_tag, TAG_A)?;
        return Ok(RadrootsListingFarmRef { pubkey, d_tag });
    }
    Err(EventParseError::MissingTag(TAG_A))
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

fn parse_resource_area(tags: &[Vec<String>]) -> Result<Option<RadrootsResourceAreaRef>, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_RADROOTS_RESOURCE_AREA));
    let Some(tag) = tag else {
        return Ok(None);
    };
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA))?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA))?;
    if kind != KIND_RESOURCE_AREA {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA));
    }
    let pubkey = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA))?
        .to_string();
    let d_tag = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA))?
        .to_string();
    if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA));
    }
    validate_d_tag_tag(&d_tag, TAG_RADROOTS_RESOURCE_AREA)?;
    Ok(Some(RadrootsResourceAreaRef { pubkey, d_tag }))
}

fn parse_plot_ref(tags: &[Vec<String>]) -> Result<Option<RadrootsPlotRef>, EventParseError> {
    let tag = tags
        .iter()
        .find(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_RADROOTS_PLOT));
    let Some(tag) = tag else {
        return Ok(None);
    };
    let value = tag
        .get(1)
        .map(|s| s.to_string())
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PLOT))?;
    let mut parts = value.splitn(3, ':');
    let kind = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PLOT))?;
    if kind != KIND_PLOT {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_PLOT));
    }
    let pubkey = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PLOT))?
        .to_string();
    let d_tag = parts
        .next()
        .ok_or(EventParseError::InvalidTag(TAG_RADROOTS_PLOT))?
        .to_string();
    if pubkey.trim().is_empty() || d_tag.trim().is_empty() {
        return Err(EventParseError::InvalidTag(TAG_RADROOTS_PLOT));
    }
    validate_d_tag_tag(&d_tag, TAG_RADROOTS_PLOT)?;
    Ok(Some(RadrootsPlotRef { pubkey, d_tag }))
}

pub fn listing_from_event(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsListing, EventParseError> {
    if kind != DEFAULT_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "30402",
            got: kind,
        });
    }
    if content.trim().is_empty() {
        return Err(EventParseError::InvalidJson("content"));
    }
    let d_tag = parse_d_tag(tags)?;
    let farm_ref = parse_farm_ref(tags)?;
    let farm_pubkey = parse_farm_pubkey(tags)?;
    let resource_area = parse_resource_area(tags)?;
    let plot = parse_plot_ref(tags)?;
    let mut listing: RadrootsListing =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;

    if listing.d_tag.trim().is_empty() {
        listing.d_tag = d_tag;
    } else if listing.d_tag != d_tag {
        return Err(EventParseError::InvalidTag(TAG_D));
    }

    if listing.farm.pubkey.trim().is_empty() || listing.farm.d_tag.trim().is_empty() {
        listing.farm = farm_ref;
    } else if listing.farm.pubkey != farm_ref.pubkey || listing.farm.d_tag != farm_ref.d_tag {
        return Err(EventParseError::InvalidTag(TAG_A));
    }
    if listing.farm.pubkey != farm_pubkey {
        return Err(EventParseError::InvalidTag(TAG_P));
    }

    if let Some(tag_area) = resource_area {
        match listing.resource_area.as_ref() {
            None => listing.resource_area = Some(tag_area),
            Some(area) => {
                if area.pubkey != tag_area.pubkey || area.d_tag != tag_area.d_tag {
                    return Err(EventParseError::InvalidTag(TAG_RADROOTS_RESOURCE_AREA));
                }
            }
        }
    }

    if let Some(tag_plot) = plot {
        match listing.plot.as_ref() {
            None => listing.plot = Some(tag_plot),
            Some(existing) => {
                if existing.pubkey != tag_plot.pubkey || existing.d_tag != tag_plot.d_tag {
                    return Err(EventParseError::InvalidTag(TAG_RADROOTS_PLOT));
                }
            }
        }
    }

    Ok(listing)
}

pub fn metadata_from_event(
    id: String,
    author: String,
    published_at: u32,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsListingEventMetadata, EventParseError> {
    let listing = listing_from_event(kind, &tags, &content)?;
    Ok(RadrootsListingEventMetadata {
        id,
        author,
        published_at,
        kind,
        listing,
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
) -> Result<RadrootsListingEventIndex, EventParseError> {
    let metadata = metadata_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    Ok(RadrootsListingEventIndex {
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
