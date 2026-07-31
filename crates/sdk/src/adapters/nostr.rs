use core::time::Duration;

use radroots_nostr::prelude::{RadrootsNostrEvent, RadrootsNostrEventId, RadrootsNostrKeys};
use radroots_transport_nostr::{
    RadrootsNostrClient, RadrootsNostrClientOptions, RadrootsNostrOutput,
    RadrootsRelayTransportError,
};

pub fn signerless_client() -> RadrootsNostrClient {
    RadrootsNostrClient::new_signerless()
}

pub fn signerless_client_with_options(options: RadrootsNostrClientOptions) -> RadrootsNostrClient {
    RadrootsNostrClient::new_signerless_with_options(options)
}

pub fn client_from_keys(keys: RadrootsNostrKeys) -> RadrootsNostrClient {
    RadrootsNostrClient::new(keys)
}

pub async fn configure_write_relays(
    client: &RadrootsNostrClient,
    relay_urls: &[String],
    connect_timeout: Duration,
) -> Result<(), RadrootsRelayTransportError> {
    for relay_url in relay_urls {
        client.add_write_relay(relay_url).await?;
    }
    client.connect().await;
    client.wait_for_connection(connect_timeout).await;
    Ok(())
}

pub async fn connected_client_from_keys(
    keys: RadrootsNostrKeys,
    relay_urls: &[String],
    connect_timeout: Duration,
) -> Result<RadrootsNostrClient, RadrootsRelayTransportError> {
    let client = client_from_keys(keys);
    configure_write_relays(&client, relay_urls, connect_timeout).await?;
    Ok(client)
}

pub async fn connected_relay_urls(client: &RadrootsNostrClient) -> Vec<String> {
    let mut relay_urls = client
        .relays()
        .await
        .into_values()
        .filter(|relay| relay.is_connected())
        .map(|relay| relay.url().to_string())
        .collect::<Vec<_>>();
    relay_urls.sort();
    relay_urls
}

pub async fn publish_signed_event(
    client: &RadrootsNostrClient,
    event: &RadrootsNostrEvent,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsRelayTransportError> {
    client.send_event(event).await
}

#[cfg(test)]
#[path = "../../tests/unit/adapters_nostr_tests.rs"]
mod tests;
