use crate::types::{RadrootsNostrFilter, RadrootsNostrKind, RadrootsNostrTimestamp};

pub fn radroots_nostr_kind(kind: u16) -> RadrootsNostrKind {
    RadrootsNostrKind::Custom(kind)
}

pub fn radroots_nostr_filter_kind(kind: u16) -> RadrootsNostrFilter {
    RadrootsNostrFilter::new().kind(RadrootsNostrKind::Custom(kind))
}

pub fn radroots_nostr_filter_new_events(filter: RadrootsNostrFilter) -> RadrootsNostrFilter {
    filter.since(RadrootsNostrTimestamp::now())
}
