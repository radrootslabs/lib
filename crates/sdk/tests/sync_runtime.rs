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
    BackupRequest, IntegrityRequest, LISTING_PUBLISH_OPERATION_KIND, ListingEnqueuePublishRequest,
    ListingPreparePublishRequest, PUSH_OUTBOX_DEFAULT_CLAIM_TTL_MS, PUSH_OUTBOX_DEFAULT_LIMIT,
    PUSH_OUTBOX_DEFAULT_NEXT_ATTEMPT_DELAY_MS, PUSH_OUTBOX_MAX_LIMIT, PushOutboxEventReceipt,
    PushOutboxEventState, PushOutboxReceipt, PushOutboxRelayOutcomeKind, PushOutboxRelayReceipt,
    PushOutboxRequest, RadrootsClient, RadrootsSdkError, RadrootsSdkTimestamp, RestoreRequest,
    SdkBackupManifestKind, SdkRelayAuthPolicy, SdkRelayTargetPolicy, SdkRelayUrlPolicy,
    SdkRestoreState, StorageStatusRequest, SyncStatusRequest, SyncStatusSource,
};
#[cfg(feature = "radrootsd-proxy")]
use radroots_sdk::{SdkPublishTransport, adapters::radrootsd::RadrootsdProxyConfig};
#[cfg(feature = "radrootsd-proxy")]
use std::io::{Read, Write};
#[cfg(feature = "radrootsd-proxy")]
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
#[cfg(feature = "radrootsd-proxy")]
use std::thread::JoinHandle;
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

#[cfg(feature = "radrootsd-proxy")]
struct RecordedProxyRequest {
    body: String,
}

#[cfg(feature = "radrootsd-proxy")]
#[derive(Clone, Copy)]
enum ProxyResponseMode {
    Accepted,
    Retryable,
}

#[derive(Clone)]
struct RecordingPublishAdapter {
    delay: Duration,
    raw_events: Arc<Mutex<Vec<String>>>,
    request_times_ms: Arc<Mutex<Vec<i64>>>,
}

#[cfg(feature = "radrootsd-proxy")]
fn spawn_publish_proxy_server() -> (String, JoinHandle<RecordedProxyRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy server");
    let endpoint = format!("http://{}/rpc", listener.local_addr().expect("addr"));
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let body = read_proxy_request_body(&mut stream);
        write_proxy_response(&mut stream, body.as_str(), ProxyResponseMode::Accepted, 1);
        RecordedProxyRequest { body }
    });
    (endpoint, handle)
}

#[cfg(feature = "radrootsd-proxy")]
fn spawn_publish_proxy_sequence_server(
    responses: Vec<ProxyResponseMode>,
) -> (String, JoinHandle<Vec<RecordedProxyRequest>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy server");
    let endpoint = format!("http://{}/rpc", listener.local_addr().expect("addr"));
    let handle = std::thread::spawn(move || {
        responses
            .into_iter()
            .enumerate()
            .map(|(index, mode)| {
                let (mut stream, _) = listener.accept().expect("accept");
                let body = read_proxy_request_body(&mut stream);
                write_proxy_response(&mut stream, body.as_str(), mode, index + 1);
                RecordedProxyRequest { body }
            })
            .collect()
    });
    (endpoint, handle)
}

#[cfg(feature = "radrootsd-proxy")]
fn read_proxy_request_body(stream: &mut TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let read = stream.read(&mut buffer).expect("read request");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            let headers_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("headers end")
                + 4;
            let header_text = String::from_utf8_lossy(&request[..headers_end]);
            let content_length = header_text
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().expect("content length"))
                })
                .unwrap_or(0);
            while request.len() < headers_end + content_length {
                let read = stream.read(&mut buffer).expect("read body");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
            break;
        }
    }
    let request_text = String::from_utf8_lossy(&request);
    let (_, body) = request_text.split_once("\r\n\r\n").expect("request body");
    body.to_owned()
}

