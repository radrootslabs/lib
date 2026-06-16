#![cfg(feature = "runtime")]

use radroots_sdk::{
    RadrootsSdk, RadrootsSdkClock, RadrootsSdkError, RadrootsSdkStorageConfig,
    RadrootsSdkTimestamp, SDK_IDEMPOTENCY_KEY_MAX_LEN, SDK_RELAY_TARGET_MAX_COUNT,
    SdkIdempotencyKey, SdkRelayTargetSet, SdkRelayUrlPolicy,
};

#[tokio::test]
async fn sdk_builder_defaults_to_memory_storage_and_no_relays() {
    let sdk = RadrootsSdk::builder().build().await.expect("sdk");

    assert!(sdk.relay_urls().is_empty());
    assert!(sdk.storage_paths().is_none());
    let _listings = sdk.listings();
    let _orders = sdk.orders();
    let _sync = sdk.sync();
}

#[tokio::test]
async fn sdk_builder_validates_configured_relay_targets() {
    let sdk = RadrootsSdk::builder()
        .relay_url(" wss://relay-b.example.com/ ")
        .relay_url("wss://relay-a.example.com")
        .relay_url("wss://relay-a.example.com")
        .build()
        .await
        .expect("sdk");

    assert_eq!(
        sdk.relay_urls(),
        &[
            "wss://relay-a.example.com".to_owned(),
            "wss://relay-b.example.com".to_owned()
        ]
    );
}

#[tokio::test]
async fn sdk_builder_rejects_ws_relay_without_localhost_policy() {
    let result = RadrootsSdk::builder()
        .relay_url("ws://127.0.0.1:8080")
        .build()
        .await;

    assert!(matches!(
        result,
        Err(RadrootsSdkError::InvalidRelayUrl { .. })
    ));
}

#[test]
fn invalid_relay_url_errors_redact_userinfo() {
    let error = SdkRelayTargetSet::new(
        ["wss://user:password@relay.example.com"],
        SdkRelayUrlPolicy::Public,
    )
    .expect_err("invalid relay");
    let message = error.to_string();

    assert!(matches!(error, RadrootsSdkError::InvalidRelayUrl { .. }));
    assert!(message.contains("<redacted>@relay.example.com"));
    assert!(!message.contains("password"));
}

#[tokio::test]
async fn sdk_builder_allows_only_local_ws_targets_with_localhost_policy() {
    let sdk = RadrootsSdk::builder()
        .relay_url_policy(SdkRelayUrlPolicy::Localhost)
        .relay_url("ws://localhost:8080")
        .relay_url("ws://127.0.0.1:8081")
        .relay_url("ws://[::1]:8082")
        .build()
        .await
        .expect("sdk");

    assert_eq!(sdk.relay_urls().len(), 3);

    let result = RadrootsSdk::builder()
        .relay_url_policy(SdkRelayUrlPolicy::Localhost)
        .relay_url("ws://relay.example.com")
        .build()
        .await;

    assert!(matches!(
        result,
        Err(RadrootsSdkError::InvalidRelayUrl { .. })
    ));
}

#[tokio::test]
async fn sdk_directory_storage_creates_deterministic_sqlite_files() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsSdk::builder()
        .storage(RadrootsSdkStorageConfig::Directory(
            tempdir.path().join("sdk-runtime"),
        ))
        .build()
        .await
        .expect("sdk");

    let paths = sdk.storage_paths().expect("paths");
    assert_eq!(
        paths.event_store_path,
        tempdir
            .path()
            .join("sdk-runtime")
            .join("event_store.sqlite")
    );
    assert_eq!(
        paths.outbox_path,
        tempdir.path().join("sdk-runtime").join("outbox.sqlite")
    );
    assert!(paths.event_store_path.exists());
    assert!(paths.outbox_path.exists());
}

#[tokio::test]
async fn sdk_fixed_clock_is_used_by_runtime() {
    let timestamp = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000);
    let sdk = RadrootsSdk::builder()
        .clock(RadrootsSdkClock::Fixed(timestamp))
        .build()
        .await
        .expect("sdk");

    assert_eq!(sdk.now().expect("now"), timestamp);
}

