#![cfg(feature = "runtime")]

use radroots_event::ids::RadrootsOrderId;
use radroots_sdk::{
    BackupRequest, IntegrityRequest, LISTING_PUBLISH_OPERATION_KIND, NostrProfile,
    NostrRelayUrlPolicy, RadrootsClient, RadrootsSdkClock, RadrootsSdkError, RadrootsSdkErrorClass,
    RadrootsSdkGeoNamesErrorKind, RadrootsSdkRecoveryAction, RadrootsSdkStorageConfig,
    RadrootsSdkTimestamp, RestoreRequest, ReticulumPreviewBehavior, SDK_IDEMPOTENCY_KEY_MAX_LEN,
    SDK_TRANSPORT_TARGET_MAX_COUNT, SdkBackupState, SdkBackupVerification,
    SdkEventStoreStorageStatus, SdkIdempotencyKey, SdkOutboxStorageStatus,
    SdkPrivateStoreStorageStatus, SdkRestoreState, SdkSqliteStoreStatus,
    SdkSqliteWalCheckpointReceipt, SdkSqliteWalStatus, SdkStorageKind, StorageCheckpointReceipt,
    StorageCheckpointRequest, StorageStatusReceipt, StorageStatusRequest, TargetPolicy, TargetSet,
    TransportProfile,
};
use radroots_trade::identity::RadrootsTradeLocator;
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::{Path, PathBuf};

fn nostr_profile<I, S>(
    relays: I,
    policy: NostrRelayUrlPolicy,
) -> Result<TransportProfile, RadrootsSdkError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Ok(TransportProfile::nostr(NostrProfile::new(relays, policy)?))
}

#[tokio::test]
async fn sdk_builder_defaults_to_memory_storage_and_no_relays() {
    let sdk = RadrootsClient::builder().build().await.expect("sdk");

    assert!(sdk.configured_nostr_relay_urls().is_empty());
    assert!(sdk.storage_paths().is_none());
    let _listings = sdk.listings();
    let _market = sdk.market();
    let _geonames = sdk.geonames();
    let _trades = sdk.trades();
    let _sync = sdk.sync();
    let _dvm = sdk.dvm();
}

#[tokio::test]
async fn sdk_builder_validates_configured_relay_targets() {
    let sdk = RadrootsClient::builder()
        .transport_profile(
            nostr_profile(
                [" wss://relay-b.example.com/ ", "wss://relay-a.example.com"],
                NostrRelayUrlPolicy::Public,
            )
            .expect("profile"),
        )
        .build()
        .await
        .expect("sdk");

    assert_eq!(
        sdk.configured_nostr_relay_urls(),
        &[
            "wss://relay-b.example.com".to_owned(),
            "wss://relay-a.example.com".to_owned()
        ]
    );
}

#[tokio::test]
async fn sdk_builder_rejects_ws_relay_without_localhost_policy() {
    let result = nostr_profile(["ws://127.0.0.1:8080"], NostrRelayUrlPolicy::Public);

    match result {
        Err(RadrootsSdkError::InvalidRelayUrl { .. }) => {}
        Err(error) => panic!("unexpected profile error: {error}"),
        Ok(_) => panic!("profile accepted ws relay without localhost policy"),
    }
}

#[test]
fn invalid_relay_url_errors_redact_userinfo() {
    let error = TargetSet::nostr_relays(
        ["wss://user:password@relay.example.com/path?token=secret#frag"],
        NostrRelayUrlPolicy::Public,
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
        vec![RadrootsSdkRecoveryAction::ConfigureTransportTargets]
    );
    assert!(message.contains("<redacted>@relay.example.com/path?<redacted>"));
    assert!(!message.contains("password"));
    assert!(!message.contains("token=secret"));
    assert!(!message.contains("frag"));
    assert_eq!(detail["code"], "invalid_relay_url");
    assert_eq!(detail["class"], "configuration");
    assert_eq!(detail["retryable"], false);
    assert_eq!(detail["recovery_actions"][0], "configure_transport_targets");
    assert!(!detail.to_string().contains("password"));
    assert!(!detail.to_string().contains("token=secret"));
    assert!(!detail.to_string().contains("frag"));
}

