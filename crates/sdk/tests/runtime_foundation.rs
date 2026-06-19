#![cfg(feature = "runtime")]

use radroots_sdk::{
    BackupRequest, IntegrityRequest, LISTING_PUBLISH_OPERATION_KIND, RadrootsSdk, RadrootsSdkClock,
    RadrootsSdkError, RadrootsSdkErrorClass, RadrootsSdkRecoveryAction, RadrootsSdkStorageConfig,
    RadrootsSdkTimestamp, RestoreRequest, SDK_IDEMPOTENCY_KEY_MAX_LEN, SDK_RELAY_TARGET_MAX_COUNT,
    SdkBackupState, SdkBackupVerification, SdkEventStoreStorageStatus, SdkIdempotencyKey,
    SdkOutboxStorageStatus, SdkRelayTargetPolicy, SdkRelayTargetSet, SdkRelayUrlPolicy,
    SdkRestoreState, SdkSqliteStoreStatus, SdkStorageKind, StorageStatusReceipt,
    StorageStatusRequest,
};
use std::path::PathBuf;

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
            "wss://relay-b.example.com".to_owned(),
            "wss://relay-a.example.com".to_owned()
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
        ["wss://user:password@relay.example.com/path?token=secret#frag"],
        SdkRelayUrlPolicy::Public,
    )
    .expect_err("invalid relay");
    let message = error.to_string();
    let detail = error.detail_json();

    assert!(matches!(error, RadrootsSdkError::InvalidRelayUrl { .. }));
    assert_eq!(error.code(), "invalid_relay_url");
    assert_eq!(error.class(), RadrootsSdkErrorClass::Configuration);
    assert!(!error.retryable());
    assert_eq!(
        error.recovery_actions(),
        vec![RadrootsSdkRecoveryAction::ConfigureRelayTargets]
    );
    assert!(message.contains("<redacted>@relay.example.com/path?<redacted>"));
    assert!(!message.contains("password"));
    assert!(!message.contains("token=secret"));
    assert!(!message.contains("frag"));
    assert_eq!(detail["code"], "invalid_relay_url");
    assert_eq!(detail["class"], "configuration");
    assert_eq!(detail["retryable"], false);
    assert_eq!(detail["recovery_actions"][0], "configure_relay_targets");
    assert!(!detail.to_string().contains("password"));
    assert!(!detail.to_string().contains("token=secret"));
    assert!(!detail.to_string().contains("frag"));
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

    let result = RadrootsSdk::builder()
        .relay_url_policy(SdkRelayUrlPolicy::Localhost)
        .relay_url("ws://192.168.1.10:8080")
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
async fn sdk_memory_storage_status_and_integrity_report_canonical_stores() {
    let sdk = RadrootsSdk::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .build()
        .await
        .expect("sdk");

    let status = sdk
        .storage_status(StorageStatusRequest::new())
        .await
        .expect("status");
    assert_eq!(status.storage, SdkStorageKind::Memory);
    assert_eq!(status.paths, None);
    assert_eq!(status.event_store.store.schema_version, 1);
    assert_eq!(status.outbox.store.schema_version, 1);
    assert!(status.event_store.store.foreign_keys_enabled);
    assert!(status.outbox.store.foreign_keys_enabled);
    assert_eq!(status.event_store.total_events, 0);
    assert_eq!(status.outbox.total_events, 0);
    assert_eq!(status.outbox.failed_terminal_events, 0);
    assert!(status.event_store.store.integrity_ok);
    assert!(status.outbox.store.integrity_ok);

    let integrity = sdk
        .integrity(IntegrityRequest::new())
        .await
        .expect("integrity");
    assert!(integrity.checked_paths.is_empty());
    assert!(integrity.event_store_ok);
    assert!(integrity.outbox_ok);
    assert_eq!(integrity.event_store_result, "ok");
    assert_eq!(integrity.outbox_result, "ok");
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
        LISTING_PUBLISH_OPERATION_KIND,
        "abcdef123456",
    );
    let message = error.to_string();

    assert!(message.contains(LISTING_PUBLISH_OPERATION_KIND));
    assert!(message.contains("abcdef123456"));
    assert!(message.contains("stored=true"));
    assert!(message.contains("queued=false"));
    assert!(!message.contains("sig"));
    assert!(!message.contains("raw"));
    assert!(!message.contains("idempotency-key"));
}

