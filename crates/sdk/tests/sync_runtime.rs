#![cfg(feature = "runtime")]

use futures::future::BoxFuture;
use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_event_store::RadrootsEventStore;
use radroots_events::{
    contract::RadrootsActorRole,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent, RadrootsSignedNostrEventParts},
    farm::RadrootsFarmRef,
    ids::{RadrootsDTag, RadrootsEventId, RadrootsInventoryBinId},
    listing::{RadrootsListing, RadrootsListingBin, RadrootsListingProduct},
};
use radroots_outbox::{RadrootsOutbox, RadrootsOutboxEventState, RadrootsOutboxOperationInput};
use radroots_relay_transport::{
    RadrootsMockRelayPublishAdapter, RadrootsRelayOutcome, RadrootsRelayPublishAdapter,
    RadrootsRelayPublishRelayReceipt, RadrootsRelayPublishRequest, RadrootsRelayTransportError,
};
use radroots_sdk::{
    BackupRequest, IntegrityRequest, ListingEnqueuePublishRequest, ListingPreparePublishRequest,
    PUSH_OUTBOX_DEFAULT_CLAIM_TTL_MS, PUSH_OUTBOX_DEFAULT_LIMIT,
    PUSH_OUTBOX_DEFAULT_NEXT_ATTEMPT_DELAY_MS, PUSH_OUTBOX_MAX_LIMIT, PushOutboxEventReceipt,
    PushOutboxEventState, PushOutboxReceipt, PushOutboxRelayOutcomeKind, PushOutboxRelayReceipt,
    PushOutboxRequest, RadrootsSdk, RadrootsSdkError, RadrootsSdkTimestamp, SdkRelayAuthPolicy,
    SdkRelayTargetPolicy, SdkRelayUrlPolicy, StorageStatusRequest, SyncStatusRequest,
    SyncStatusSource,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SELLER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FARM_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAA";
const LISTING_A_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAQ";
const LISTING_B_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAg";
const LISTING_C_D_TAG: &str = "AAAAAAAAAAAAAAAAAAAAAw";
const RELAY_A: &str = "wss://relay-a.example.com";
const RELAY_B: &str = "wss://relay-b.example.com";
const RELAY_C: &str = "wss://relay-c.example.com";
const LOCAL_RELAY_A: &str = "ws://localhost:8080";
const LOCAL_RELAY_B: &str = "ws://127.0.0.1:8081";
const LOCAL_RELAY_C: &str = "ws://[::1]:8082";
const NONLOCAL_WS_RELAY: &str = "ws://relay.example.com";
const PRIVATE_LAN_WS_RELAY: &str = "ws://192.168.1.10:8080";

#[derive(Clone)]
struct FixtureSigner {
    identity: RadrootsSignerIdentity,
}

struct TransportFailurePublishAdapter;

#[derive(Clone)]
struct RecordingPublishAdapter {
    delay: Duration,
    raw_events: Arc<Mutex<Vec<String>>>,
    request_times_ms: Arc<Mutex<Vec<i64>>>,
}

impl RadrootsRelayPublishAdapter for TransportFailurePublishAdapter {
    fn publish<'a>(
        &'a self,
        _request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async {
            Err(RadrootsRelayTransportError::Transport(
                "adapter boundary unavailable".to_owned(),
            ))
        })
    }
}

