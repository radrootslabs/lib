//! Explicit construction of portable Nostr subscription filters.

use crate::error::RadrootsNostrError;
use crate::event::Kind;
#[cfg(feature = "std")]
use crate::event::Timestamp;
use alloc::{string::String, string::ToString, vec::Vec};

/// Upstream Nostr filter used only at the explicit protocol boundary.
pub type Filter = nostr::Filter;

pub fn radroots_nostr_filter_tag(
    filter: Filter,
    tag: &str,
    values: Vec<String>,
) -> Result<Filter, RadrootsNostrError> {
    let mut chars = tag.chars();
    let tag_char = chars
        .next()
        .ok_or_else(|| RadrootsNostrError::FilterTagError("tag is empty".to_string()))?;
    if chars.next().is_some() {
        return Err(RadrootsNostrError::FilterTagError(
            "tag must be a single letter".to_string(),
        ));
    }
    let tag_key = nostr::filter::SingleLetterTag::from_char(tag_char)
        .map_err(|err| RadrootsNostrError::FilterTagError(err.to_string()))?;
    Ok(filter.custom_tags(tag_key, values))
}

pub fn radroots_nostr_kind(kind: u16) -> Kind {
    Kind::Custom(kind)
}

pub fn radroots_nostr_filter_kind(kind: u16) -> Filter {
    Filter::new().kind(Kind::Custom(kind))
}

#[cfg(feature = "std")]
pub fn radroots_nostr_filter_new_events(filter: Filter) -> Filter {
    filter.since(Timestamp::now())
}
