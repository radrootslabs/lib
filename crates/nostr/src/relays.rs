use crate::client::RadrootsNostrClient;
use crate::error::RadrootsNostrError;

pub async fn radroots_nostr_add_relay(
    client: &RadrootsNostrClient,
    url: &str,
) -> Result<(), RadrootsNostrError> {
    client.add_relay(url).await?;
    Ok(())
}

pub async fn radroots_nostr_remove_relay(
    client: &RadrootsNostrClient,
    url: &str,
) -> Result<(), RadrootsNostrError> {
    client.force_remove_relay(url).await?;
    Ok(())
}

pub async fn radroots_nostr_connect(client: &RadrootsNostrClient) {
    client.connect().await;
}
