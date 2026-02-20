use crate::error::RadrootsNostrError;
use crate::types::{RadrootsNostrFilter, RadrootsNostrKind, RadrootsNostrTimestamp};

pub fn radroots_nostr_filter_tag(
    filter: RadrootsNostrFilter,
    tag: &str,
    values: Vec<String>,
) -> Result<RadrootsNostrFilter, RadrootsNostrError> {
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

pub fn radroots_nostr_kind(kind: u16) -> RadrootsNostrKind {
    RadrootsNostrKind::Custom(kind)
}

pub fn radroots_nostr_filter_kind(kind: u16) -> RadrootsNostrFilter {
    RadrootsNostrFilter::new().kind(RadrootsNostrKind::Custom(kind))
}

pub fn radroots_nostr_filter_new_events(filter: RadrootsNostrFilter) -> RadrootsNostrFilter {
    filter.since(RadrootsNostrTimestamp::now())
}
