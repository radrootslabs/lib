use radroots_sdk::{
    NetworkConfig, RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT, RADROOTS_SDK_LOCAL_RELAY_URL,
    RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT, RADROOTS_SDK_PRODUCTION_RELAY_URL,
    RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT, RADROOTS_SDK_STAGING_RELAY_URL, RadrootsSdkConfig,
    RadrootsdAuth, SdkConfigError, SdkEnvironment, SdkTransportMode, SignerConfig,
};
use std::sync::{Mutex, OnceLock};

fn sdk_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_local_sdk_env<F>(pairs: &[(&str, &str)], test: F)
where
    F: FnOnce(),
{
    let _guard = sdk_env_lock().lock().expect("sdk env lock");
    let saved = pairs
        .iter()
        .map(|(key, _)| (key.to_string(), std::env::var(key).ok()))
        .collect::<Vec<_>>();

    for (key, value) in pairs {
        // The global lock keeps env mutation single-threaded for this test file.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    test();

    for (key, original) in saved {
        match original {
            Some(value) => unsafe {
                std::env::set_var(&key, value);
            },
            None => unsafe {
                std::env::remove_var(&key);
            },
        }
    }
}

#[test]
fn default_config_uses_production_relay_direct_draft_only() {
    let config = RadrootsSdkConfig::default();

    assert_eq!(config.environment, SdkEnvironment::Production);
    assert_eq!(config.transport, SdkTransportMode::RelayDirect);
    assert_eq!(config.signer, SignerConfig::DraftOnly);
    assert_eq!(config.network, NetworkConfig::default());
    assert_eq!(config.radrootsd.auth, RadrootsdAuth::None);
}

#[test]
fn production_environment_resolves_radroots_org_defaults() {
    let config = RadrootsSdkConfig::production();

    assert_eq!(
        config.resolved_relay_urls().expect("relay defaults"),
        vec![RADROOTS_SDK_PRODUCTION_RELAY_URL.to_owned()]
    );
    assert_eq!(
        config
            .resolved_radrootsd_endpoint()
            .expect("radrootsd endpoint"),
        RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT
    );
}

#[test]
fn staging_environment_resolves_staging_defaults() {
    let config = RadrootsSdkConfig::staging();

    assert_eq!(
        config.resolved_relay_urls().expect("relay defaults"),
        vec![RADROOTS_SDK_STAGING_RELAY_URL.to_owned()]
    );
    assert_eq!(
        config
            .resolved_radrootsd_endpoint()
            .expect("radrootsd endpoint"),
        RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT
    );
}

#[test]
fn local_environment_resolves_localhost_defaults() {
    let config = RadrootsSdkConfig::local();

    assert_eq!(
        config.resolved_relay_urls().expect("relay defaults"),
        vec![RADROOTS_SDK_LOCAL_RELAY_URL.to_owned()]
    );
    assert_eq!(
        config
            .resolved_radrootsd_endpoint()
            .expect("radrootsd endpoint"),
        RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT
    );
}

#[test]
fn local_environment_prefers_root_env_contract_when_present() {
    with_local_sdk_env(
        &[
            ("NOSTR_RS_RELAY_PUBLIC_SCHEME", "ws"),
            ("NOSTR_RS_RELAY_PUBLIC_HOST", "127.0.0.1"),
            ("NOSTR_RS_RELAY_PUBLIC_PORT", "18080"),
            ("RADROOTSD_RPC_URL", "http://127.0.0.1:17070/jsonrpc"),
        ],
        || {
            let config = RadrootsSdkConfig::local();

            assert_eq!(
                config.resolved_relay_urls().expect("relay defaults"),
                vec!["ws://127.0.0.1:18080".to_owned()]
            );
            assert_eq!(
                config
                    .resolved_radrootsd_endpoint()
                    .expect("radrootsd endpoint"),
                "http://127.0.0.1:17070/jsonrpc"
            );
        },
    );
}

#[test]
fn explicit_coordinates_override_environment_defaults_exactly() {
    let mut config = RadrootsSdkConfig::production();
    config.relay.urls = vec![
        " wss://relay.custom.one ".to_owned(),
        "wss://relay.custom.one".to_owned(),
        "ws://relay.custom.two".to_owned(),
    ];
    config.radrootsd.endpoint = Some(" https://rpc.custom.radroots.org ".to_owned());

    assert_eq!(
        config.resolved_relay_urls().expect("relay overrides"),
        vec![
            "wss://relay.custom.one".to_owned(),
            "ws://relay.custom.two".to_owned(),
        ]
    );
    assert_eq!(
        config
            .resolved_radrootsd_endpoint()
            .expect("endpoint override"),
        "https://rpc.custom.radroots.org"
    );
}

#[test]
fn custom_environment_requires_explicit_coordinates() {
    let config = RadrootsSdkConfig::custom();

    assert_eq!(
        config
            .resolved_relay_urls()
            .expect_err("custom relay error"),
        SdkConfigError::MissingCustomRelayUrls
    );
    assert_eq!(
        config
            .resolved_radrootsd_endpoint()
            .expect_err("custom radrootsd error"),
        SdkConfigError::MissingCustomRadrootsdEndpoint
    );
}

#[test]
fn custom_environment_accepts_explicit_coordinates() {
    let mut config = RadrootsSdkConfig::custom();
    config.relay.urls = vec!["wss://relay.custom.radroots.org".to_owned()];
    config.radrootsd.endpoint = Some("https://rpc.custom.radroots.org".to_owned());

    assert_eq!(
        config.resolved_relay_urls().expect("custom relay"),
        vec!["wss://relay.custom.radroots.org".to_owned()]
    );
    assert_eq!(
        config
            .resolved_radrootsd_endpoint()
            .expect("custom endpoint"),
        "https://rpc.custom.radroots.org"
    );
}

#[test]
fn invalid_coordinate_schemes_fail_loudly() {
    let mut config = RadrootsSdkConfig::production();
    config.relay.urls = vec!["https://relay.bad".to_owned()];
    config.radrootsd.endpoint = Some("wss://rpc.bad".to_owned());

    assert_eq!(
        config
            .resolved_relay_urls()
            .expect_err("relay scheme error"),
        SdkConfigError::InvalidRelayUrl("https://relay.bad".to_owned())
    );
    assert_eq!(
        config
            .resolved_radrootsd_endpoint()
            .expect_err("endpoint scheme error"),
        SdkConfigError::InvalidRadrootsdEndpoint("wss://rpc.bad".to_owned())
    );
}

#[test]
fn sdk_config_debug_redacts_bearer_tokens() {
    let mut config = RadrootsSdkConfig::production();
    config.radrootsd.auth = RadrootsdAuth::BearerToken("sdk-secret-token".to_owned());

    let debug = format!("{config:?}");

    assert!(!debug.contains("sdk-secret-token"));
    assert!(debug.contains("BearerToken(\"<redacted>\")"));
}