impl RecordingPublishAdapter {
    fn new(delay: Duration) -> Self {
        Self {
            delay,
            raw_events: Arc::new(Mutex::new(Vec::new())),
            request_times_ms: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn captured_raw_events(&self) -> Vec<String> {
        self.raw_events.lock().expect("raw event lock").clone()
    }

    fn request_times_ms(&self) -> Vec<i64> {
        self.request_times_ms
            .lock()
            .expect("request time lock")
            .clone()
    }
}

impl RadrootsRelayPublishAdapter for RecordingPublishAdapter {
    fn publish<'a>(
        &'a self,
        request: RadrootsRelayPublishRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayPublishRelayReceipt>, RadrootsRelayTransportError>>
    {
        Box::pin(async move {
            if !self.delay.is_zero() {
                tokio::time::sleep(self.delay).await;
            }
            self.raw_events
                .lock()
                .expect("raw event lock")
                .push(request.signed_event.raw_json.clone());
            self.request_times_ms
                .lock()
                .expect("request time lock")
                .push(request.now_ms);
            Ok(request
                .targets
                .relays()
                .iter()
                .map(|relay| {
                    RadrootsRelayPublishRelayReceipt::attempted(
                        relay.as_str(),
                        RadrootsRelayOutcome::accepted(),
                    )
                })
                .collect())
        })
    }
}

impl FixtureSigner {
    fn new(pubkey: &str) -> Self {
        Self {
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
        }
    }
}

impl RadrootsEventSigner for FixtureSigner {
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        if self.pubkey().as_str() != draft.expected_pubkey.as_str() {
            return Err(RadrootsSignerError::SigningFailed {
                message: "wrong fixture signer".to_owned(),
            });
        }
        let sig = "f".repeat(128);
        let raw_json = serde_json::json!({
            "id": draft.expected_event_id,
            "pubkey": self.pubkey().as_str(),
            "created_at": draft.created_at,
            "kind": draft.kind,
            "tags": draft.tags,
            "content": draft.content,
            "sig": sig,
        })
        .to_string();
        RadrootsSignedNostrEvent::new(RadrootsSignedNostrEventParts {
            id: draft.expected_event_id.clone(),
            pubkey: self.pubkey().as_str().to_owned(),
            created_at: draft.created_at,
            kind: draft.kind,
            tags: draft.tags.clone(),
            content: draft.content.clone(),
            sig,
            raw_json,
        })
        .map_err(|error| RadrootsSignerError::SigningFailed {
            message: error.to_string(),
        })
    }
}

fn actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER, [RadrootsActorRole::Seller]).expect("actor")
}

fn listing(d_tag: &str, title: &str) -> RadrootsListing {
    RadrootsListing {
        d_tag: RadrootsDTag::parse(d_tag).expect("d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: SELLER.to_owned(),
            d_tag: FARM_D_TAG.to_owned(),
        },
        product: RadrootsListingProduct {
            key: "coffee".to_owned(),
            title: title.to_owned(),
            category: "coffee".to_owned(),
            summary: Some("Single origin coffee".to_owned()),
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
        bins: vec![RadrootsListingBin {
            bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
            quantity: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(1000u32),
                RadrootsCoreUnit::MassG,
            ),
            price_per_canonical_unit: RadrootsCoreQuantityPrice {
                amount: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(20u32),
                    RadrootsCoreCurrency::USD,
                ),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1u32),
                    RadrootsCoreUnit::MassG,
                ),
            },
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }],
        resource_area: None,
        plot: None,
        discounts: None,
        inventory_available: None,
        availability: None,
        delivery_method: None,
        location: None,
        images: None,
    }
}

async fn directory_sdk(relays: &[&str]) -> (tempfile::TempDir, RadrootsSdk) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsSdk::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000));
    for relay in relays {
        builder = builder.relay_url(*relay);
    }
    let sdk = builder.build().await.expect("sdk");
    (tempdir, sdk)
}

async fn system_clock_directory_sdk(relays: &[&str]) -> (tempfile::TempDir, RadrootsSdk) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsSdk::builder().directory_storage(tempdir.path().join("sdk"));
    for relay in relays {
        builder = builder.relay_url(*relay);
    }
    let sdk = builder.build().await.expect("sdk");
    (tempdir, sdk)
}

async fn enqueue_listing(sdk: &RadrootsSdk, d_tag: &str, title: &str, relays: &[&str]) -> i64 {
    enqueue_listing_with_policy(sdk, d_tag, title, relays, SdkRelayUrlPolicy::Public).await
}

