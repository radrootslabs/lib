use nostr::{event::Event, key::PublicKey, nips::nip19::ToBech32, Timestamp};

pub fn npub_string(pk: &PublicKey) -> Option<String> {
    pk.to_bech32().ok()
}

pub fn created_at_u32_saturating(ts: Timestamp) -> u32 {
    u32::try_from(ts.as_u64()).unwrap_or(u32::MAX)
}

pub fn event_created_at_u32_saturating(event: &Event) -> u32 {
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
