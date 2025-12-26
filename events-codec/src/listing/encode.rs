#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use alloc::string::String;

use radroots_events::listing::RadrootsListing;
#[cfg(feature = "serde_json")]
use radroots_events::kinds::KIND_LISTING;

use crate::error::EventEncodeError;
use crate::listing::tags::listing_tags;
#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;

#[cfg(feature = "serde_json")]
const DEFAULT_KIND: u32 = KIND_LISTING;

pub fn listing_build_tags(listing: &RadrootsListing) -> Result<Vec<Vec<String>>, EventEncodeError> {
    listing_tags(listing)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(listing: &RadrootsListing) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(listing, DEFAULT_KIND)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_kind(
    listing: &RadrootsListing,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    let tags = listing_build_tags(listing)?;
    let content = serde_json::to_string(listing).map_err(|_| EventEncodeError::Json)?;
    Ok(WireEventParts { kind, content, tags })
}
