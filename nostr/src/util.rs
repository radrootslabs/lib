use crate::types::{
    RadrootsNostrEvent,
    RadrootsNostrPublicKey,
    RadrootsNostrTimestamp,
    RadrootsNostrToBech32,
};

pub fn radroots_nostr_npub_string(pk: &RadrootsNostrPublicKey) -> Option<String> {
    pk.to_bech32().ok()
}

pub fn created_at_u32_saturating(ts: RadrootsNostrTimestamp) -> u32 {
    u32::try_from(ts.as_u64()).unwrap_or(u32::MAX)
}

pub fn event_created_at_u32_saturating(event: &RadrootsNostrEvent) -> u32 {
    created_at_u32_saturating(event.created_at)
}

#[cfg(feature = "http")]
pub fn ws_to_http(ws: &str) -> Option<String> {
    let mut u = reqwest::Url::parse(ws).ok()?;
    let scheme = u.scheme().to_owned();

    let new_scheme = match scheme.as_str() {
        "wss" => "https",
        "ws" => "http",
        other => other,
    };

    u.set_scheme(new_scheme).ok()?;
    Some(u.into())
}