async fn enqueue_listing_with_policy(
    sdk: &RadrootsSdk,
    d_tag: &str,
    title: &str,
    relays: &[&str],
    url_policy: SdkRelayUrlPolicy,
) -> i64 {
    sdk.listings()
        .enqueue_publish(
            ListingEnqueuePublishRequest::new(
                actor(),
                listing(d_tag, title),
                SdkRelayTargetPolicy::UseConfiguredRelays,
            )
            .try_with_target_relays(relays, url_policy)
            .expect("relay targets"),
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect("enqueue")
        .outbox_event_id
}

#[tokio::test]
async fn sync_status_empty_store_reports_canonical_sources_and_configured_relays() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_B, RELAY_A]).await;

    let receipt = sdk
        .sync()
        .status(SyncStatusRequest::new())
        .await
        .expect("status");

    assert_eq!(receipt.source, SyncStatusSource::SdkCanonicalStores);
    assert_eq!(receipt.observed_at_ms, 1_700_000_000_000);
    assert_eq!(receipt.event_store.total_events, 0);
    assert_eq!(receipt.event_store.projection_eligible_events, 0);
    assert_eq!(receipt.event_store.relay_observations, 0);
    assert_eq!(receipt.event_store.last_event_seq, None);
    assert_eq!(receipt.outbox.total_events, 0);
    assert_eq!(receipt.outbox.pending_events, 0);
    assert_eq!(receipt.outbox.retryable_events, 0);
    assert_eq!(receipt.outbox.terminal_events, 0);
    assert_eq!(receipt.outbox.failed_terminal_events, 0);
    assert_eq!(receipt.outbox.ready_signed_events, 0);
    assert_eq!(receipt.relay_targets.configured_count, 2);
    assert_eq!(
        receipt.relay_targets.configured_relays,
        vec![RELAY_B.to_owned(), RELAY_A.to_owned()]
    );
    assert_eq!(
        serde_json::to_value(&receipt).expect("status json"),
        serde_json::json!({
            "source": "sdk_canonical_stores",
            "observed_at_ms": 1700000000000i64,
            "event_store": {
                "total_events": 0,
                "projection_eligible_events": 0,
                "relay_observations": 0,
                "last_event_seq": null,
                "last_event_updated_at_ms": null
            },
            "outbox": {
                "total_events": 0,
                "pending_events": 0,
                "retryable_events": 0,
                "terminal_events": 0,
                "failed_terminal_events": 0,
                "ready_signed_events": 0,
                "publishing_events": 0,
                "last_attempt_at_ms": null,
                "last_error": null
            },
            "relay_targets": {
                "configured_count": 2,
                "configured_relays": [RELAY_B, RELAY_A]
            }
        })
    );
}

#[tokio::test]
async fn sync_status_reports_pending_retryable_terminal_and_last_attempt_metadata() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A, RELAY_B, RELAY_C]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Retryable Coffee", &[RELAY_A]).await;
    sdk.sync()
        .push_outbox_with_adapter(
            &TransportFailurePublishAdapter,
            PushOutboxRequest::new().with_limit(1),
        )
        .await
        .expect("retryable push");
    enqueue_listing(&sdk, LISTING_B_D_TAG, "Published Coffee", &[RELAY_B]).await;
    sdk.sync()
        .push_outbox_with_adapter(
            &RadrootsMockRelayPublishAdapter::new(),
            PushOutboxRequest::new().with_limit(1),
        )
        .await
        .expect("published push");
    enqueue_listing(&sdk, LISTING_C_D_TAG, "Pending Coffee", &[RELAY_C]).await;

    let receipt = sdk
        .sync()
        .status(SyncStatusRequest::new())
        .await
        .expect("status");

    assert_eq!(receipt.event_store.total_events, 3);
    assert_eq!(receipt.outbox.total_events, 3);
    assert_eq!(receipt.outbox.pending_events, 1);
    assert_eq!(receipt.outbox.retryable_events, 1);
    assert_eq!(receipt.outbox.terminal_events, 1);
    assert_eq!(receipt.outbox.failed_terminal_events, 0);
    assert_eq!(receipt.outbox.ready_signed_events, 1);
    assert_eq!(receipt.outbox.publishing_events, 0);
    assert_eq!(receipt.outbox.last_attempt_at_ms, Some(1_700_000_000_000));
    assert_eq!(
        receipt.outbox.last_error.as_deref(),
        Some("relay publish incomplete")
    );
}

