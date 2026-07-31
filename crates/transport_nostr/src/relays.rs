use crate::{RadrootsNostrClient, RadrootsRelayTransportError};

pub async fn radroots_nostr_add_relay(
    client: &RadrootsNostrClient,
    url: &str,
) -> Result<(), RadrootsRelayTransportError> {
    client.add_relay(url).await?;
    Ok(())
}

pub async fn radroots_nostr_remove_relay(
    client: &RadrootsNostrClient,
    url: &str,
) -> Result<(), RadrootsRelayTransportError> {
    client.remove_relay(url).await?;
    Ok(())
}

pub async fn radroots_nostr_connect(client: &RadrootsNostrClient) {
    client.connect().await;
}
