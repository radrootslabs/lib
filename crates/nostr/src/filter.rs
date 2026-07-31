//! Explicit construction of portable Nostr subscription filters.
//!
//! Filters are passive protocol values. Constructing one does not subscribe,
//! contact a relay, install a clock, or retain runtime state.

use crate::Error;
use crate::event::Kind;
#[cfg(feature = "std")]
use crate::event::Timestamp;
use alloc::{string::String, string::ToString, vec::Vec};

/// Upstream Nostr filter used only at the explicit protocol boundary.
pub type Filter = nostr::Filter;

pub fn with_tag(filter: Filter, tag: &str, values: Vec<String>) -> Result<Filter, Error> {
    let mut chars = tag.chars();
    let tag_char = chars
        .next()
        .ok_or_else(|| Error::FilterTagError("tag is empty".to_string()))?;
    if chars.next().is_some() {
        return Err(Error::FilterTagError(
            "tag must be a single letter".to_string(),
        ));
    }
    let tag_key = nostr::filter::SingleLetterTag::from_char(tag_char)
        .map_err(|err| Error::FilterTagError(err.to_string()))?;
    Ok(filter.custom_tags(tag_key, values))
}

pub fn kind(kind: u16) -> Kind {
    Kind::Custom(kind)
}

pub fn for_kind(kind: u16) -> Filter {
    Filter::new().kind(Kind::Custom(kind))
}

#[cfg(feature = "std")]
pub fn since_now(filter: Filter) -> Filter {
    filter.since(Timestamp::now())
}