#[tokio::test]
async fn sdk_builder_allows_only_local_ws_targets_with_localhost_policy() {
    let sdk = RadrootsClient::builder()
        .transport_profile(
            nostr_profile(
                [
                    "ws://localhost:8080",
                    "ws://127.0.0.1:8081",
                    "ws://[::1]:8082",
                ],
                NostrRelayUrlPolicy::Localhost,
            )
            .expect("profile"),
        )
        .build()
        .await
        .expect("sdk");

    assert_eq!(sdk.configured_nostr_relay_urls().len(), 3);

    let result = nostr_profile(["ws://relay.example.com"], NostrRelayUrlPolicy::Localhost);

    assert!(matches!(
        result,
        Err(RadrootsSdkError::InvalidRelayUrl { .. })
    ));

    let result = nostr_profile(["ws://192.168.1.10:8080"], NostrRelayUrlPolicy::Localhost);

    assert!(matches!(
        result,
        Err(RadrootsSdkError::InvalidRelayUrl { .. })
    ));
}

#[tokio::test]
async fn sdk_directory_storage_creates_deterministic_sqlite_files() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsClient::builder()
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
    assert_eq!(
        paths.private_store_path,
        tempdir.path().join("sdk-runtime").join("private.sqlite")
    );
    assert!(paths.event_store_path.exists());
    assert!(paths.outbox_path.exists());
    assert!(paths.private_store_path.exists());
    let event_tables = sqlite_table_names(&paths.event_store_path).await;
    assert!(event_tables.iter().any(|name| name == "event_envelopes"));
    assert!(
        event_tables
            .iter()
            .any(|name| name == "event_envelope_tags")
    );
    assert!(event_tables.iter().any(|name| name == "listing_projection"));
    assert!(event_tables.iter().any(|name| name == "trade_projection"));
    assert!(event_tables.iter().any(|name| name == "listing_search_fts"));
    assert!(!event_tables.iter().any(|name| name == "nostr_event"));
    assert!(!event_tables.iter().any(|name| name == "nostr_event_tag"));
    assert_eq!(
        sqlite_trade_projection_primary_key(&paths.event_store_path).await,
        vec!["order_id", "root_event_id", "projection_version"]
    );
    let outbox_tables = sqlite_table_names(&paths.outbox_path).await;
    assert!(outbox_tables.iter().any(|name| name == "outbox_operations"));
    assert!(!outbox_tables.iter().any(|name| name == "outbox_operation"));
    let private_tables = sqlite_table_names(&paths.private_store_path).await;
    assert!(
        private_tables
            .iter()
            .any(|name| name == "sdk_private_farm_location")
    );
}

#[tokio::test]
async fn sdk_memory_storage_status_and_integrity_report_canonical_stores() {
    let sdk = RadrootsClient::builder()
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
    assert_eq!(status.private_store.store.schema_version, 2);
    assert!(status.event_store.store.foreign_keys_enabled);
    assert!(status.outbox.store.foreign_keys_enabled);
    assert!(status.private_store.store.foreign_keys_enabled);
    assert_eq!(status.event_store.total_events, 0);
    assert_eq!(status.outbox.total_events, 0);
    assert_eq!(status.private_store.farm_private_locations, 0);
    assert_eq!(status.outbox.failed_terminal_events, 0);
    assert!(status.event_store.store.integrity_ok);
    assert!(status.outbox.store.integrity_ok);
    assert!(status.private_store.store.integrity_ok);

    let integrity = sdk
        .integrity(IntegrityRequest::new())
        .await
        .expect("integrity");
    assert!(integrity.checked_paths.is_empty());
    assert!(integrity.event_store_ok);
    assert!(integrity.outbox_ok);
    assert!(integrity.private_store_ok);
    assert_eq!(integrity.event_store_result, "ok");
    assert_eq!(integrity.outbox_result, "ok");
    assert_eq!(integrity.private_store_result, "ok");
}