#[tokio::test]
async fn sdk_directory_backup_creates_verified_canonical_store_copy() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let outbox_event_id = enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;

    let status = sdk
        .storage_status(StorageStatusRequest::default())
        .await
        .expect("storage status");
    let source_paths = sdk.storage_paths().expect("source paths");
    assert_eq!(status.paths.as_ref(), Some(source_paths));
    assert_eq!(status.event_store.total_events, 1);
    assert_eq!(status.outbox.total_events, 1);
    assert_eq!(status.outbox.ready_signed_events, 1);
    assert!(status.event_store.store.integrity_ok);
    assert!(status.outbox.store.integrity_ok);
    assert_eq!(status.event_store.store.journal_mode, "wal");
    assert_eq!(status.outbox.store.journal_mode, "wal");

    let integrity = sdk
        .integrity(IntegrityRequest::default())
        .await
        .expect("integrity");
    assert_eq!(
        integrity.checked_paths,
        vec![
            source_paths.event_store_path.clone(),
            source_paths.outbox_path.clone()
        ]
    );
    assert!(integrity.event_store_ok);
    assert!(integrity.outbox_ok);

    let backup_destination = tempdir.path().join("backup");
    let backup = sdk
        .backup(BackupRequest {
            destination: backup_destination.clone(),
            overwrite: false,
        })
        .await
        .expect("backup");
    let event_store_path = backup
        .event_store_path
        .as_ref()
        .expect("event store backup");
    let outbox_path = backup.outbox_path.as_ref().expect("outbox backup");
    let manifest_path = backup.manifest_path.as_ref().expect("manifest");
    assert!(event_store_path.exists());
    assert!(outbox_path.exists());
    assert!(manifest_path.exists());
    assert_eq!(backup.manifest.created_at_ms, 1_700_000_000_000);
    assert_eq!(backup.manifest.source_status.event_store.total_events, 1);
    assert_eq!(backup.manifest.source_status.outbox.total_events, 1);
    assert!(backup.manifest.backup_verification.event_store_ok);
    assert!(backup.manifest.backup_verification.outbox_ok);

    let backup_event_store = RadrootsEventStore::open_file(event_store_path)
        .await
        .expect("backup event store");
    let backup_outbox = RadrootsOutbox::open_file(outbox_path)
        .await
        .expect("backup outbox");
    assert_eq!(
        backup_event_store
            .status_summary()
            .await
            .expect("backup event status")
            .total_events,
        1
    );
    assert_eq!(
        backup_outbox
            .status_summary(i64::MAX)
            .await
            .expect("backup outbox status")
            .total_events,
        1
    );
    assert_eq!(
        backup_outbox
            .get_event(outbox_event_id)
            .await
            .expect("backup event")
            .expect("backup event")
            .state,
        RadrootsOutboxEventState::Signed
    );

    let duplicate = sdk
        .backup(BackupRequest {
            destination: backup_destination.clone(),
            overwrite: false,
        })
        .await
        .expect_err("duplicate backup");
    assert!(matches!(duplicate, RadrootsSdkError::InvalidRequest { .. }));

    sdk.backup(BackupRequest {
        destination: backup_destination,
        overwrite: true,
    })
    .await
    .expect("overwrite backup");
}

#[cfg(unix)]
#[tokio::test]
async fn sdk_backup_rejects_symlink_destination_even_with_overwrite() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let target = tempdir.path().join("backup-target");
    let destination = tempdir.path().join("backup-link");
    std::fs::create_dir(&target).expect("target");
    std::os::unix::fs::symlink(&target, &destination).expect("symlink");

    let error = sdk
        .backup(BackupRequest {
            destination,
            overwrite: true,
        })
        .await
        .expect_err("symlink destination");
    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert!(target.exists());
}

#[tokio::test]
async fn push_outbox_empty_queue_returns_zero_counts() {
    let (_tempdir, sdk) = directory_sdk(&[]).await;
    let adapter = RadrootsMockRelayPublishAdapter::new();
    let request = PushOutboxRequest::new();

    assert_eq!(request.limit, PUSH_OUTBOX_DEFAULT_LIMIT);

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, request)
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 0);
    assert!(receipt.events.is_empty());
    assert!(adapter.captured_raw_events().is_empty());
}

