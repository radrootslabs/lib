use radroots_sdk::{
    NetworkConfig, RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT, RADROOTS_SDK_LOCAL_RELAY_URL,
    RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT, RADROOTS_SDK_PRODUCTION_RELAY_URL,
    RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT, RADROOTS_SDK_STAGING_RELAY_URL, RadrootsSdkConfig,
    RadrootsdAuth, SdkConfigError, SdkEnvironment, SdkTransportMode, SignerConfig,
};
use std::{
    ffi::OsString,
    sync::{Mutex, OnceLock},
};

const LOCAL_SDK_ENV_KEYS: &[&str] = &[
    "NOSTR_RS_RELAY_PUBLIC_SCHEME",
    "NOSTR_RS_RELAY_PUBLIC_HOST",
    "NOSTR_RS_RELAY_PUBLIC_PORT",
    "RADROOTSD_RPC_URL",
    "RADROOTSD_RPC_HOST",
    "RADROOTSD_RPC_PORT",
];

fn sdk_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct LocalSdkEnvRestore {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl LocalSdkEnvRestore {
    fn apply(pairs: &[(&str, &str)]) -> Self {
        let saved = LOCAL_SDK_ENV_KEYS
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();

        for key in LOCAL_SDK_ENV_KEYS {
            unsafe {
                std::env::remove_var(key);
            }
        }
        for (key, value) in pairs {
            assert!(
                LOCAL_SDK_ENV_KEYS.contains(key),
                "unexpected local sdk env key `{key}`"
            );
            unsafe {
                std::env::set_var(key, value);
            }
        }

        Self { saved }
    }
}

impl Drop for LocalSdkEnvRestore {
    fn drop(&mut self) {
        for (key, original) in self.saved.drain(..) {
            match original {
                Some(value) => unsafe {
                    std::env::set_var(key, value);
                },
                None => unsafe {
                    std::env::remove_var(key);
                },
            }
        }
    }
}

struct EnvKeyRestore {
    key: &'static str,
    saved: Option<OsString>,
}

impl EnvKeyRestore {
    fn capture(key: &'static str) -> Self {
        Self {
            key,
            saved: std::env::var_os(key),
        }
    }
}

impl Drop for EnvKeyRestore {
    fn drop(&mut self) {
        match &self.saved {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

fn with_local_sdk_env<F>(pairs: &[(&str, &str)], test: F)
where
    F: FnOnce(),
{
    let _guard = sdk_env_lock().lock().expect("sdk env lock");
    let _env_restore = LocalSdkEnvRestore::apply(pairs);

    test();
}

#[test]
fn local_sdk_env_restore_preserves_original_os_string_values() {
    let _guard = sdk_env_lock().lock().expect("sdk env lock");
    let key = "NOSTR_RS_RELAY_PUBLIC_HOST";
    let _restore_key = EnvKeyRestore::capture(key);
    let original = OsString::from("relay.before.example");

    unsafe {
        std::env::set_var(key, &original);
    }

    {
        let _env_restore = LocalSdkEnvRestore::apply(&[("RADROOTSD_RPC_PORT", "18080")]);

        assert_eq!(std::env::var_os(key), None);
    }

    assert_eq!(std::env::var_os(key), Some(original));
}

#[cfg(unix)]
#[test]
fn local_sdk_env_restore_preserves_non_unicode_original_values() {
    use std::os::unix::ffi::OsStringExt;

    let _guard = sdk_env_lock().lock().expect("sdk env lock");
    let key = "NOSTR_RS_RELAY_PUBLIC_HOST";
    let _restore_key = EnvKeyRestore::capture(key);
    let original = OsString::from_vec(vec![b'r', b'e', b'l', b'a', b'y', 0x80]);

    unsafe {
        std::env::set_var(key, &original);
    }

    {
        let _env_restore = LocalSdkEnvRestore::apply(&[("RADROOTSD_RPC_PORT", "18080")]);

        assert_eq!(std::env::var_os(key), None);
    }

    assert_eq!(std::env::var_os(key), Some(original));
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
    with_local_sdk_env(&[], || {
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
    });
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
fn invalid_relay_authorities_fail_loudly() {
    let invalid_relays = [
        "wss://",
        "wss:///relay",
        "ws://:8080",
        "wss://relay example",
        "wss://user@relay.example",
        "wss://relay.example:abc",
        "wss://2001:db8::1",
    ];

    for relay_url in invalid_relays {
        let mut config = RadrootsSdkConfig::production();
        config.relay.urls = vec![relay_url.to_owned()];

        assert_eq!(
            config
                .resolved_relay_urls()
                .expect_err("relay authority error"),
            SdkConfigError::InvalidRelayUrl(relay_url.to_owned())
        );
    }
}

#[test]
fn valid_relay_authorities_still_resolve() {
    let mut config = RadrootsSdkConfig::production();
    config.relay.urls = vec![
        " wss://relay.example/nostr ".to_owned(),
        "ws://127.0.0.1:8080".to_owned(),
        "wss://[2001:db8::1]:443/relay".to_owned(),
    ];

    assert_eq!(
        config.resolved_relay_urls().expect("valid relays"),
        vec![
            "wss://relay.example/nostr".to_owned(),
            "ws://127.0.0.1:8080".to_owned(),
            "wss://[2001:db8::1]:443/relay".to_owned()
        ]
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