#[test]
fn sdk_error_contract_methods_cover_all_variants() {
    let cases = vec![
        (
            RadrootsSdkError::Io {
                path: PathBuf::from("store.sqlite"),
                message: "permission denied".to_owned(),
            },
            "io",
            RadrootsSdkErrorClass::Storage,
            true,
            vec![RadrootsSdkRecoveryAction::InspectLocalStores],
        ),
        (
            RadrootsSdkError::ClockBeforeUnixEpoch,
            "clock_before_unix_epoch",
            RadrootsSdkErrorClass::Clock,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::TimestampOutOfRange { value: u64::MAX },
            "timestamp_out_of_range",
            RadrootsSdkErrorClass::Clock,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::UnauthorizedActor {
                operation: "listing.prepare_publish".to_owned(),
                reason: "missing role".to_owned(),
            },
            "unauthorized_actor",
            RadrootsSdkErrorClass::Authorization,
            false,
            vec![RadrootsSdkRecoveryAction::SelectAuthorizedActor],
        ),
        (
            RadrootsSdkError::SignerPubkeyMismatch {
                operation: "event signing".to_owned(),
                expected_pubkey_prefix: "aaaaaaaaaaaa".to_owned(),
                signer_pubkey_prefix: "bbbbbbbbbbbb".to_owned(),
            },
            "signer_pubkey_mismatch",
            RadrootsSdkErrorClass::Authorization,
            false,
            vec![RadrootsSdkRecoveryAction::SelectAuthorizedActor],
        ),
        (
            RadrootsSdkError::EmptyTargetRelays {
                operation: "listing.publish".to_owned(),
            },
            "empty_target_relays",
            RadrootsSdkErrorClass::Configuration,
            false,
            vec![RadrootsSdkRecoveryAction::ConfigureRelayTargets],
        ),
        (
            RadrootsSdkError::RelayTargetLimitExceeded {
                max: 20,
                actual: 21,
            },
            "relay_target_limit_exceeded",
            RadrootsSdkErrorClass::Configuration,
            false,
            vec![RadrootsSdkRecoveryAction::ConfigureRelayTargets],
        ),
        (
            SdkRelayTargetSet::new(["wss://u:p@relay.example.com"], SdkRelayUrlPolicy::Public)
                .expect_err("invalid relay"),
            "invalid_relay_url",
            RadrootsSdkErrorClass::Configuration,
            false,
            vec![RadrootsSdkRecoveryAction::ConfigureRelayTargets],
        ),
        (
            RadrootsSdkError::IdempotencyConflict {
                operation_kind: LISTING_PUBLISH_OPERATION_KIND.to_owned(),
                expected_pubkey_prefix: "aaaaaaaaaaaa".to_owned(),
                existing_digest_prefix: "bbbbbbbbbbbb".to_owned(),
                new_digest_prefix: "cccccccccccc".to_owned(),
            },
            "idempotency_conflict",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey],
        ),
        (
            RadrootsSdkError::OrderStatusLimitInvalid {
                limit: 0,
                min: 1,
                max: 1000,
            },
            "order_status_limit_invalid",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::InvalidOrderId {
                value: "bad".to_owned(),
                message: "invalid".to_owned(),
            },
            "invalid_order_id",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::ProductSyncUnsupported {
                operation: "sync.push_outbox",
                required_feature: "relay-runtime",
            },
            "product_sync_unsupported",
            RadrootsSdkErrorClass::Unsupported,
            false,
            vec![RadrootsSdkRecoveryAction::EnableRequiredFeature],
        ),
        (
            RadrootsSdkError::ProductSyncRelaySetupFailure {
                message: "relay setup".to_owned(),
            },
            "product_sync_relay_setup_failure",
            RadrootsSdkErrorClass::Transport,
            true,
            vec![RadrootsSdkRecoveryAction::RetryAfterTransportFailure],
        ),
        (
            RadrootsSdkError::Authority {
                message: "authority".to_owned(),
            },
            "authority",
            RadrootsSdkErrorClass::Authorization,
            false,
            vec![RadrootsSdkRecoveryAction::SelectAuthorizedActor],
        ),
        (
            RadrootsSdkError::EventStore {
                message: "store".to_owned(),
            },
            "event_store",
            RadrootsSdkErrorClass::Storage,
            true,
            vec![RadrootsSdkRecoveryAction::InspectLocalStores],
        ),
        (
            RadrootsSdkError::InvalidRequest {
                message: "bad input".to_owned(),
            },
            "invalid_request",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::ListingDraft {
                message: "draft".to_owned(),
            },
            "listing_draft",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::ListingMutation {
                message: "mutation".to_owned(),
            },
            "listing_mutation",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::Outbox {
                message: "outbox".to_owned(),
            },
            "outbox",
            RadrootsSdkErrorClass::Storage,
            true,
            vec![RadrootsSdkRecoveryAction::InspectLocalStores],
        ),
        (
            RadrootsSdkError::RelayTransport {
                message: "relay".to_owned(),
            },
            "relay_transport",
            RadrootsSdkErrorClass::Transport,
            true,
            vec![RadrootsSdkRecoveryAction::RetryAfterTransportFailure],
        ),
        (
            RadrootsSdkError::Projection {
                message: "projection".to_owned(),
            },
            "projection",
            RadrootsSdkErrorClass::Storage,
            true,
            vec![RadrootsSdkRecoveryAction::InspectLocalStores],
        ),
        (
            RadrootsSdkError::partial_outbox_enqueue_mutation(
                "a".repeat(64),
                LISTING_PUBLISH_OPERATION_KIND,
                "abcdef123456",
            ),
            "partial_local_mutation",
            RadrootsSdkErrorClass::LocalMutation,
            true,
            vec![RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey],
        ),
    ];

    for (error, code, class, retryable, recovery_actions) in cases {
        assert_eq!(error.code(), code);
        assert_eq!(error.class(), class);
        assert_eq!(error.retryable(), retryable);
        assert_eq!(error.recovery_actions(), recovery_actions);
        let detail = error.detail_json();
        assert_eq!(detail["code"], code);
        assert_eq!(
            detail["class"],
            serde_json::to_value(class).expect("class json")
        );
        assert_eq!(detail["retryable"], retryable);
        assert_eq!(
            detail["recovery_actions"],
            serde_json::to_value(&recovery_actions).expect("recovery actions json")
        );
        assert!(detail["message"].is_string());
        assert!(detail["detail"].is_object());
    }
}

