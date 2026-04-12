use crate::WireEventParts;
use crate::adapters::signing::{SignedNostrEvent, event_builder_from_parts};
use crate::identity::RadrootsIdentity;
use radroots_nostr::prelude::{
    RadrootsNostrClient, RadrootsNostrClientOptions, RadrootsNostrError,
    RadrootsNostrEventId, RadrootsNostrOutput,
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

pub async fn publish_parts(
    client: &RelayClient,
    parts: WireEventParts,
) -> Result<RelayOutput<RelayEventId>, RelayError> {
    client.send_event_builder(event_builder_from_parts(parts)?).await
}

pub async fn publish_signed_event(
    client: &RelayClient,
    event: &SignedNostrEvent,
) -> Result<RelayOutput<RelayEventId>, RelayError> {
    client.send_event(event).await
}

#[cfg(test)]
mod tests {
    use super::{client_from_identity, signerless_client, signerless_client_with_options};
    use crate::identity::RadrootsIdentity;
    use tokio::runtime::Runtime;

    #[test]
    fn client_constructors_build_without_runtime_net() {
        let identity = RadrootsIdentity::generate();
        let _client = client_from_identity(&identity);
        let _signerless = signerless_client();
        let _signerless_with_options =
            signerless_client_with_options(super::RelayClientOptions::new())
                .expect("signerless client with options");
    }

    #[test]
    fn signerless_client_has_no_signer() {
        let runtime = Runtime::new().expect("tokio runtime");
        runtime.block_on(async {
            let client = signerless_client();
            assert!(!client.has_signer().await);
        });
    }
}
