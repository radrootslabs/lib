use radroots_transport::{Target, TargetSet, source::FetchBounds};
use radroots_transport_nostr::{Config, RelayUrl, RelayUrlPolicy};

const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");
const RELAY_SOURCE: &str = include_str!("../src/relay.rs");
const CLIENT_SOURCE: &str = include_str!("../src/client.rs");

#[test]
fn tls_verification_and_pinned_dns_are_non_configurable_live_defaults() {
    assert!(WORKSPACE_MANIFEST.contains("rustls-tls-webpki-roots"));
    for required in [
        "client_async_tls(relay.as_str(), tcp)",
        "validate_resolved_addresses(",
        "connect_pinned(addresses.as_slice())",
        "ConnectionMode::Direct",
    ] {
        assert!(
            RELAY_SOURCE.contains(required),
            "missing hardening witness `{required}`"
        );
    }
    for forbidden in [
        "danger_accept_invalid_certs",
        "danger_accept_invalid_hostnames",
        "NoCertificateVerification",
    ] {
        assert!(!RELAY_SOURCE.contains(forbidden));
        assert!(!CLIENT_SOURCE.contains(forbidden));
    }

    assert!(RelayUrl::parse("ws://relay.example.com", RelayUrlPolicy::Public).is_err());
    assert!(RelayUrl::parse("wss://relay.example.com", RelayUrlPolicy::Public).is_ok());
    assert!(RelayUrl::parse("ws://127.0.0.1", RelayUrlPolicy::Local).is_ok());
}

#[test]
fn page_and_target_limits_reject_oversized_requests() {
    assert!(FetchBounds::new(1_001, u64::MAX).is_err());

    let targets = (0..=64)
        .map(|index| Target::nostr_relay(format!("wss://r{index}.example.com")).expect("target"))
        .collect::<Vec<_>>();
    assert!(TargetSet::new(targets).is_err());

    let relays = (0..=64).map(|index| format!("wss://r{index}.example.com"));
    assert!(Config::new(RelayUrlPolicy::Public, relays).is_err());
}

#[test]
fn oversized_wire_payload_is_rejected_before_delivery() {
    let oversized = "x".repeat(radroots_event_codec::decode::MAX_EVENT_JSON_BYTES + 1);
    assert!(radroots_event_codec::decode::signed_event(oversized.as_str()).is_err());
}
