#![cfg(feature = "runtime")]

use radroots_sdk::{
    RadrootsSdk, RadrootsSdkClock, RadrootsSdkError, RadrootsSdkRecoveryAction,
    RadrootsSdkStorageConfig, RadrootsSdkTimestamp, SDK_IDEMPOTENCY_KEY_MAX_LEN,
    SDK_RELAY_TARGET_MAX_COUNT, SdkIdempotencyKey, SdkRelayTargetPolicy, SdkRelayTargetSet,
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
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
}

#[tokio::test]
async fn sdk_builder_allows_only_local_ws_targets_with_localhost_policy() {
    let sdk = RadrootsSdk::builder()
        .relay_target_policy(SdkRelayTargetPolicy::Localhost)
        .relay_url("ws://localhost:8080")
        .relay_url("ws://127.0.0.1:8081")
        .relay_url("ws://[::1]:8082")
        .build()
        .await
        .expect("sdk");

    assert_eq!(sdk.relay_urls().len(), 3);

    let result = RadrootsSdk::builder()
        .relay_target_policy(SdkRelayTargetPolicy::Localhost)
        .relay_url("ws://relay.example.com")
        .build()
        .await;

    assert!(matches!(
        result,
        Err(RadrootsSdkError::InvalidRequest { .. })
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
    let error = RadrootsSdkError::partial_local_mutation(
        true,
        false,
        RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey,
    );
    let message = error.to_string();

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
        SdkRelayTargetPolicy::Public,
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
        SdkRelayTargetSet::new(Vec::<String>::new(), SdkRelayTargetPolicy::Public),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let too_many = (0..=SDK_RELAY_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    assert!(matches!(
        SdkRelayTargetSet::new(too_many, SdkRelayTargetPolicy::Public),
        Err(RadrootsSdkError::InvalidRequest { .. })
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