#[test]
fn push_outbox_contract_dtos_serialize_deterministically() {
    let request = PushOutboxRequest::new()
        .with_limit(2)
        .republish_accepted_relays(true)
        .with_relay_url_policy(SdkRelayUrlPolicy::Localhost)
        .with_auth_policy(SdkRelayAuthPolicy::DetectOnly)
        .with_claim_ttl_ms(1_000)
        .with_next_attempt_delay_ms(2_000);
    assert_eq!(
        serde_json::to_value(&request).expect("request json"),
        serde_json::json!({
            "limit": 2,
            "republish_accepted_relays": true,
            "relay_url_policy": "localhost",
            "auth_policy": "detect_only",
            "claim_ttl_ms": 1000,
            "next_attempt_delay_ms": 2000
        })
    );
    assert_eq!(PUSH_OUTBOX_DEFAULT_CLAIM_TTL_MS, 30_000);
    assert_eq!(PUSH_OUTBOX_DEFAULT_NEXT_ATTEMPT_DELAY_MS, 60_000);

    let receipt = PushOutboxReceipt {
        attempted_events: 1,
        published_events: 1,
        retryable_events: 0,
        terminal_events: 0,
        events: vec![PushOutboxEventReceipt {
            event_id: RadrootsEventId::parse(&"a".repeat(64)).expect("event id"),
            outbox_event_id: 7,
            final_state: PushOutboxEventState::Published,
            attempted_count: 2,
            accepted_count: 1,
            retryable_count: 1,
            terminal_count: 0,
            quorum: 1,
            quorum_met: true,
            relays: vec![PushOutboxRelayReceipt {
                relay_url: RELAY_A.to_owned(),
                outcome_kind: PushOutboxRelayOutcomeKind::DuplicateAccepted,
                attempted: true,
                message: Some("duplicate".to_owned()),
            }],
        }],
    };
    assert_eq!(
        serde_json::to_value(receipt).expect("receipt json"),
        serde_json::json!({
            "attempted_events": 1,
            "published_events": 1,
            "retryable_events": 0,
            "terminal_events": 0,
            "events": [{
                "event_id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "outbox_event_id": 7,
                "final_state": "published",
                "attempted_count": 2,
                "accepted_count": 1,
                "retryable_count": 1,
                "terminal_count": 0,
                "quorum": 1,
                "quorum_met": true,
                "relays": [{
                    "relay_url": RELAY_A,
                    "outcome_kind": "duplicate_accepted",
                    "attempted": true,
                    "message": "duplicate"
                }]
            }]
        })
    );
}

#[cfg(not(feature = "relay-runtime"))]
#[tokio::test]
async fn product_push_outbox_without_relay_runtime_returns_structured_error() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A]).await;

    let error = sdk
        .sync()
        .push_outbox(PushOutboxRequest::new())
        .await
        .expect_err("unsupported product push");

    assert!(matches!(
        error,
        RadrootsSdkError::ProductSyncUnsupported { .. }
    ));
}

#[cfg(feature = "relay-runtime")]
#[tokio::test]
async fn product_push_outbox_empty_queue_does_not_require_builder_relays() {
    let (_tempdir, sdk) = directory_sdk(&[]).await;

    let receipt = sdk
        .sync()
        .push_outbox(PushOutboxRequest::default())
        .await
        .expect("product push");

    assert_eq!(receipt.attempted_events, 0);
    assert!(receipt.events.is_empty());
}

#[tokio::test]
async fn push_outbox_rejects_invalid_limits_before_claiming() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let zero = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(0))
        .await
        .expect_err("zero limit");
    let too_large = sdk
        .sync()
        .push_outbox_with_adapter(
            &adapter,
            PushOutboxRequest::new().with_limit(PUSH_OUTBOX_MAX_LIMIT + 1),
        )
        .await
        .expect_err("too large");
    let zero_ttl = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_claim_ttl_ms(0))
        .await
        .expect_err("zero ttl");
    let zero_delay = sdk
        .sync()
        .push_outbox_with_adapter(
            &adapter,
            PushOutboxRequest::new().with_next_attempt_delay_ms(0),
        )
        .await
        .expect_err("zero delay");

    assert!(matches!(zero, RadrootsSdkError::InvalidRequest { .. }));
    assert!(matches!(too_large, RadrootsSdkError::InvalidRequest { .. }));
    assert!(matches!(zero_ttl, RadrootsSdkError::InvalidRequest { .. }));
    assert!(matches!(
        zero_delay,
        RadrootsSdkError::InvalidRequest { .. }
    ));
    assert!(adapter.captured_raw_events().is_empty());
}