#[cfg(feature = "radrootsd-proxy")]
fn write_proxy_response(
    stream: &mut TcpStream,
    body: &str,
    mode: ProxyResponseMode,
    job_number: usize,
) {
    let body_json: serde_json::Value = serde_json::from_str(body).expect("body json");
    let event = &body_json["params"]["event"];
    let (status, terminal, delivery_satisfied, acknowledged_count, retryable_count, relay) =
        match mode {
            ProxyResponseMode::Accepted => (
                "delivery_satisfied",
                true,
                true,
                1,
                0,
                serde_json::json!({
                    "relay_url": "wss://daemon-resolved.example.com",
                    "source": "daemon_default",
                    "attempted": true,
                    "outcome_kind": "accepted",
                    "message": "accepted"
                }),
            ),
            ProxyResponseMode::Retryable => (
                "delivery_unsatisfied_retryable",
                false,
                false,
                0,
                1,
                serde_json::json!({
                    "relay_url": "wss://daemon-resolved.example.com",
                    "source": "daemon_default",
                    "attempted": false,
                    "outcome_kind": "connection_failed",
                    "message": "dns lookup failed"
                }),
            ),
        };
    let response_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": body_json["id"],
        "result": {
            "deduplicated": false,
            "job": {
                "job_id": format!("job-{job_number}"),
                "status": status,
                "terminal": terminal,
                "delivery_satisfied": delivery_satisfied,
                "event_id": event["id"],
                "pubkey": event["pubkey"],
                "event_kind": event["kind"],
                "relay_policy": body_json["params"]["relay_policy"],
                "delivery_policy": body_json["params"]["delivery_policy"],
                "relay_count": 1,
                "acknowledged_count": acknowledged_count,
                "retryable_count": retryable_count,
                "terminal_count": 0,
                "requested_at_ms": 1700000000000i64,
                "completed_at_ms": 1700000000100i64,
                "relays": [relay]
            }
        }
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    stream
        .write_all(response.as_bytes())
        .expect("write response");
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

async fn directory_sdk(relays: &[&str]) -> (tempfile::TempDir, RadrootsClient) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000));
    for relay in relays {
        builder = builder.relay_url(*relay);
    }
    let sdk = builder.build().await.expect("sdk");
    (tempdir, sdk)
}

async fn system_clock_directory_sdk(relays: &[&str]) -> (tempfile::TempDir, RadrootsClient) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsClient::builder().directory_storage(tempdir.path().join("sdk"));
    for relay in relays {
        builder = builder.relay_url(*relay);
    }
    let sdk = builder.build().await.expect("sdk");
    (tempdir, sdk)
}

async fn enqueue_listing(sdk: &RadrootsClient, d_tag: &str, title: &str, relays: &[&str]) -> i64 {
    enqueue_listing_with_policy(sdk, d_tag, title, relays, SdkRelayUrlPolicy::Public).await
}

async fn backup_source(sdk: &RadrootsClient, root: &Path, name: &str) -> PathBuf {
    let source = root.join(name);
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    source
}

fn rewrite_backup_manifest(source: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
    let manifest_path = source.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("manifest bytes"))
            .expect("manifest json");
    mutate(&mut manifest);
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
    )
    .expect("write manifest");
}

