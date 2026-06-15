#![cfg(feature = "runtime")]

use radroots_sdk::{
    RadrootsSdk, RadrootsSdkClock, RadrootsSdkError, RadrootsSdkRecoveryAction,
    RadrootsSdkStorageConfig, RadrootsSdkTimestamp,
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