#[tokio::test]
async fn push_outbox_with_adapter_uses_queued_targets_without_builder_relays() {
    let (_tempdir, sdk) = directory_sdk(&[]).await;
    let outbox_event_id = enqueue_listing(&sdk, LISTING_A_D_TAG, "Coffee", &[RELAY_A]).await;
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 1);
    assert_eq!(receipt.published_events, 1);
    assert_eq!(receipt.retryable_events, 0);
    assert_eq!(receipt.terminal_events, 0);
    assert_eq!(receipt.events.len(), 1);
    let event = &receipt.events[0];
    assert_eq!(event.outbox_event_id, outbox_event_id);
    assert_eq!(event.final_state, PushOutboxEventState::Published);
    assert_eq!(event.attempted_count, 1);
    assert_eq!(event.accepted_count, 1);
    assert_eq!(event.retryable_count, 0);
    assert_eq!(event.terminal_count, 0);
    assert_eq!(event.quorum, 1);
    assert!(event.quorum_met);
    assert_eq!(event.relays.len(), 1);
    assert_eq!(
        event.relays[0].outcome_kind,
        PushOutboxRelayOutcomeKind::Accepted
    );
    assert_eq!(adapter.captured_raw_events().len(), 1);

    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    let stored = outbox
        .get_event(outbox_event_id)
        .await
        .expect("stored")
        .expect("stored");
    assert_eq!(stored.state, RadrootsOutboxEventState::Published);
}

#[tokio::test]
async fn push_outbox_default_public_policy_rejects_queued_localhost_ws_targets() {
    let (_tempdir, sdk) = directory_sdk(&[]).await;
    enqueue_listing_with_policy(
        &sdk,
        LISTING_A_D_TAG,
        "Local Coffee",
        &[LOCAL_RELAY_A],
        SdkRelayUrlPolicy::Localhost,
    )
    .await;
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let error = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect_err("public push should reject ws target");

    assert!(matches!(error, RadrootsSdkError::InvalidRelayUrl { .. }));
    assert!(adapter.captured_raw_events().is_empty());
}

#[tokio::test]
async fn push_outbox_with_adapter_accepts_explicit_queued_localhost_ws_targets() {
    let (_tempdir, sdk) = directory_sdk(&[]).await;
    let outbox_event_id = enqueue_listing_with_policy(
        &sdk,
        LISTING_A_D_TAG,
        "Local Coffee",
        &[LOCAL_RELAY_A, LOCAL_RELAY_B, LOCAL_RELAY_C],
        SdkRelayUrlPolicy::Localhost,
    )
    .await;
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(
            &adapter,
            PushOutboxRequest::new()
                .with_limit(1)
                .with_relay_url_policy(SdkRelayUrlPolicy::Localhost),
        )
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 1);
    assert_eq!(receipt.published_events, 1);
    assert_eq!(receipt.retryable_events, 0);
    assert_eq!(receipt.terminal_events, 0);
    assert_eq!(receipt.events.len(), 1);
    let event = &receipt.events[0];
    assert_eq!(event.outbox_event_id, outbox_event_id);
    assert_eq!(event.final_state, PushOutboxEventState::Published);
    assert_eq!(event.attempted_count, 3);
    assert_eq!(event.accepted_count, 3);
    assert_eq!(event.retryable_count, 0);
    assert_eq!(event.terminal_count, 0);
    assert_eq!(event.quorum, 3);
    assert!(event.quorum_met);
    assert_eq!(event.relays.len(), 3);
    assert!(
        event
            .relays
            .iter()
            .all(|relay| relay.outcome_kind == PushOutboxRelayOutcomeKind::Accepted)
    );
    let relay_urls = event
        .relays
        .iter()
        .map(|relay| relay.relay_url.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        relay_urls,
        vec![LOCAL_RELAY_A, LOCAL_RELAY_B, LOCAL_RELAY_C]
    );
    assert_eq!(adapter.captured_raw_events().len(), 1);
}

