use radroots_event::social::relay_document::RelayDocument;
pub async fn fetch_nip11(ws_url: &str) -> Option<RelayDocument> {
    let http_url = ws_to_http(ws_url)?;
    let client = reqwest::Client::new();
    client
        .get(&http_url)
        .header("Accept", "application/nostr+json")
        .send()
        .await
        .ok()?
        .json::<RelayDocument>()
        .await
        .ok()
}

fn ws_to_http(ws: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(ws).ok()?;
    let scheme = url.scheme().to_owned();
    let replacement = match scheme.as_str() {
        "wss" => "https",
        "ws" => "http",
        other => other,
    };
    url.set_scheme(replacement).ok()?;
    Some(url.into())
}
