use core::time::Duration;

use crate::adapters::signing::SignedNostrEvent;
use crate::identity::RadrootsIdentity;
use radroots_nostr::prelude::{
    RadrootsNostrClient, RadrootsNostrClientOptions, RadrootsNostrError, RadrootsNostrEventId,
    RadrootsNostrOutput,
};

pub fn signerless_client() -> RadrootsNostrClient {
    RadrootsNostrClient::new_signerless()
}

pub fn signerless_client_with_options(
    options: RadrootsNostrClientOptions,
) -> Result<RadrootsNostrClient, RadrootsNostrError> {
    RadrootsNostrClient::new_signerless_with_options(options)
}

pub fn client_from_identity(identity: &RadrootsIdentity) -> RadrootsNostrClient {
    RadrootsNostrClient::from_identity(identity)
}

pub async fn configure_write_relays(
    client: &RadrootsNostrClient,
    relay_urls: &[String],
    connect_timeout: Duration,
) -> Result<(), RadrootsNostrError> {
    for relay_url in relay_urls {
        client.add_write_relay(relay_url).await?;
    }
    client.connect().await;
    client.wait_for_connection(connect_timeout).await;
    Ok(())
}

pub async fn connected_client_from_identity(
    identity: &RadrootsIdentity,
    relay_urls: &[String],
    connect_timeout: Duration,
) -> Result<RadrootsNostrClient, RadrootsNostrError> {
    let client = client_from_identity(identity);
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
    event: &SignedNostrEvent,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    client.send_event(event).await
}

#[cfg(test)]
#[path = "../../tests/unit/adapters_nostr_tests.rs"]
mod tests;