#[test]
fn enqueue_publish_rejects_nonlocal_ws_relay_targets() {
    let error = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_C_D_TAG, "Nonlocal Coffee"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([NONLOCAL_WS_RELAY], SdkRelayUrlPolicy::Localhost)
    .expect_err("nonlocal ws relay target");

    assert!(matches!(error, RadrootsSdkError::InvalidRelayUrl { .. }));

    let error = ListingEnqueuePublishRequest::new(
        actor(),
        listing(LISTING_C_D_TAG, "Private LAN Coffee"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([PRIVATE_LAN_WS_RELAY], SdkRelayUrlPolicy::Localhost)
    .expect_err("private LAN ws relay target");

    assert!(matches!(error, RadrootsSdkError::InvalidRelayUrl { .. }));
}

#[tokio::test]
async fn push_outbox_preserves_retryable_and_terminal_relay_outcomes() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A, RELAY_B, RELAY_C]).await;
    enqueue_listing(
        &sdk,
        LISTING_B_D_TAG,
        "Coffee",
        &[RELAY_A, RELAY_B, RELAY_C],
    )
    .await;
    let adapter = RadrootsMockRelayPublishAdapter::new()
        .with_outcome(
            RELAY_A,
            RadrootsRelayOutcome::duplicate_accepted("duplicate: already accepted"),
        )
        .with_outcome(
            RELAY_B,
            RadrootsRelayOutcome::classify("auth-required: login"),
        )
        .with_outcome(
            RELAY_C,
            RadrootsRelayOutcome::classify("restricted: denied"),
        );

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 1);
    assert_eq!(receipt.published_events, 0);
    assert_eq!(receipt.retryable_events, 1);
    assert_eq!(receipt.terminal_events, 0);
    let event = &receipt.events[0];
    assert_eq!(event.final_state, PushOutboxEventState::PublishRetryable);
    assert_eq!(event.accepted_count, 1);
    assert_eq!(event.retryable_count, 1);
    assert_eq!(event.terminal_count, 1);
    assert!(!event.quorum_met);

    let relay_a = event
        .relays
        .iter()
        .find(|relay| relay.relay_url == RELAY_A)
        .expect("relay a");
    let relay_b = event
        .relays
        .iter()
        .find(|relay| relay.relay_url == RELAY_B)
        .expect("relay b");
    let relay_c = event
        .relays
        .iter()
        .find(|relay| relay.relay_url == RELAY_C)
        .expect("relay c");

    assert_eq!(
        relay_a.outcome_kind,
        PushOutboxRelayOutcomeKind::DuplicateAccepted
    );
    assert_eq!(
        relay_b.outcome_kind,
        PushOutboxRelayOutcomeKind::AuthRequired
    );
    assert_eq!(relay_c.outcome_kind, PushOutboxRelayOutcomeKind::Restricted);
    assert_eq!(relay_b.message.as_deref(), Some("auth-required: login"));
}

#[tokio::test]
async fn push_outbox_continues_after_adapter_transport_failure_and_releases_claims() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A, RELAY_B]).await;
    let first_outbox_event_id =
        enqueue_listing(&sdk, LISTING_A_D_TAG, "Coffee One", &[RELAY_A]).await;
    let second_outbox_event_id =
        enqueue_listing(&sdk, LISTING_B_D_TAG, "Coffee Two", &[RELAY_B]).await;

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(
            &TransportFailurePublishAdapter,
            PushOutboxRequest::new().with_limit(2),
        )
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 2);
    assert_eq!(receipt.published_events, 0);
    assert_eq!(receipt.retryable_events, 2);
    assert_eq!(receipt.terminal_events, 0);
    assert_eq!(
        receipt
            .events
            .iter()
            .map(|event| event.outbox_event_id)
            .collect::<Vec<_>>(),
        vec![first_outbox_event_id, second_outbox_event_id]
    );
    assert!(
        receipt
            .events
            .iter()
            .all(|event| event.final_state == PushOutboxEventState::PublishRetryable)
    );
    assert!(
        receipt
            .events
            .iter()
            .flat_map(|event| event.relays.iter())
            .all(|relay| {
                relay.attempted
                    && relay.outcome_kind == PushOutboxRelayOutcomeKind::ConnectionFailed
                    && relay.message.as_deref() == Some("adapter boundary unavailable")
            })
    );

    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    for outbox_event_id in [first_outbox_event_id, second_outbox_event_id] {
        let stored = outbox
            .get_event(outbox_event_id)
            .await
            .expect("stored")
            .expect("stored");
        assert_eq!(stored.state, RadrootsOutboxEventState::PublishRetryable);
        assert!(stored.claim_token.is_none());
    }
}