#[test]
fn sdk_timestamp_rejects_values_outside_nostr_created_at_range() {
    let valid = RadrootsSdkTimestamp::from_unix_seconds(u64::from(u32::MAX));
    assert_eq!(valid.try_into_nostr_created_at().expect("valid"), u32::MAX);

    let invalid = RadrootsSdkTimestamp::from_unix_seconds(u64::from(u32::MAX) + 1);
    assert!(matches!(
        invalid.try_into_nostr_created_at(),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
}

#[test]
fn sdk_partial_local_mutation_error_is_sanitized() {
    let event_id = "a".repeat(64);
    let error = RadrootsSdkError::partial_outbox_enqueue_mutation(
        event_id,
        "listing.publish.v1",
        "abcdef123456",
    );
    let message = error.to_string();

    assert!(message.contains("listing.publish.v1"));
    assert!(message.contains("abcdef123456"));
    assert!(message.contains("stored=true"));
    assert!(message.contains("queued=false"));
    assert!(!message.contains("sig"));
    assert!(!message.contains("raw"));
    assert!(!message.contains("idempotency-key"));
}

#[test]
fn relay_target_set_validates_normalizes_dedupes_sorts_and_caps() {
    let targets = SdkRelayTargetSet::new(
        [
            " wss://relay-b.example.com/ ",
            "wss://relay-a.example.com",
            "wss://relay-a.example.com",
        ],
        SdkRelayUrlPolicy::Public,
    )
    .expect("targets");

    assert_eq!(
        targets.relays(),
        &[
            "wss://relay-a.example.com".to_owned(),
            "wss://relay-b.example.com".to_owned()
        ]
    );

    assert!(matches!(
        SdkRelayTargetSet::new(Vec::<String>::new(), SdkRelayUrlPolicy::Public),
        Err(RadrootsSdkError::EmptyTargetRelays { .. })
    ));

    let too_many = (0..=SDK_RELAY_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    assert!(matches!(
        SdkRelayTargetSet::new(too_many, SdkRelayUrlPolicy::Public),
        Err(RadrootsSdkError::RelayTargetLimitExceeded {
            max: SDK_RELAY_TARGET_MAX_COUNT,
            actual
        }) if actual == SDK_RELAY_TARGET_MAX_COUNT + 1
    ));
}

#[test]
fn idempotency_key_validation_is_bounded_and_debug_redacted() {
    let key = SdkIdempotencyKey::new(" idem-a ").expect("key");
    assert_eq!(key.as_str(), "idem-a");
    let debug = format!("{key:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("idem-a"));

    assert!(matches!(
        SdkIdempotencyKey::new(" "),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    assert!(matches!(
        SdkIdempotencyKey::new("idem\nbad"),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    assert!(matches!(
        SdkIdempotencyKey::new("x".repeat(SDK_IDEMPOTENCY_KEY_MAX_LEN + 1)),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
}

#[test]
fn outbox_idempotency_conflict_maps_to_structured_sdk_error() {
    let error = RadrootsSdkError::from(radroots_outbox::RadrootsOutboxError::IdempotencyConflict {
        operation_kind: "listing.publish.v1".to_owned(),
        expected_pubkey: "a".repeat(64),
        idempotency_key: "secret-idempotency-key".to_owned(),
        existing_digest: "b".repeat(64),
        new_digest: "c".repeat(64),
    });
    let message = error.to_string();

    assert!(matches!(
        error,
        RadrootsSdkError::IdempotencyConflict {
            operation_kind,
            expected_pubkey_prefix,
            existing_digest_prefix,
            new_digest_prefix,
        } if operation_kind == "listing.publish.v1"
            && expected_pubkey_prefix == "aaaaaaaaaaaa"
            && existing_digest_prefix == "bbbbbbbbbbbb"
            && new_digest_prefix == "cccccccccccc"
    ));
    assert!(!message.contains("secret-idempotency-key"));
    assert!(!message.contains(&"b".repeat(64)));
    assert!(!message.contains(&"c".repeat(64)));
}