#[tokio::test]
async fn sdk_fixed_clock_is_used_by_runtime() {
    let timestamp = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000);
    let sdk = RadrootsClient::builder()
        .clock(RadrootsSdkClock::Fixed(timestamp))
        .build()
        .await
        .expect("sdk");

    assert_eq!(sdk.now().expect("now"), timestamp);
}

#[tokio::test]
async fn runtime_defaults_and_clock_overflow_paths_are_explicit() {
    assert_eq!(
        RadrootsSdkStorageConfig::default(),
        RadrootsSdkStorageConfig::Memory
    );
    assert_eq!(RadrootsSdkClock::default(), RadrootsSdkClock::System);
    assert!(
        RadrootsSdkClock::default()
            .now()
            .expect("system clock")
            .unix_seconds()
            > 0
    );

    let overflow_sdk = RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX))
        .build()
        .await
        .expect("overflow sdk");
    assert!(matches!(
        overflow_sdk
            .storage_status(StorageStatusRequest::new())
            .await
            .expect_err("checked mul overflow"),
        RadrootsSdkError::TimestampOutOfRange { value } if value == u64::MAX
    ));

    let too_large_for_i64 = u64::try_from(i64::MAX).expect("i64 max") / 1_000 + 1;
    let i64_overflow_sdk = RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(too_large_for_i64))
        .build()
        .await
        .expect("i64 overflow sdk");
    assert!(matches!(
        i64_overflow_sdk
            .storage_status(StorageStatusRequest::new())
            .await
            .expect_err("i64 overflow"),
        RadrootsSdkError::TimestampOutOfRange { value } if value == too_large_for_i64
    ));
}

