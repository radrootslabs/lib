#[cfg(all(feature = "http", feature = "codec"))]
use radroots_events::relay_document::models::RadrootsRelayDocument;

#[cfg(all(feature = "http", feature = "codec"))]
use crate::util::ws_to_http;

#[cfg(all(feature = "http", feature = "codec"))]
pub async fn fetch_nip11(ws_url: &str) -> Option<RadrootsRelayDocument> {
    let http_url = ws_to_http(ws_url)?;
    let client = reqwest::Client::new();
    client
        .get(&http_url)
        .header("Accept", "application/nostr+json")
        .send()
        .await
        .ok()?
        .json::<RadrootsRelayDocument>()
        .await
        .ok()
}
