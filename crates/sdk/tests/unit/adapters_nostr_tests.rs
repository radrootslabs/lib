use super::{
    client_from_keys, configure_write_relays, connected_client_from_keys, connected_relay_urls,
    publish_signed_event, signerless_client, signerless_client_with_options,
};
use core::time::Duration;
use nostr::{EventBuilder, Keys, Kind};
use radroots_transport_nostr::{RadrootsNostrClientOptions, RadrootsRelayTransportError};
use tokio::runtime::Runtime;

#[test]
fn client_constructors_build_without_runtime_net() {
    let keys = Keys::generate();
    let _client = client_from_keys(keys);
    let _signerless = signerless_client();
    let _signerless_with_options =
        signerless_client_with_options(RadrootsNostrClientOptions::new());
}

#[test]
fn signerless_client_has_no_signer() {
    let runtime = Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let client = signerless_client();
        assert!(!client.has_signer().await);
    });
}

#[test]
fn relay_helpers_accept_empty_relay_sets_without_network_endpoints() {
    let runtime = Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let keys = Keys::generate();
        let client = client_from_keys(keys.clone());

        configure_write_relays(&client, &[], Duration::from_millis(1))
            .await
            .expect("configure empty relays");
        assert_eq!(connected_relay_urls(&client).await, Vec::<String>::new());

        let invalid_relays = vec!["not-a-relay-url".to_owned()];
        let error = configure_write_relays(&client, &invalid_relays, Duration::from_millis(1))
            .await
            .expect_err("invalid relay");
        assert!(matches!(error, RadrootsRelayTransportError::Client(_)));
        let connected_error = match connected_client_from_keys(
            keys.clone(),
            &invalid_relays,
            Duration::from_millis(1),
        )
        .await
        {
            Ok(_) => panic!("expected invalid connected relay"),
            Err(error) => error,
        };
        assert!(matches!(
            connected_error,
            RadrootsRelayTransportError::Client(_)
        ));

        let disconnected = client_from_keys(keys.clone());
        disconnected
            .add_write_relay("wss://relay.example.com")
            .await
            .expect("add relay");
        assert_eq!(
            connected_relay_urls(&disconnected).await,
            Vec::<String>::new()
        );

        let connected = connected_client_from_keys(keys.clone(), &[], Duration::from_millis(1))
            .await
            .expect("connected client");
        assert_eq!(connected_relay_urls(&connected).await, Vec::<String>::new());

        // Relay publication consumes an already-signed transport fixture; it
        // does not expose an SDK event-authoring path.
        let signed = EventBuilder::new(Kind::Custom(30_001), "hello")
            .sign_with_keys(&keys)
            .expect("signed event");
        let error = publish_signed_event(&connected, &signed)
            .await
            .expect_err("publish without relays");
        assert!(matches!(error, RadrootsRelayTransportError::Client(_)));
    });
}