#[tokio::test]
async fn runtime_directory_storage_rejects_file_path() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let file_path = tempdir.path().join("sdk-file");
    std::fs::write(&file_path, b"not a directory").expect("file");

    let result = RadrootsClient::builder()
        .directory_storage(file_path.clone())
        .build()
        .await;

    assert!(matches!(
        result,
        Err(RadrootsSdkError::Io { path, .. }) if path == file_path
    ));
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
            RadrootsSdkError::EmptyTransportTargets {
                operation: "listing.publish".to_owned(),
            },
            "empty_transport_targets",
            RadrootsSdkErrorClass::Configuration,
            false,
            vec![RadrootsSdkRecoveryAction::ConfigureTransportTargets],
        ),
        (
            RadrootsSdkError::TransportTargetLimitExceeded {
                max: 20,
                actual: 21,
            },
            "transport_target_limit_exceeded",
            RadrootsSdkErrorClass::Configuration,
            false,
            vec![RadrootsSdkRecoveryAction::ConfigureTransportTargets],
        ),
        (
            TargetSet::nostr_relays(["wss://u:p@relay.example.com"], NostrRelayUrlPolicy::Public)
                .expect_err("invalid relay"),
            "invalid_relay_url",
            RadrootsSdkErrorClass::Configuration,
            false,
            vec![RadrootsSdkRecoveryAction::ConfigureTransportTargets],
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
            RadrootsSdkError::TradeStatusLimitInvalid {
                limit: 0,
                min: 1,
                max: 1000,
            },
            "trade_status_limit_invalid",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::InvalidTradeId {
                value: "bad".to_owned(),
                message: "invalid".to_owned(),
            },
            "invalid_trade_id",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::FixRequest],
        ),
        (
            RadrootsSdkError::TradeAmbiguous {
                operation: "trade.accept".to_owned(),
                locator: RadrootsTradeLocator::from_order_id(
                    RadrootsOrderId::parse("trade-error").expect("order id"),
                ),
                candidates: vec![RadrootsTradeLocator::from_order_id(
                    RadrootsOrderId::parse("trade-error").expect("order id"),
                )],
            },
            "trade_ambiguous",
            RadrootsSdkErrorClass::Request,
            false,
            vec![RadrootsSdkRecoveryAction::SelectTradeRoot],
        ),
        (
            RadrootsSdkError::ProductSyncUnsupported {
                operation: "sync.push_outbox",
                required_feature: "transport-nostr-runtime",
            },
            "product_sync_unsupported",
            RadrootsSdkErrorClass::Unsupported,
            false,
            vec![RadrootsSdkRecoveryAction::EnableRequiredFeature],
        ),
        (
            RadrootsSdkError::ReticulumPreviewTransportUnavailable {
                operation: "sync.push_outbox".to_owned(),
                endpoint_uri: "reticulum:preview-unavailable".to_owned(),
                behavior: ReticulumPreviewBehavior::RejectDeliveryAttempts,
            },
            "reticulum_preview_transport_unavailable",
            RadrootsSdkErrorClass::Unsupported,
            false,
            vec![RadrootsSdkRecoveryAction::ConfigureTransportTargets],
        ),
        (
            RadrootsSdkError::ReticulumPreviewTransportUnavailable {
                operation: "sync.push_outbox".to_owned(),
                endpoint_uri: "reticulum:preview-unavailable".to_owned(),
                behavior: ReticulumPreviewBehavior::DeferDeliveryPlans,
            },
            "reticulum_preview_transport_deferred",
            RadrootsSdkErrorClass::Unsupported,
            false,
            vec![RadrootsSdkRecoveryAction::ConfigureTransportTargets],
        ),
        (
            RadrootsSdkError::ProductSyncTransportSetupFailure {
                message: "relay setup".to_owned(),
            },
            "product_sync_transport_setup_failure",
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
            RadrootsSdkError::GeoNames {
                kind: RadrootsSdkGeoNamesErrorKind::Download,
                message: "download".to_owned(),
            },
            "geonames_download",
            RadrootsSdkErrorClass::Transport,
            true,
            vec![RadrootsSdkRecoveryAction::RetryGeoNamesDownload],
        ),
        (
            RadrootsSdkError::Transport {
                message: "relay".to_owned(),
            },
            "transport",
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
fn relay_target_set_validates_normalizes_preserves_order_and_caps() {
    let targets = TargetSet::nostr_relays(
        [" wss://relay-b.example.com/ ", "wss://relay-a.example.com"],
        NostrRelayUrlPolicy::Public,
    )
    .expect("targets");

    assert_eq!(
        targets.nostr_relay_urls(),
        &[
            "wss://relay-b.example.com".to_owned(),
            "wss://relay-a.example.com".to_owned()
        ]
    );
    assert_eq!(
        targets.canonical_targets(),
        &[
            "5136077cfe7eddcbfaddc5d7bf1f42cdbb8191f3691b86ccc3a81047851cef05".to_owned(),
            "fc957b234632cc52e2be19cba88bc85c69966ee5a2df61742b5875ff717fd6fa".to_owned()
        ]
    );
    assert_eq!(
        serde_json::to_value(TargetPolicy::explicit(targets.clone()))
            .expect("relay target policy json"),
        serde_json::json!({
            "kind": "explicit",
            "targets": [
                {
                    "kind": "nostr",
                    "uri": "wss://relay-b.example.com",
                    "scope": null,
                    "label": null,
                    "fingerprint": "5136077cfe7eddcbfaddc5d7bf1f42cdbb8191f3691b86ccc3a81047851cef05"
                },
                {
                    "kind": "nostr",
                    "uri": "wss://relay-a.example.com",
                    "scope": null,
                    "label": null,
                    "fingerprint": "fc957b234632cc52e2be19cba88bc85c69966ee5a2df61742b5875ff717fd6fa"
                }
            ],
            "canonical_targets": [
                "5136077cfe7eddcbfaddc5d7bf1f42cdbb8191f3691b86ccc3a81047851cef05",
                "fc957b234632cc52e2be19cba88bc85c69966ee5a2df61742b5875ff717fd6fa"
            ]
        })
    );

    assert!(matches!(
        TargetSet::nostr_relays(
            ["wss://relay-a.example.com", "WSS://RELAY-A.EXAMPLE.COM/"],
            NostrRelayUrlPolicy::Public,
        ),
        Err(RadrootsSdkError::Transport { ref message })
            if message == "transport target set contains duplicate fingerprints"
    ));

    assert!(matches!(
        TargetSet::nostr_relays(Vec::<String>::new(), NostrRelayUrlPolicy::Public),
        Err(RadrootsSdkError::EmptyTransportTargets { .. })
    ));

    let too_many = (0..=SDK_TRANSPORT_TARGET_MAX_COUNT)
        .map(|index| format!("wss://relay-{index}.example.com"))
        .collect::<Vec<_>>();
    assert!(matches!(
        TargetSet::nostr_relays(too_many, NostrRelayUrlPolicy::Public),
        Err(RadrootsSdkError::TransportTargetLimitExceeded {
            max: SDK_TRANSPORT_TARGET_MAX_COUNT,
            actual
        }) if actual == SDK_TRANSPORT_TARGET_MAX_COUNT + 1
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
        wal_status: SdkSqliteWalStatus { wal_enabled: true },
        integrity_ok: true,
        integrity_result: "ok".to_owned(),
    };
    let checkpoint = SdkSqliteWalCheckpointReceipt {
        wal_enabled: true,
        busy: 0,
        log_frame_count: 8,
        checkpointed_frame_count: 8,
        checkpoint_complete: true,
    };
    let private_store = SdkSqliteStoreStatus {
        schema_version: 2,
        ..store.clone()
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
                transport_observations: 1,
                last_event_seq: Some(2),
                last_event_updated_at_ms: Some(1_700_000_000_000),
            },
            outbox: SdkOutboxStorageStatus {
                store: store.clone(),
                total_events: 3,
                pending_events: 1,
                retryable_events: 1,
                terminal_events: 1,
                failed_terminal_events: 0,
                preview_unavailable_events: 0,
                deferred_until_implemented_events: 0,
                ready_signed_events: 1,
                publishing_events: 0,
                last_attempt_at_ms: Some(1_700_000_000_000),
                last_error: Some("relay publish incomplete".to_owned()),
            },
            private_store: SdkPrivateStoreStorageStatus {
                store: private_store,
                farm_private_locations: 4,
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
                    "wal_status": {
                        "wal_enabled": true
                    },
                    "integrity_ok": true,
                    "integrity_result": "ok"
                },
                "total_events": 2,
                "projection_eligible_events": 1,
                "transport_observations": 1,
                "last_event_seq": 2,
                "last_event_updated_at_ms": 1700000000000i64
            },
            "outbox": {
                "store": {
                    "schema_version": 1,
                    "journal_mode": "wal",
                    "foreign_keys_enabled": true,
                    "busy_timeout_ms": 5000,
                    "wal_status": {
                        "wal_enabled": true
                    },
                    "integrity_ok": true,
                    "integrity_result": "ok"
                },
                "total_events": 3,
                "pending_events": 1,
                "retryable_events": 1,
                "terminal_events": 1,
                "failed_terminal_events": 0,
                "preview_unavailable_events": 0,
                "deferred_until_implemented_events": 0,
                "ready_signed_events": 1,
                "publishing_events": 0,
                "last_attempt_at_ms": 1700000000000i64,
                "last_error": "relay publish incomplete"
            },
            "private_store": {
                "store": {
                    "schema_version": 2,
                    "journal_mode": "wal",
                    "foreign_keys_enabled": true,
                    "busy_timeout_ms": 5000,
                    "wal_status": {
                        "wal_enabled": true
                    },
                    "integrity_ok": true,
                    "integrity_result": "ok"
                },
                "farm_private_locations": 4
            }
        })
    );
    assert_eq!(
        serde_json::to_value(StorageCheckpointRequest::new()).expect("checkpoint request"),
        serde_json::json!({})
    );
    assert_eq!(
        serde_json::to_value(StorageCheckpointReceipt {
            storage: SdkStorageKind::Directory,
            paths: None,
            event_store: checkpoint.clone(),
            outbox: checkpoint.clone(),
            private_store: checkpoint,
        })
        .expect("checkpoint receipt"),
        serde_json::json!({
            "storage": "directory",
            "paths": null,
            "event_store": {
                "wal_enabled": true,
                "busy": 0,
                "log_frame_count": 8,
                "checkpointed_frame_count": 8,
                "checkpoint_complete": true
            },
            "outbox": {
                "wal_enabled": true,
                "busy": 0,
                "log_frame_count": 8,
                "checkpointed_frame_count": 8,
                "checkpoint_complete": true
            },
            "private_store": {
                "wal_enabled": true,
                "busy": 0,
                "log_frame_count": 8,
                "checkpointed_frame_count": 8,
                "checkpoint_complete": true
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
            private_store_ok: true,
            event_store_events: 2,
            outbox_events: 3,
            private_farm_locations: 4,
        })
        .expect("backup verification"),
        serde_json::json!({
            "event_store_ok": true,
            "outbox_ok": true,
            "private_store_ok": true,
            "event_store_events": 2,
            "outbox_events": 3,
            "private_farm_locations": 4
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

async fn sqlite_table_names(path: &Path) -> Vec<String> {
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open sqlite for table inspection");
    let names = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("table names");
    pool.close().await;
    names
}

async fn sqlite_trade_projection_primary_key(path: &Path) -> Vec<String> {
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open sqlite for trade projection inspection");
    let rows = sqlx::query("PRAGMA table_info(trade_projection)")
        .fetch_all(&pool)
        .await
        .expect("trade projection table info");
    let mut primary_key = rows
        .iter()
        .filter_map(|row| {
            let pk = row.try_get::<i64, _>("pk").expect("pk");
            (pk > 0).then(|| {
                (
                    pk,
                    row.try_get::<String, _>("name")
                        .expect("primary key column"),
                )
            })
        })
        .collect::<Vec<_>>();
    primary_key.sort_by_key(|(pk, _)| *pk);
    pool.close().await;
    primary_key
        .into_iter()
        .map(|(_, name)| name)
        .collect::<Vec<_>>()
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
        (
            "sdk_v1_myc_nip46_signer_setup",
            include_str!("../examples/sdk_v1_myc_nip46_signer_setup.rs"),
        ),
    ];

    for (name, example) in examples {
        assert!(!example.contains(concat!("Wire", "EventParts")), "{name}");
        assert!(
            !example.contains(concat!("Radroots", "Frozen", "EventDraft")),
            "{name}"
        );
        assert!(!example.contains("protocol::wire"), "{name}");
        assert!(!example.contains("event_codec::wire"), "{name}");
        assert!(!example.contains(".as_wire_parts("), "{name}");
        assert!(!example.contains(".into_wire_parts("), "{name}");
    }

    let listing_prepare = include_str!("../examples/sdk_v1_listing_prepare.rs");
    assert!(listing_prepare.contains("RadrootsClient::builder()"));
    assert!(listing_prepare.contains("ListingPreparePublishRequest"));
    assert!(listing_prepare.contains("prepare_publish"));

    let local_enqueue = include_str!("../examples/sdk_v1_local_enqueue_and_mock_sync.rs");
    assert!(local_enqueue.contains("RadrootsClient::builder()"));
    assert!(local_enqueue.contains("ListingPreparePublishRequest"));
    assert!(local_enqueue.contains("TargetPolicy"));
    assert!(local_enqueue.contains("TargetSet"));
    assert!(local_enqueue.contains("NostrRelayUrlPolicy::Localhost"));
    assert!(local_enqueue.contains("RadrootsSdkLocalKeySigner"));
    assert!(local_enqueue.contains("RadrootsSdkSignerProvider::LocalKey"));
    assert!(local_enqueue.contains("enqueue_prepared_publish"));
    assert!(!local_enqueue.contains("enqueue_prepared_publish_with_explicit_signer"));
    assert!(local_enqueue.contains("push_outbox_with_transport"));
    assert!(local_enqueue.contains("TradeStatusRequest"));

    let myc_setup = include_str!("../examples/sdk_v1_myc_nip46_signer_setup.rs");
    assert!(myc_setup.contains("RadrootsSdkMycNip46Signer"));
    assert!(myc_setup.contains("RadrootsSdkSignerProvider::MycNip46"));
    assert!(myc_setup.contains("radroots_sdk_myc_nip46_product_permission_strings"));
}