async fn enqueue_listing_with_policy(
    sdk: &RadrootsClient,
    d_tag: &str,
    title: &str,
    relays: &[&str],
    url_policy: SdkRelayUrlPolicy,
) -> i64 {
    sdk.listings()
        .enqueue_publish_with_explicit_signer(
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
        .storage_status(StorageStatusRequest::new())
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
        .integrity(IntegrityRequest::new())
        .await
        .expect("integrity");
    assert_eq!(
        integrity.checked_paths,
        vec![
            source_paths.event_store_path.clone(),
            source_paths.outbox_path.clone(),
            source_paths.private_store_path.clone()
        ]
    );
    assert!(integrity.event_store_ok);
    assert!(integrity.outbox_ok);
    assert!(integrity.private_store_ok);

    let backup_destination = tempdir.path().join("backup");
    let backup = sdk
        .backup(BackupRequest::new(backup_destination.clone()))
        .await
        .expect("backup");
    let event_store_path = backup
        .event_store_path
        .as_ref()
        .expect("event store backup");
    let outbox_path = backup.outbox_path.as_ref().expect("outbox backup");
    let private_store_path = backup
        .private_store_path
        .as_ref()
        .expect("private store backup");
    let manifest_path = backup.manifest_path.as_ref().expect("manifest");
    assert!(event_store_path.exists());
    assert!(outbox_path.exists());
    assert!(private_store_path.exists());
    assert!(manifest_path.exists());
    assert_eq!(
        backup.manifest.manifest_kind,
        SdkBackupManifestKind::StorageBackup
    );
    assert_eq!(
        backup.manifest.backup_paths.event_store_path,
        PathBuf::from("event_store.sqlite")
    );
    assert_eq!(
        backup.manifest.backup_paths.outbox_path,
        PathBuf::from("outbox.sqlite")
    );
    assert_eq!(
        backup.manifest.backup_paths.private_store_path,
        PathBuf::from("private.sqlite")
    );
    assert_eq!(backup.manifest.created_at_ms, 1_700_000_000_000);
    assert_eq!(backup.manifest.source_status.event_store.total_events, 1);
    assert_eq!(backup.manifest.source_status.outbox.total_events, 1);
    assert_eq!(
        backup
            .manifest
            .source_status
            .private_store
            .farm_private_locations,
        0
    );
    assert!(backup.manifest.backup_verification.event_store_ok);
    assert!(backup.manifest.backup_verification.outbox_ok);
    assert!(backup.manifest.backup_verification.private_store_ok);
    assert_eq!(
        backup.manifest.backup_verification.private_farm_locations,
        0
    );

    let restore_archive = RadrootsClient::inspect_restore_archive(backup_destination.clone())
        .await
        .expect("restore archive");
    assert_eq!(restore_archive.manifest, backup.manifest);
    assert_eq!(
        restore_archive.verification,
        backup.manifest.backup_verification
    );
    assert_eq!(
        restore_archive.event_store_path,
        event_store_path.canonicalize().expect("event canonical")
    );
    assert_eq!(
        restore_archive.outbox_path,
        outbox_path.canonicalize().expect("outbox canonical")
    );
    assert_eq!(
        restore_archive.private_store_path,
        private_store_path
            .canonicalize()
            .expect("private store canonical")
    );

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
        .backup(BackupRequest::new(backup_destination.clone()))
        .await
        .expect_err("duplicate backup");
    assert!(matches!(duplicate, RadrootsSdkError::InvalidRequest { .. }));

    sdk.backup(BackupRequest::new(backup_destination).with_overwrite(true))
        .await
        .expect("overwrite backup");
}

#[tokio::test]
async fn runtime_backup_rejects_empty_destination_and_overwrites_file_destination() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;

    let empty_destination = sdk
        .backup(BackupRequest::new(PathBuf::new()))
        .await
        .expect_err("empty backup destination");
    assert!(matches!(
        empty_destination,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let destination = tempdir.path().join("backup-file");
    std::fs::write(&destination, b"old backup placeholder").expect("destination file");
    let duplicate_file = sdk
        .backup(BackupRequest::new(destination.clone()))
        .await
        .expect_err("file destination without overwrite");
    assert!(matches!(
        duplicate_file,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let receipt = sdk
        .backup(BackupRequest::new(destination.clone()).with_overwrite(true))
        .await
        .expect("overwrite file backup");
    assert!(destination.is_dir());
    assert!(
        receipt
            .event_store_path
            .as_ref()
            .expect("event store")
            .exists()
    );
    assert!(receipt.outbox_path.as_ref().expect("outbox").exists());
    assert!(
        receipt
            .private_store_path
            .as_ref()
            .expect("private store")
            .exists()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_backup_rejects_invalid_utf8_destination() {
    use std::os::unix::ffi::OsStringExt;

    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let destination = tempdir
        .path()
        .join(std::ffi::OsString::from_vec(vec![b'b', b'a', b'd', 0x80]));

    let error = sdk
        .backup(BackupRequest::new(destination))
        .await
        .expect_err("invalid utf8 destination");

    assert!(matches!(
        error,
        RadrootsSdkError::InvalidRequest { .. } | RadrootsSdkError::Io { .. }
    ));
    if matches!(error, RadrootsSdkError::InvalidRequest { .. }) {
        assert!(error.to_string().contains("valid UTF-8"));
    }
}

#[tokio::test]
async fn sdk_restore_archive_rejects_missing_manifest() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let source = tempdir.path().join("backup");
    std::fs::create_dir(&source).expect("source");

    let error = RadrootsClient::inspect_restore_archive(source)
        .await
        .expect_err("missing manifest");
    assert!(matches!(error, RadrootsSdkError::Io { .. }));
}

#[tokio::test]
async fn sdk_restore_archive_rejects_malformed_manifest() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let source = tempdir.path().join("backup");
    std::fs::create_dir(&source).expect("source");
    std::fs::write(source.join("manifest.json"), b"{not json").expect("manifest");

    let error = RadrootsClient::inspect_restore_archive(source)
        .await
        .expect_err("malformed manifest");
    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
}

#[tokio::test]
async fn runtime_restore_rejects_empty_missing_file_and_manifest_sources() {
    let tempdir = tempfile::tempdir().expect("tempdir");

    let empty_source = RadrootsClient::inspect_restore_archive(PathBuf::new())
        .await
        .expect_err("empty source");
    assert!(matches!(
        empty_source,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let missing_source = RadrootsClient::inspect_restore_archive(tempdir.path().join("missing"))
        .await
        .expect_err("missing source");
    assert!(matches!(missing_source, RadrootsSdkError::Io { .. }));

    let file_source = tempdir.path().join("backup-file");
    std::fs::write(&file_source, b"not a directory").expect("source file");
    let file_error = RadrootsClient::inspect_restore_archive(file_source)
        .await
        .expect_err("file source");
    assert!(matches!(
        file_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let manifest_dir_source = tempdir.path().join("manifest-dir-source");
    std::fs::create_dir(&manifest_dir_source).expect("manifest source");
    std::fs::create_dir(manifest_dir_source.join("manifest.json")).expect("manifest dir");
    let manifest_dir_error = RadrootsClient::inspect_restore_archive(manifest_dir_source)
        .await
        .expect_err("manifest dir");
    assert!(matches!(
        manifest_dir_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_restore_rejects_symlink_source_and_manifest() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let source = backup_source(&sdk, tempdir.path(), "backup-symlink-manifest").await;

    let source_link = tempdir.path().join("backup-source-link");
    std::os::unix::fs::symlink(&source, &source_link).expect("source symlink");
    let source_error = RadrootsClient::inspect_restore_archive(source_link)
        .await
        .expect_err("source symlink");
    assert!(matches!(
        source_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let manifest_link_source = backup_source(&sdk, tempdir.path(), "backup-manifest-link").await;
    let manifest_path = manifest_link_source.join("manifest.json");
    let manifest_copy = tempdir.path().join("manifest-copy.json");
    std::fs::copy(&manifest_path, &manifest_copy).expect("manifest copy");
    std::fs::remove_file(&manifest_path).expect("remove manifest");
    std::os::unix::fs::symlink(&manifest_copy, &manifest_path).expect("manifest symlink");
    let manifest_error = RadrootsClient::inspect_restore_archive(manifest_link_source)
        .await
        .expect_err("manifest symlink");
    assert!(matches!(
        manifest_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[tokio::test]
async fn runtime_restore_rejects_manifest_contract_edges() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;

    let version_source = backup_source(&sdk, tempdir.path(), "backup-version").await;
    rewrite_backup_manifest(&version_source, |manifest| {
        manifest["manifest_version"] = serde_json::json!(2);
    });
    let version_error = RadrootsClient::inspect_restore_archive(version_source)
        .await
        .expect_err("unsupported manifest version");
    assert!(matches!(
        version_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let empty_path_source = backup_source(&sdk, tempdir.path(), "backup-empty-path").await;
    rewrite_backup_manifest(&empty_path_source, |manifest| {
        manifest["backup_paths"]["event_store_path"] = serde_json::json!("");
    });
    let empty_path_error = RadrootsClient::inspect_restore_archive(empty_path_source)
        .await
        .expect_err("empty archive path");
    assert!(matches!(
        empty_path_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let mismatch_source = backup_source(&sdk, tempdir.path(), "backup-mismatch").await;
    rewrite_backup_manifest(&mismatch_source, |manifest| {
        manifest["backup_verification"]["event_store_events"] = serde_json::json!(999);
    });
    let mismatch_error = RadrootsClient::inspect_restore_archive(mismatch_source)
        .await
        .expect_err("verification mismatch");
    assert!(matches!(
        mismatch_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[tokio::test]
async fn sdk_restore_archive_rejects_traversal_backup_paths() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");

    let manifest_path = source.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
            .expect("manifest json");
    manifest["backup_paths"]["event_store_path"] = serde_json::json!("../event_store.sqlite");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
    )
    .expect("write manifest");

    let error = RadrootsClient::inspect_restore_archive(source)
        .await
        .expect_err("traversal path");
    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
}

#[tokio::test]
async fn sdk_restore_archive_rejects_corrupt_store() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    std::fs::write(source.join("event_store.sqlite"), b"not sqlite").expect("corrupt store");

    let error = RadrootsClient::inspect_restore_archive(source)
        .await
        .expect_err("corrupt store");
    assert!(matches!(
        error,
        RadrootsSdkError::EventStore { .. } | RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn sdk_restore_archive_rejects_symlink_store_member() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    let event_store_path = source.join("event_store.sqlite");
    let target = tempdir.path().join("sdk").join("event_store.sqlite");
    std::fs::remove_file(&event_store_path).expect("remove backup event store");
    std::os::unix::fs::symlink(target, &event_store_path).expect("symlink");

    let error = RadrootsClient::inspect_restore_archive(source)
        .await
        .expect_err("symlink member");
    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
}

#[tokio::test]
async fn runtime_restore_rejects_missing_destination_and_empty_destination() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let source = backup_source(&sdk, tempdir.path(), "backup-destination-required").await;

    let missing_destination = RadrootsClient::restore(RestoreRequest::new(source.clone()))
        .await
        .expect_err("missing destination");
    assert!(matches!(
        missing_destination,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let empty_destination = RadrootsClient::restore(
        RestoreRequest::new(source)
            .with_destination(PathBuf::new())
            .dry_run(),
    )
    .await
    .expect_err("empty destination");
    assert!(matches!(
        empty_destination,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[tokio::test]
async fn sdk_restore_dry_run_validates_destination_without_writing() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");

    let receipt = RadrootsClient::restore(
        RestoreRequest::new(source.clone())
            .with_destination(destination.clone())
            .dry_run(),
    )
    .await
    .expect("restore dry run");

    assert_eq!(receipt.state, SdkRestoreState::DryRun);
    assert_eq!(receipt.destination.as_deref(), Some(destination.as_path()));
    assert_eq!(
        receipt
            .destination_paths
            .as_ref()
            .expect("destination paths")
            .event_store_path,
        destination.join("event_store.sqlite")
    );
    assert!(!destination.exists());
    assert_eq!(receipt.restored_paths, None);
}

#[tokio::test]
async fn sdk_restore_dry_run_rejects_existing_destination_without_overwrite() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    std::fs::create_dir(&destination).expect("destination");
    std::fs::write(destination.join("event_store.sqlite"), b"existing").expect("existing file");

    let error = RadrootsClient::restore(
        RestoreRequest::new(source)
            .with_destination(destination.clone())
            .dry_run(),
    )
    .await
    .expect_err("existing destination");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert!(destination.join("event_store.sqlite").exists());
}

#[tokio::test]
async fn sdk_restore_dry_run_overwrite_keeps_existing_destination_untouched() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    std::fs::create_dir(&destination).expect("destination");
    std::fs::write(destination.join("event_store.sqlite"), b"existing").expect("existing file");

    let receipt = RadrootsClient::restore(
        RestoreRequest::new(source)
            .with_destination(destination.clone())
            .with_overwrite(true)
            .dry_run(),
    )
    .await
    .expect("overwrite dry run");

    assert_eq!(receipt.state, SdkRestoreState::DryRun);
    assert_eq!(
        std::fs::read(destination.join("event_store.sqlite")).expect("existing file"),
        b"existing"
    );
}

#[tokio::test]
async fn sdk_restore_dry_run_rejects_destination_inside_source() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");

    let error = RadrootsClient::restore(
        RestoreRequest::new(source.clone())
            .with_destination(source.join("restore"))
            .with_overwrite(true)
            .dry_run(),
    )
    .await
    .expect_err("destination inside source");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
}

#[tokio::test]
async fn sdk_restore_dry_run_rejects_corrupt_source_without_destination_writes() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    std::fs::write(source.join("event_store.sqlite"), b"not sqlite").expect("corrupt store");

    let error = RadrootsClient::restore(
        RestoreRequest::new(source)
            .with_destination(destination.clone())
            .dry_run(),
    )
    .await
    .expect_err("corrupt source");

    assert!(matches!(
        error,
        RadrootsSdkError::EventStore { .. } | RadrootsSdkError::InvalidRequest { .. }
    ));
    assert!(!destination.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn sdk_restore_dry_run_rejects_symlink_destination() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore-link");
    let target = tempdir.path().join("restore-target");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    std::fs::create_dir(&target).expect("target");
    std::os::unix::fs::symlink(&target, &destination).expect("symlink");

    let error = RadrootsClient::restore(
        RestoreRequest::new(source)
            .with_destination(destination)
            .with_overwrite(true)
            .dry_run(),
    )
    .await
    .expect_err("symlink destination");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert!(target.exists());
}

#[tokio::test]
async fn runtime_restore_handles_existing_file_destinations_by_overwrite_policy() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    let source = backup_source(&sdk, tempdir.path(), "backup-file-destination").await;
    let destination = tempdir.path().join("restore-file");
    std::fs::write(&destination, b"old restore file").expect("destination file");

    let without_overwrite =
        RadrootsClient::restore(RestoreRequest::new(source.clone()).with_destination(&destination))
            .await
            .expect_err("file destination without overwrite");
    assert!(matches!(
        without_overwrite,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let receipt = RadrootsClient::restore(
        RestoreRequest::new(source)
            .with_destination(destination.clone())
            .with_overwrite(true),
    )
    .await
    .expect("file destination overwrite");

    assert_eq!(receipt.state, SdkRestoreState::Completed);
    assert!(destination.is_dir());
    assert!(
        receipt
            .restored_paths
            .as_ref()
            .expect("restored paths")
            .event_store_path
            .exists()
    );
}

#[tokio::test]
async fn sdk_restore_to_empty_destination_succeeds() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");

    let receipt = RadrootsClient::restore(
        RestoreRequest::new(source.clone()).with_destination(destination.clone()),
    )
    .await
    .expect("restore");

    assert_eq!(receipt.state, SdkRestoreState::Completed);
    assert_eq!(receipt.destination.as_deref(), Some(destination.as_path()));
    assert_eq!(
        receipt.restored_paths.as_ref(),
        receipt.destination_paths.as_ref()
    );
    assert!(destination.join("event_store.sqlite").exists());
    assert!(destination.join("outbox.sqlite").exists());
    let restored_sdk = RadrootsClient::builder()
        .directory_storage(destination)
        .build()
        .await
        .expect("restored sdk");
    let status = restored_sdk
        .storage_status(StorageStatusRequest::new())
        .await
        .expect("restored status");
    assert_eq!(status.event_store.total_events, 1);
    assert_eq!(status.outbox.total_events, 1);
    assert_eq!(
        receipt.verification.event_store_events,
        receipt.manifest.backup_verification.event_store_events
    );
    assert_eq!(
        receipt.verification.outbox_events,
        receipt.manifest.backup_verification.outbox_events
    );
}

#[tokio::test]
async fn sdk_restore_existing_destination_fails_without_overwrite() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    std::fs::create_dir(&destination).expect("destination");
    std::fs::write(destination.join("sentinel"), b"keep").expect("sentinel");

    let error = RadrootsClient::restore(RestoreRequest::new(source).with_destination(&destination))
        .await
        .expect_err("existing destination");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert_eq!(
        std::fs::read(destination.join("sentinel")).expect("sentinel"),
        b"keep"
    );
}

#[tokio::test]
async fn sdk_restore_overwrite_replaces_existing_destination() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    std::fs::create_dir(&destination).expect("destination");
    std::fs::write(destination.join("sentinel"), b"replace").expect("sentinel");

    let receipt = RadrootsClient::restore(
        RestoreRequest::new(source)
            .with_destination(destination.clone())
            .with_overwrite(true),
    )
    .await
    .expect("restore");

    assert_eq!(receipt.state, SdkRestoreState::Completed);
    assert!(!destination.join("sentinel").exists());
    let restored_sdk = RadrootsClient::builder()
        .directory_storage(destination)
        .build()
        .await
        .expect("restored sdk");
    let status = restored_sdk
        .storage_status(StorageStatusRequest::new())
        .await
        .expect("restored status");
    assert_eq!(status.event_store.total_events, 1);
    assert_eq!(status.outbox.total_events, 1);
}

#[tokio::test]
async fn sdk_restore_corrupt_backup_leaves_destination_unchanged() {
    let (tempdir, sdk) = directory_sdk(&[RELAY_A]).await;
    enqueue_listing(&sdk, LISTING_A_D_TAG, "Backup Coffee", &[RELAY_A]).await;
    let source = tempdir.path().join("backup");
    let destination = tempdir.path().join("restore");
    sdk.backup(BackupRequest::new(source.clone()))
        .await
        .expect("backup");
    std::fs::write(source.join("event_store.sqlite"), b"not sqlite").expect("corrupt store");
    std::fs::create_dir(&destination).expect("destination");
    std::fs::write(destination.join("sentinel"), b"keep").expect("sentinel");

    let error = RadrootsClient::restore(
        RestoreRequest::new(source)
            .with_destination(destination.clone())
            .with_overwrite(true),
    )
    .await
    .expect_err("corrupt source");

    assert!(matches!(
        error,
        RadrootsSdkError::EventStore { .. } | RadrootsSdkError::InvalidRequest { .. }
    ));
    assert_eq!(
        std::fs::read(destination.join("sentinel")).expect("sentinel"),
        b"keep"
    );
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
        .backup(BackupRequest::new(destination).with_overwrite(true))
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

#[cfg(feature = "radrootsd-proxy")]
#[tokio::test]
async fn product_push_outbox_uses_radrootsd_proxy_transport_with_daemon_resolved_relays() {
    let (endpoint, handle) = spawn_publish_proxy_server();
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .publish_transport(SdkPublishTransport::RadrootsdProxy(
            RadrootsdProxyConfig::new(endpoint),
        ))
        .build()
        .await
        .expect("sdk");

    let enqueue = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(
            ListingEnqueuePublishRequest::new(
                actor(),
                listing(LISTING_A_D_TAG, "Proxy Coffee"),
                SdkRelayTargetPolicy::use_publish_transport(),
            ),
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect("enqueue");

    let receipt = sdk
        .sync()
        .push_outbox(PushOutboxRequest::new().with_limit(1))
        .await
        .expect("proxy push");

    assert_eq!(receipt.attempted_events, 1);
    assert_eq!(receipt.published_events, 1);
    assert_eq!(receipt.events[0].outbox_event_id, enqueue.outbox_event_id);
    assert_eq!(
        receipt.events[0].final_state,
        PushOutboxEventState::Published
    );
    assert_eq!(receipt.events[0].relays.len(), 1);
    assert_eq!(
        receipt.events[0].relays[0].relay_url,
        "wss://daemon-resolved.example.com"
    );
    assert_eq!(
        receipt.events[0].relays[0].outcome_kind,
        PushOutboxRelayOutcomeKind::Accepted
    );

    let recorded = handle.join().expect("proxy request");
    let body: serde_json::Value = serde_json::from_str(recorded.body.as_str()).expect("body");
    assert_eq!(body["method"], "publish.event");
    assert_eq!(body["params"]["relays"], serde_json::json!([]));
    assert_eq!(
        body["params"]["relay_policy"],
        "request_then_author_write_then_daemon_default"
    );
    assert_eq!(body["params"]["delivery_policy"]["mode"], "any");
    assert!(body["params"]["event"]["sig"].as_str().is_some());
    assert!(!recorded.body.contains("bridge."));
    assert!(!recorded.body.contains("signer_session_id"));

    let status = sdk
        .sync()
        .status(SyncStatusRequest::new())
        .await
        .expect("status");
    assert_eq!(status.outbox.terminal_events, 1);
    assert_eq!(status.outbox.ready_signed_events, 0);
}

#[cfg(feature = "radrootsd-proxy")]
#[tokio::test]
async fn product_push_outbox_radrootsd_proxy_idempotency_is_attempt_scoped() {
    let (endpoint, handle) = spawn_publish_proxy_sequence_server(vec![
        ProxyResponseMode::Retryable,
        ProxyResponseMode::Accepted,
    ]);
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage = tempdir.path().join("sdk");
    let transport = SdkPublishTransport::RadrootsdProxy(RadrootsdProxyConfig::new(endpoint));
    let sdk = RadrootsClient::builder()
        .directory_storage(storage.clone())
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .publish_transport(transport.clone())
        .build()
        .await
        .expect("sdk");

    let enqueue = sdk
        .listings()
        .enqueue_publish_with_explicit_signer(
            ListingEnqueuePublishRequest::new(
                actor(),
                listing(LISTING_A_D_TAG, "Retry Coffee"),
                SdkRelayTargetPolicy::use_publish_transport(),
            ),
            &FixtureSigner::new(SELLER),
        )
        .await
        .expect("enqueue");

    let first = sdk
        .sync()
        .push_outbox(
            PushOutboxRequest::new()
                .with_limit(1)
                .with_next_attempt_delay_ms(1),
        )
        .await
        .expect("first proxy push");

    assert_eq!(first.attempted_events, 1);
    assert_eq!(first.retryable_events, 1);
    assert_eq!(first.events[0].outbox_event_id, enqueue.outbox_event_id);
    assert_eq!(
        first.events[0].final_state,
        PushOutboxEventState::PublishRetryable
    );

    drop(sdk);
    let sdk = RadrootsClient::builder()
        .directory_storage(storage)
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_001))
        .publish_transport(transport)
        .build()
        .await
        .expect("reopened sdk");
    let second = sdk
        .sync()
        .push_outbox(PushOutboxRequest::new().with_limit(1))
        .await
        .expect("second proxy push");

    assert_eq!(second.attempted_events, 1);
    assert_eq!(second.published_events, 1);
    assert_eq!(second.events[0].outbox_event_id, enqueue.outbox_event_id);
    assert_eq!(
        second.events[0].final_state,
        PushOutboxEventState::Published
    );

    let recorded = handle.join().expect("proxy requests");
    assert_eq!(recorded.len(), 2);
    let first_body: serde_json::Value =
        serde_json::from_str(recorded[0].body.as_str()).expect("first body");
    let second_body: serde_json::Value =
        serde_json::from_str(recorded[1].body.as_str()).expect("second body");
    let first_key = first_body["params"]["idempotency_key"]
        .as_str()
        .expect("first idempotency key");
    let second_key = second_body["params"]["idempotency_key"]
        .as_str()
        .expect("second idempotency key");
    assert_ne!(first_key, second_key);
    assert_eq!(
        first_body["params"]["event"]["id"],
        second_body["params"]["event"]["id"]
    );
    assert!(
        first_key
            .starts_with(format!("radroots-sdk-outbox-{}-1-", enqueue.outbox_event_id).as_str())
    );
    assert!(
        second_key
            .starts_with(format!("radroots-sdk-outbox-{}-2-", enqueue.outbox_event_id).as_str())
    );
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
            LISTING_PUBLISH_OPERATION_KIND,
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
