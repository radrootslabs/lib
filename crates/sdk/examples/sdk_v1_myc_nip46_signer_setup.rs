use nostr::Keys as RadrootsNostrKeys;
use radroots_event::envelope::kind::KIND_TRADE_PROPOSAL;
use radroots_nostr_connect::{
    client::{CancellationToken, Client, ClientEvent, Receive, Target, Transport, TransportFuture},
    uri::RelayUrl as ConnectRelayUrl,
};
use radroots_sdk::{
    RadrootsClient, RadrootsSdkMycNip46Signer, RadrootsSdkSignerMode, RadrootsSdkSignerProvider,
    radroots_sdk_myc_nip46_product_permission_strings,
};

struct ExampleNip46Transport;

impl Transport for ExampleNip46Transport {
    fn publish<'a>(&'a mut self, _event: ClientEvent) -> TransportFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn receive<'a>(
        &'a mut self,
        _cancellation: &'a CancellationToken,
    ) -> TransportFuture<'a, Receive> {
        Box::pin(async { Ok(Receive::TimedOut) })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let remote_signer_keys = RadrootsNostrKeys::generate();
    let user_keys = RadrootsNostrKeys::generate();
    let remote_signer_public_key =
        radroots_nostr::key::public_key_from_nostr(remote_signer_keys.public_key())?;
    let target = Target::try_new(
        remote_signer_public_key,
        vec![ConnectRelayUrl::parse("wss://relay.example.com")?],
    )?;
    let client = Client::generate(target)?;
    let signer = RadrootsSdkMycNip46Signer::from_client(
        client,
        user_keys.public_key().to_hex(),
        ExampleNip46Transport,
    )?;
    let sdk = RadrootsClient::builder()
        .signer_provider(RadrootsSdkSignerProvider::MycNip46(Box::new(signer)))
        .build()
        .await?;
    let status = sdk.signer_status().expect("configured signer status");
    let permissions = radroots_sdk_myc_nip46_product_permission_strings();

    assert_eq!(status.mode, RadrootsSdkSignerMode::MycNip46);
    assert!(permissions.iter().any(|value| value == "sign_event:30340"));
    assert!(
        permissions
            .iter()
            .any(|value| value == &format!("sign_event:{KIND_TRADE_PROPOSAL}"))
    );
    assert!(!permissions.iter().any(|value| value == "sign_event:3422"));
    println!("configured signer mode: {}", status.mode.as_str());
    println!("requested permissions: {}", permissions.join(","));
    Ok(())
}
