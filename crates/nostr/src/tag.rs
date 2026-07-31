//! Portable Nostr tag conversion and validation.
//!
//! Ordered protocol-neutral tag parts cross the upstream Nostr boundary only
//! through this module's validated conversion functions.

use alloc::{string::String, vec::Vec};

use crate::Error;

/// Upstream Nostr tag used only at the explicit protocol boundary.
pub type Tag = nostr::Tag;
/// Upstream Nostr tag kind used only at the explicit protocol boundary.
pub type TagKind<'a> = nostr::TagKind<'a>;
/// Upstream standardized Nostr tag used only at the explicit protocol boundary.
pub type TagStandard = nostr::TagStandard;

pub use crate::error::ResolveError;
pub use crate::tags::{
    tag_at_value as value_at, tag_first_value as first_value, tag_match_geohash as match_geohash,
    tag_match_l as match_location_coordinate, tag_match_location as match_location,
    tag_match_summary as match_summary, tag_match_title as match_title,
    tag_relays_parse as relay_urls, tag_slice as values_from, tags_match as match_parts,
    tags_resolve as resolve,
};

/// Parses canonical tag parts into the explicit Nostr tag boundary value.
pub fn from_parts(parts: Vec<String>) -> Result<Tag, Error> {
    Tag::parse(parts).map_err(|_| Error::TagConversion)
}

/// Copies a Nostr tag into its protocol-neutral ordered string parts.
#[must_use]
pub fn to_parts(tag: &Tag) -> Vec<String> {
    tag.as_slice().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_parts_round_trip_without_upstream_error_leakage() {
        let parts = vec!["t".to_owned(), "soil".to_owned()];
        let tag = from_parts(parts.clone()).expect("tag");

        assert_eq!(to_parts(&tag), parts);
        assert!(matches!(from_parts(Vec::new()), Err(Error::TagConversion)));
    }
}