#[test]
fn relay_target_set_validates_normalizes_dedupes_preserves_order_and_caps() {
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
            "wss://relay-b.example.com".to_owned(),
            "wss://relay-a.example.com".to_owned()
        ]
    );
    assert_eq!(
        targets.canonical_relays(),
        &[
            "wss://relay-a.example.com".to_owned(),
            "wss://relay-b.example.com".to_owned()
        ]
    );
    assert_eq!(
        serde_json::to_value(SdkRelayTargetPolicy::explicit(targets.clone()))
            .expect("relay target policy json"),
        serde_json::json!({
            "kind": "explicit",
            "relays": ["wss://relay-b.example.com", "wss://relay-a.example.com"],
            "canonical_relays": ["wss://relay-a.example.com", "wss://relay-b.example.com"]
        })
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
    let key = SdkIdempotencyKey::new("idem-a").expect("key");
    assert_eq!(key.as_str(), "idem-a");
    let debug = format!("{key:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("idem-a"));
    assert_eq!(
        serde_json::to_value(&key).expect("key json"),
        serde_json::json!({ "value": "<redacted>", "len": 6 })
    );

    assert!(matches!(
        SdkIdempotencyKey::new(" "),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    let untrimmed = SdkIdempotencyKey::new(" idem-a ").expect_err("untrimmed");
    assert!(matches!(
        untrimmed,
        RadrootsSdkError::InvalidRequest { ref message }
            if message == "idempotency key must not include boundary whitespace"
    ));
    assert!(!untrimmed.to_string().contains("idem-a"));
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
fn storage_backup_and_integrity_contract_dtos_serialize() {
    let store = SdkSqliteStoreStatus {
        schema_version: 1,
        journal_mode: "wal".to_owned(),
        foreign_keys_enabled: true,
        busy_timeout_ms: 5_000,
        integrity_ok: true,
        integrity_result: "ok".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(StorageStatusRequest::new()).expect("status request"),
        serde_json::json!({})
    );
    assert_eq!(
        serde_json::to_value(StorageStatusReceipt {
            storage: SdkStorageKind::Directory,
            paths: None,
            event_store: SdkEventStoreStorageStatus {
                store: store.clone(),
                total_events: 2,
                projection_eligible_events: 1,
                relay_observations: 1,
                last_event_seq: Some(2),
                last_event_updated_at_ms: Some(1_700_000_000_000),
            },
            outbox: SdkOutboxStorageStatus {
                store,
                total_events: 3,
                pending_events: 1,
                retryable_events: 1,
                terminal_events: 1,
                failed_terminal_events: 0,
                ready_signed_events: 1,
                publishing_events: 0,
                last_attempt_at_ms: Some(1_700_000_000_000),
                last_error: Some("relay publish incomplete".to_owned()),
            },
        })
        .expect("status receipt"),
        serde_json::json!({
            "storage": "directory",
            "paths": null,
            "event_store": {
                "store": {
                    "schema_version": 1,
                    "journal_mode": "wal",
                    "foreign_keys_enabled": true,
                    "busy_timeout_ms": 5000,
                    "integrity_ok": true,
                    "integrity_result": "ok"
                },
                "total_events": 2,
                "projection_eligible_events": 1,
                "relay_observations": 1,
                "last_event_seq": 2,
                "last_event_updated_at_ms": 1700000000000i64
            },
            "outbox": {
                "store": {
                    "schema_version": 1,
                    "journal_mode": "wal",
                    "foreign_keys_enabled": true,
                    "busy_timeout_ms": 5000,
                    "integrity_ok": true,
                    "integrity_result": "ok"
                },
                "total_events": 3,
                "pending_events": 1,
                "retryable_events": 1,
                "terminal_events": 1,
                "failed_terminal_events": 0,
                "ready_signed_events": 1,
                "publishing_events": 0,
                "last_attempt_at_ms": 1700000000000i64,
                "last_error": "relay publish incomplete"
            }
        })
    );
    assert_eq!(
        serde_json::to_value(BackupRequest::new("backup")).expect("backup request"),
        serde_json::json!({
            "destination": "backup",
            "overwrite": false
        })
    );
    assert_eq!(
        serde_json::to_value(SdkBackupState::Completed).expect("backup state"),
        serde_json::json!("completed")
    );
    assert_eq!(
        serde_json::to_value(
            RestoreRequest::new("backup")
                .with_destination("sdk-runtime")
                .with_overwrite(true)
                .with_dry_run(true)
        )
        .expect("restore request"),
        serde_json::json!({
            "source": "backup",
            "destination": "sdk-runtime",
            "overwrite": true,
            "dry_run": true
        })
    );
    assert_eq!(
        serde_json::to_value(SdkRestoreState::Validated).expect("restore state"),
        serde_json::json!("validated")
    );
    assert_eq!(
        serde_json::to_value(SdkBackupVerification {
            event_store_ok: true,
            outbox_ok: true,
            event_store_events: 2,
            outbox_events: 3,
        })
        .expect("backup verification"),
        serde_json::json!({
            "event_store_ok": true,
            "outbox_ok": true,
            "event_store_events": 2,
            "outbox_events": 3
        })
    );
    assert_eq!(
        serde_json::to_value(IntegrityRequest::new()).expect("integrity request"),
        serde_json::json!({})
    );
}

#[test]
fn outbox_idempotency_conflict_maps_to_structured_sdk_error() {
    let error = RadrootsSdkError::from(radroots_outbox::RadrootsOutboxError::IdempotencyConflict {
        operation_kind: LISTING_PUBLISH_OPERATION_KIND.to_owned(),
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
        } if operation_kind == LISTING_PUBLISH_OPERATION_KIND
            && expected_pubkey_prefix == "aaaaaaaaaaaa"
            && existing_digest_prefix == "bbbbbbbbbbbb"
            && new_digest_prefix == "cccccccccccc"
    ));
    assert!(!message.contains("secret-idempotency-key"));
    assert!(!message.contains(&"b".repeat(64)));
    assert!(!message.contains(&"c".repeat(64)));
}

#[test]
fn sdk_examples_stay_on_product_api_boundary() {
    let examples = [
        (
            "runtime_local",
            include_str!("../examples/runtime_local.rs"),
        ),
        (
            "sdk_v1_listing_prepare",
            include_str!("../examples/sdk_v1_listing_prepare.rs"),
        ),
        (
            "sdk_v1_local_enqueue_and_mock_sync",
            include_str!("../examples/sdk_v1_local_enqueue_and_mock_sync.rs"),
        ),
    ];

    for (name, example) in examples {
        assert!(!example.contains("WireEventParts"), "{name}");
        assert!(!example.contains("protocol::wire"), "{name}");
        assert!(!example.contains("events_codec::wire"), "{name}");
        assert!(!example.contains(".as_wire_parts("), "{name}");
        assert!(!example.contains(".into_wire_parts("), "{name}");
    }

    let listing_prepare = include_str!("../examples/sdk_v1_listing_prepare.rs");
    assert!(listing_prepare.contains("RadrootsSdk::builder()"));
    assert!(listing_prepare.contains("ListingPreparePublishRequest"));
    assert!(listing_prepare.contains("prepare_publish"));

    let local_enqueue = include_str!("../examples/sdk_v1_local_enqueue_and_mock_sync.rs");
    assert!(local_enqueue.contains("RadrootsSdk::builder()"));
    assert!(local_enqueue.contains("ListingPreparePublishRequest"));
    assert!(local_enqueue.contains("SdkRelayTargetPolicy"));
    assert!(local_enqueue.contains("SdkRelayTargetSet"));
    assert!(local_enqueue.contains("SdkRelayUrlPolicy::Localhost"));
    assert!(local_enqueue.contains("enqueue_prepared_publish"));
    assert!(local_enqueue.contains("push_outbox_with_adapter"));
    assert!(local_enqueue.contains("OrderStatusRequest"));
}
