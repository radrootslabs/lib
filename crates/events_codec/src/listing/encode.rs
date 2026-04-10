#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

#[cfg(feature = "serde_json")]
use radroots_events::kinds::{KIND_LISTING, is_listing_kind};
use radroots_events::listing::RadrootsListing;

use crate::error::EventEncodeError;
use crate::listing::tags::listing_tags;
#[cfg(feature = "serde_json")]
use crate::listing::tags::listing_tags_full;
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
    if !is_listing_kind(kind) {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = listing_tags_full(listing)?;
    let content = listing_markdown_content(listing);
    Ok(WireEventParts {
        kind,
        content,
        tags,
    })
}

#[cfg(feature = "serde_json")]
fn listing_markdown_content(listing: &RadrootsListing) -> String {
    let title = listing.product.title.trim();
    let summary = listing
        .product
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match (title.is_empty(), summary) {
        (false, Some(summary)) => format!("# {title}\n\n{summary}"),
        (false, None) => format!("# {title}"),
        (true, Some(summary)) => summary.to_string(),
        (true, None) => String::new(),
    }
}
