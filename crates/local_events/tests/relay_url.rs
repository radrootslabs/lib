use radroots_local_events::{RelayUrlValidationError, normalize_relay_url, normalize_relay_urls};

#[test]
fn relay_url_error_display_is_stable() {
    assert_eq!(
        RelayUrlValidationError::Empty.to_string(),
        "relay url must not be empty"
    );
    assert_eq!(
        RelayUrlValidationError::UnsupportedScheme("https://relay.example".to_owned()).to_string(),
        "relay url must use ws or wss, got `https://relay.example`"
    );
    assert_eq!(
        RelayUrlValidationError::MissingHost("wss://".to_owned()).to_string(),
        "relay url must include a host, got `wss://`"
    );
    assert_eq!(
        RelayUrlValidationError::InvalidAuthority("wss://user@relay.example".to_owned())
            .to_string(),
        "relay url authority is invalid, got `wss://user@relay.example`"
    );
    assert_eq!(
        RelayUrlValidationError::InvalidPort("wss://relay.example:abc".to_owned()).to_string(),
        "relay url port is invalid, got `wss://relay.example:abc`"
    );
}

#[test]
fn relay_url_normalization_trims_and_dedupes() {
    let relays = normalize_relay_urls([
        " wss://relay-a.example ",
        "wss://relay-a.example",
        "ws://127.0.0.1:8080/nostr",
    ])
    .expect("normalize relays");

    assert_eq!(
        relays,
        vec![
            "wss://relay-a.example".to_owned(),
            "ws://127.0.0.1:8080/nostr".to_owned()
        ]
    );
}

#[test]
fn relay_url_validation_rejects_empty_values() {
    assert_eq!(
        normalize_relay_url("   "),
        Err(RelayUrlValidationError::Empty)
    );
}

#[test]
fn relay_url_validation_rejects_non_websocket_schemes() {
    assert_eq!(
        normalize_relay_url("https://relay.example"),
        Err(RelayUrlValidationError::UnsupportedScheme(
            "https://relay.example".to_owned()
        ))
    );
}

#[test]
fn relay_url_validation_rejects_hostless_values() {
    assert_eq!(
        normalize_relay_url("wss://"),
        Err(RelayUrlValidationError::MissingHost("wss://".to_owned()))
    );
    assert_eq!(
        normalize_relay_url("wss:///relay"),
        Err(RelayUrlValidationError::MissingHost(
            "wss:///relay".to_owned()
        ))
    );
    assert_eq!(
        normalize_relay_url("ws://:8080"),
        Err(RelayUrlValidationError::MissingHost(
            "ws://:8080".to_owned()
        ))
    );
}

#[test]
fn relay_url_validation_rejects_malformed_authority() {
    assert_eq!(
        normalize_relay_url("wss://user@relay.example"),
        Err(RelayUrlValidationError::InvalidAuthority(
            "wss://user@relay.example".to_owned()
        ))
    );
    assert_eq!(
        normalize_relay_url("wss://relay example"),
        Err(RelayUrlValidationError::InvalidAuthority(
            "wss://relay example".to_owned()
        ))
    );
    assert_eq!(
        normalize_relay_url("wss://2001:db8::1"),
        Err(RelayUrlValidationError::InvalidAuthority(
            "wss://2001:db8::1".to_owned()
        ))
    );
    assert_eq!(
        normalize_relay_url("wss://[2001:db8::1"),
        Err(RelayUrlValidationError::InvalidAuthority(
            "wss://[2001:db8::1".to_owned()
        ))
    );
    assert_eq!(
        normalize_relay_url("wss://[]"),
        Err(RelayUrlValidationError::MissingHost("wss://[]".to_owned()))
    );
    assert_eq!(
        normalize_relay_url("wss://[2001:db8::1]extra"),
        Err(RelayUrlValidationError::InvalidAuthority(
            "wss://[2001:db8::1]extra".to_owned()
        ))
    );
}

#[test]
fn relay_url_validation_rejects_invalid_ports() {
    assert_eq!(
        normalize_relay_url("wss://relay.example:"),
        Err(RelayUrlValidationError::InvalidPort(
            "wss://relay.example:".to_owned()
        ))
    );
    assert_eq!(
        normalize_relay_url("wss://relay.example:abc"),
        Err(RelayUrlValidationError::InvalidPort(
            "wss://relay.example:abc".to_owned()
        ))
    );
    assert_eq!(
        normalize_relay_url("wss://[2001:db8::1]:abc"),
        Err(RelayUrlValidationError::InvalidPort(
            "wss://[2001:db8::1]:abc".to_owned()
        ))
    );
}

#[test]
fn relay_url_validation_accepts_bracketed_ipv6() {
    assert_eq!(
        normalize_relay_url("wss://[2001:db8::1]:8080/nostr").expect("ipv6 relay"),
        "wss://[2001:db8::1]:8080/nostr"
    );
    assert_eq!(
        normalize_relay_url("wss://[2001:db8::1]").expect("ipv6 relay without port"),
        "wss://[2001:db8::1]"
    );
}
