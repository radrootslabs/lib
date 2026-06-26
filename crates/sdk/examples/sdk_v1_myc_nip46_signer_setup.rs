use radroots_nostr::prelude::{RadrootsNostrEvent, RadrootsNostrKeys};
use radroots_nostr_connect::prelude::{
    RadrootsNostrConnectClientTarget, RadrootsNostrConnectError,
};
use radroots_sdk::{
    RadrootsClient, RadrootsSdkMycNip46Signer, RadrootsSdkNip46Transport,
    RadrootsSdkNip46TransportFuture, RadrootsSdkSignerMode, RadrootsSdkSignerProvider,
    radroots_sdk_myc_nip46_product_permission_strings,
};
use std::sync::Arc;

struct ExampleNip46Transport;

impl RadrootsSdkNip46Transport for ExampleNip46Transport {
    fn publish_request_event<'a>(
        &'a self,
        _event: RadrootsNostrEvent,
    ) -> RadrootsSdkNip46TransportFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn next_response_event<'a>(
        &'a self,
    ) -> RadrootsSdkNip46TransportFuture<'a, RadrootsNostrEvent> {
        Box::pin(async { Err(RadrootsNostrConnectError::RequestTimedOut) })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client_keys = RadrootsNostrKeys::generate();
    let remote_signer_keys = RadrootsNostrKeys::generate();
    let user_keys = RadrootsNostrKeys::generate();
    let target = RadrootsNostrConnectClientTarget::new(
        remote_signer_keys.public_key(),
        vec![nostr::RelayUrl::parse("wss://relay.example.com")?],
    );
    let signer = RadrootsSdkMycNip46Signer::new(
        client_keys,
        target,
        user_keys.public_key().to_hex(),
        Arc::new(ExampleNip46Transport),
    )?;
    let sdk = RadrootsClient::builder()
        .signer_provider(RadrootsSdkSignerProvider::MycNip46(signer))
        .build()
        .await?;
    let status = sdk.signer_status().expect("configured signer status");
    let permissions = radroots_sdk_myc_nip46_product_permission_strings();

    assert_eq!(status.mode, RadrootsSdkSignerMode::MycNip46);
    assert!(permissions.iter().any(|value| value == "sign_event:30340"));
    println!("configured signer mode: {}", status.mode.as_str());
    println!("requested permissions: {}", permissions.join(","));
    Ok(())
}
