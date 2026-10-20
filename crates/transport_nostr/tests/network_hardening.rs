use radroots_transport::{Target, TargetSet, source::FetchBounds};
use radroots_transport_nostr::{
    RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrl, RelayUrlPolicy,
};

const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");
const RELAY_SOURCE: &str = include_str!("../src/relay.rs");
const CLIENT_SOURCE: &str = include_str!("../src/client.rs");

#[test]
fn tls_verification_and_pinned_dns_are_non_configurable_live_defaults() {
    assert!(WORKSPACE_MANIFEST.contains("rustls-tls-webpki-roots"));
    let socket = include_str!("../src/session_socket.rs");
    let native = include_str!("../../transport_nostr_native/src/connector.rs");
    for required in [
        "client_async_with_config(",
        "roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned())",
        ".with_no_client_auth()",
        "rustls::pki_types::ServerName::try_from(host.to_owned())",
        "MeteredIo::new(plaintext, metering, max_message_bytes.get())",
        ".max_message_size(Some(max_message_bytes.get()))",
        ".max_frame_size(Some(max_message_bytes.get()))",
        "admission.permits_address(address.ip())",
        "connect_pinned(&addresses)",
    ] {
        assert!(
            native.contains(required),
            "missing native policy witness `{required}`"
        );
    }
    assert!(socket.contains("if proxy.is_some()"));
    for forbidden in [
        "tokio::net::",
        "tokio_tungstenite",
        "TlsConnector",
        "lookup_host",
        "MeteredIo",
    ] {
        assert!(!RELAY_SOURCE.contains(forbidden));
        assert!(!socket.contains(forbidden));
    }
    for forbidden in [
        "danger_accept_invalid_certs",
        "danger_accept_invalid_hostnames",
        "NoCertificateVerification",
    ] {
        assert!(!RELAY_SOURCE.contains(forbidden));
        assert!(!CLIENT_SOURCE.contains(forbidden));
        assert!(!native.contains(forbidden));
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

    let relays = (0..=64)
        .map(|index| {
            RelayEndpoint::new(
                format!("wss://r{index}.example.com"),
                RelayUrlPolicy::Public,
                RelayAccess::ReadWrite,
            )
            .expect("relay endpoint")
        })
        .collect::<Vec<_>>();
    assert!(RelayProfile::explicit(RelayProfileKind::Public, relays).is_err());
}

#[test]
fn oversized_wire_payload_is_rejected_before_delivery() {
    let oversized = "x".repeat(radroots_event_codec::decode::MAX_EVENT_JSON_BYTES + 1);
    assert!(radroots_event_codec::decode::signed_event(oversized.as_str()).is_err());
}
