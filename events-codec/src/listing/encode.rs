#![cfg(feature = "serde_json")]

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec, vec::Vec};

use radroots_events::{listing::RadrootsListing, tags::TAG_D};

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = 30402;

pub fn listing_build_tags(listing: &RadrootsListing) -> Result<Vec<Vec<String>>, EventEncodeError> {
    let d_tag = listing.d_tag.trim();
    if d_tag.is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d"));
    }
    let mut tags = Vec::with_capacity(1);
    tags.push(vec![TAG_D.to_string(), d_tag.to_string()]);
    Ok(tags)
}

pub fn to_wire_parts(listing: &RadrootsListing) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(listing, DEFAULT_KIND)
}

pub fn to_wire_parts_with_kind(
    listing: &RadrootsListing,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    let tags = listing_build_tags(listing)?;
    let content = serde_json::to_string(listing).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts { kind, content, tags })
}
