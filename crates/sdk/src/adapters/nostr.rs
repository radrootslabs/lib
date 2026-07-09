use core::time::Duration;

use crate::adapters::signing::SignedNostrEvent;
use crate::identity::RadrootsIdentity;
use radroots_nostr::prelude::{
    RadrootsNostrClient, RadrootsNostrClientOptions, RadrootsNostrError, RadrootsNostrEventId,
    RadrootsNostrOutput,
};

pub type RelayClient = RadrootsNostrClient;
pub type RelayClientOptions = RadrootsNostrClientOptions;
pub type RelayError = RadrootsNostrError;
pub type RelayEventId = RadrootsNostrEventId;
pub type RelayOutput<T> = RadrootsNostrOutput<T>;

pub fn signerless_client() -> RelayClient {
    RelayClient::new_signerless()
}

pub fn signerless_client_with_options(
    options: RelayClientOptions,
) -> Result<RelayClient, RelayError> {
    RelayClient::new_signerless_with_options(options)
}

pub fn client_from_identity(identity: &RadrootsIdentity) -> RelayClient {
    RelayClient::from_identity(identity)
}

pub async fn configure_write_relays(
    client: &RelayClient,
    relay_urls: &[String],
    connect_timeout: Duration,
) -> Result<(), RelayError> {
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
) -> Result<RelayClient, RelayError> {
    let client = client_from_identity(identity);
    configure_write_relays(&client, relay_urls, connect_timeout).await?;
    Ok(client)
}

pub async fn connected_relay_urls(client: &RelayClient) -> Vec<String> {
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
    client: &RelayClient,
    event: &SignedNostrEvent,
) -> Result<RelayOutput<RelayEventId>, RelayError> {
    client.send_event(event).await
}

#[cfg(test)]
#[path = "../../tests/unit/adapters_nostr_tests.rs"]
mod tests;
