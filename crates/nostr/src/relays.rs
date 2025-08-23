use crate::error::NostrUtilsError;
use nostr_sdk::Client;

pub async fn add_relay(client: &Client, url: &str) -> Result<(), NostrUtilsError> {
    client.add_relay(url).await?;
    Ok(())
}

pub async fn remove_relay(client: &Client, url: &str) -> Result<(), NostrUtilsError> {
    client.force_remove_relay(url).await?;
    Ok(())
}

pub async fn connect(client: &Client) {
    client.connect().await;
}
