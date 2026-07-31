use crate::types::{
    RadrootsNostrEvent, RadrootsNostrPublicKey, RadrootsNostrTimestamp, RadrootsNostrToBech32,
};
use alloc::string::String;

pub fn radroots_nostr_npub_string(pk: &RadrootsNostrPublicKey) -> Option<String> {
    pk.to_bech32().ok()
}

pub fn created_at_u32_saturating(ts: RadrootsNostrTimestamp) -> u32 {
    u32::try_from(ts.as_secs()).unwrap_or(u32::MAX)
}

pub fn event_created_at_u32_saturating(event: &RadrootsNostrEvent) -> u32 {
    created_at_u32_saturating(event.created_at)
}