#[tokio::test]
async fn concurrent_push_outbox_claims_do_not_publish_the_same_event_twice() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Coffee", &[RELAY_A]).await;
    let adapter = RecordingPublishAdapter::new(Duration::from_millis(50));
    let request = PushOutboxRequest::new().with_limit(1);
    let sync = sdk.sync();

    let (left, right) = tokio::join!(
        sync.push_outbox_with_adapter(&adapter, request.clone()),
        sync.push_outbox_with_adapter(&adapter, request)
    );
    let left = left.expect("left push");
    let right = right.expect("right push");

    assert_eq!(left.attempted_events + right.attempted_events, 1);
    assert_eq!(left.published_events + right.published_events, 1);
    assert_eq!(adapter.captured_raw_events().len(), 1);
}

#[tokio::test]
async fn push_outbox_computes_publish_time_for_each_iteration() {
    let (_tempdir, sdk) = system_clock_directory_sdk(&[RELAY_A, RELAY_B]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Coffee One", &[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_B_D_TAG, "Coffee Two", &[RELAY_B]).await;
    let adapter = RecordingPublishAdapter::new(Duration::from_millis(1_200));

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(2))
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 2);
    let request_times_ms = adapter.request_times_ms();
    assert_eq!(request_times_ms.len(), 2);
    assert!(
        request_times_ms[1] > request_times_ms[0],
        "request publish times should advance between iterations: {request_times_ms:?}"
    );
}

#[tokio::test]
async fn push_outbox_returns_fatal_error_for_malformed_signed_event_data() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A, RELAY_B]).await;
    let corrupt_outbox_event_id =
        enqueue_listing(&sdk, LISTING_A_D_TAG, "Corrupt Coffee", &[RELAY_A]).await;
    let safe_outbox_event_id =
        enqueue_listing(&sdk, LISTING_B_D_TAG, "Safe Coffee", &[RELAY_B]).await;
    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    let changed =
        sqlx::query("UPDATE outbox_event SET signed_event_json = ? WHERE outbox_event_id = ?")
            .bind("{malformed-signed-event-json")
            .bind(corrupt_outbox_event_id)
            .execute(outbox.pool())
            .await
            .expect("corrupt signed event");
    assert_eq!(changed.rows_affected(), 1);
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let error = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(2))
        .await
        .expect_err("fatal malformed outbox data");

    assert!(matches!(error, RadrootsSdkError::Outbox { .. }));
    assert!(adapter.captured_raw_events().is_empty());
    let safe_event = outbox
        .get_event(safe_outbox_event_id)
        .await
        .expect("safe event")
        .expect("safe event");
    assert_eq!(safe_event.state, RadrootsOutboxEventState::Signed);
    assert!(safe_event.claim_token.is_none());
}

#[tokio::test]
async fn push_outbox_does_not_claim_unsigned_outbox_work() {
    let (_tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let prepared = sdk
        .listings()
        .prepare_publish(ListingPreparePublishRequest::new(
            actor(),
            listing(LISTING_C_D_TAG, "Unsigned"),
        ))
        .expect("prepared");
    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    let unsigned = outbox
        .enqueue_operation(RadrootsOutboxOperationInput::new(
            "listing.publish.v1",
            prepared.frozen_draft,
            vec![RELAY_A.to_owned()],
            1_700_000_000_000,
        ))
        .await
        .expect("unsigned enqueue");
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(receipt.attempted_events, 0);
    assert!(adapter.captured_raw_events().is_empty());

    let stored = outbox
        .get_event(unsigned.outbox_event_id)
        .await
        .expect("unsigned event")
        .expect("unsigned event");
    assert_eq!(stored.state, RadrootsOutboxEventState::DraftQueued);
    assert!(stored.claim_token.is_none());
}
