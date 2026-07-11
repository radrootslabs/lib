#![cfg(feature = "runtime")]

use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
};
use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
use radroots_events::{
    RadrootsEventEnvelope, RadrootsEventPtr,
    contract::RadrootsActorRole,
    draft::{RadrootsEventDraft, RadrootsSignedEvent},
    ids::{
        RadrootsEventId, RadrootsInventoryBinId, RadrootsListingAddress, RadrootsOrderId,
        RadrootsPublicKey,
    },
    kinds::{KIND_LISTING, KIND_TRADE_TRANSITION_PROOF_REQUEST},
    order::{
        RadrootsOrderDecision, RadrootsOrderDecisionOutcome, RadrootsOrderEconomicItem,
        RadrootsOrderEconomicLine, RadrootsOrderEconomics, RadrootsOrderInventoryCommitment,
        RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
    },
};
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_event, radroots_nostr_sign_frozen_draft,
};
use radroots_outbox::RadrootsOutbox;
use radroots_sdk::{
    DVM_TRADE_TRANSITION_PROOF_REQUEST_OPERATION_KIND, DvmTradeTransitionProofEnqueueRequest,
    DvmTradeTransitionProofPrepareRequest, DvmValidationReceiptIngestRequest, NostrRelayUrlPolicy,
    RadrootsClient, RadrootsSdkError, RadrootsSdkStorageConfig, RadrootsSdkTimestamp,
    RadrootsTradeInventoryBinWitnessDto, RadrootsTradeValidationTrustState, SdkMutationState,
    TargetPolicy, TradeStatusKind, TradeStatusNextActionKind, TradeStatusRequest,
};
#[cfg(feature = "signer-adapters")]
use radroots_sdk::{RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider};
use radroots_trade::validation_receipt::{
    RadrootsTradeCommitmentConfidence, RadrootsTradeValidationReceipt,
    RadrootsValidationReceiptProof, RadrootsValidationReceiptProofSystem,
    RadrootsValidationReceiptResult, RadrootsValidationReceiptStatement,
    RadrootsValidationReceiptType, validation_receipt_event_build,
    validation_receipt_public_values_hash_hex,
};

const BUYER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const BUYER_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const SELLER_SECRET_KEY_HEX: &str =
    "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";
const SELLER_PUBLIC_KEY_HEX: &str =
    "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";
const SERVICE_SECRET_KEY_HEX: &str =
    "48314941f2c9c01ef99f531df7b1d59a8de23dbeb45a498e5aa5f671e921931f";
const RELAY: &str = "wss://relay.radroots.test";

#[derive(Clone)]
struct FixtureSigner {
    identity: RadrootsSignerIdentity,
    keys: RadrootsNostrKeys,
}

impl FixtureSigner {
    fn new(secret_key_hex: &str) -> Self {
        let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
        let keys = RadrootsNostrKeys::new(secret_key);
        let pubkey = keys.public_key().to_hex();
        Self {
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
            keys,
        }
    }
}

impl RadrootsEventSigner for FixtureSigner {
    fn pubkey(&self) -> &RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsEventDraft,
    ) -> Result<RadrootsSignedEvent, RadrootsSignerError> {
        radroots_nostr_sign_frozen_draft(&self.keys, draft).map_err(|error| {
            RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            }
        })
    }
}

#[tokio::test]
async fn dvm_trade_transition_proof_request_enqueues_signed_job() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let signer = FixtureSigner::new(SERVICE_SECRET_KEY_HEX);
    let actor = service_actor(&signer);
    let listing_event_id = deterministic_event_id("listing-event");
    let request_event_id = deterministic_event_id("request-event");
    let decision_event_id = deterministic_event_id("decision-event");
    let request = DvmTradeTransitionProofEnqueueRequest::new(
        actor,
        signer.pubkey().clone(),
        listing_address(),
        listing_event_id.clone(),
        request_event_id.clone(),
        decision_event_id.clone(),
        inventory_bins(),
        explicit_relays(),
    )
    .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_050))
    .with_inventory_sequence(7);

    let receipt = sdk
        .dvm()
        .enqueue_trade_transition_proof_request_with_explicit_signer(request, &signer)
        .await
        .expect("enqueue DVM proof request");

    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
    assert_eq!(receipt.local_event_seq, 1);
    assert_eq!(receipt.listing_event_id, listing_event_id);
    assert_eq!(receipt.request_event_id, request_event_id);
    assert_eq!(receipt.decision_event_id, decision_event_id);
    assert_eq!(
        outbox_operation_kind(&sdk, receipt.outbox_operation_id).await,
        DVM_TRADE_TRANSITION_PROOF_REQUEST_OPERATION_KIND
    );

    let stored = store
        .get_event(receipt.signed_event_id.as_str())
        .await
        .expect("get event")
        .expect("stored DVM request");
    let content: serde_json::Value =
        serde_json::from_str(&stored.content).expect("proof request content");
    let tags: Vec<Vec<String>> = serde_json::from_str(&stored.tags_json).expect("stored tags");
    assert_eq!(stored.kind, KIND_TRADE_TRANSITION_PROOF_REQUEST);
    assert_eq!(content["proof_mode"], "none");
    assert_eq!(content["inventory_sequence"], 7);
    assert!(
        tags.iter()
            .any(|tag| tag == &vec!["p".to_owned(), signer.pubkey().as_str().to_owned()])
    );
    assert!(
        tags.iter()
            .any(|tag| tag.first().map(String::as_str) == Some("a"))
    );
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        1
    );
}

#[cfg(feature = "signer-adapters")]
#[tokio::test]
async fn dvm_trade_transition_proof_request_uses_configured_local_signer() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let signer_keys = keys_from_secret(SERVICE_SECRET_KEY_HEX);
    let signer_pubkey = signer_keys.public_key().to_hex();
    let sdk = RadrootsClient::builder()
        .storage(RadrootsSdkStorageConfig::Directory(
            tempdir.path().join("sdk"),
        ))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::new(signer_keys).expect("local signer"),
        ))
        .build()
        .await
        .expect("sdk");
    let request = DvmTradeTransitionProofEnqueueRequest::new(
        RadrootsActorContext::test(signer_pubkey.as_str(), [RadrootsActorRole::Service])
            .expect("service actor"),
        signer_pubkey.parse().expect("worker pubkey"),
        listing_address(),
        deterministic_event_id("listing-event"),
        deterministic_event_id("request-event"),
        deterministic_event_id("decision-event"),
        inventory_bins(),
        explicit_relays(),
    )
    .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_050));

    let receipt = sdk
        .dvm()
        .enqueue_trade_transition_proof_request(request)
        .await
        .expect("configured signer enqueue");

    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(
        outbox_operation_kind(&sdk, receipt.outbox_operation_id).await,
        DVM_TRADE_TRANSITION_PROOF_REQUEST_OPERATION_KIND
    );
}

#[cfg(feature = "signer-adapters")]
#[tokio::test]
async fn dvm_configured_enqueue_reports_prepare_and_target_errors_without_mutation() {
    let (tempdir, sdk, store) = directory_sdk_and_store_with_signer().await;
    let signer_keys = keys_from_secret(SERVICE_SECRET_KEY_HEX);
    let signer_pubkey = signer_keys.public_key().to_hex();
    let invalid = DvmTradeTransitionProofEnqueueRequest::new(
        RadrootsActorContext::test(signer_pubkey.as_str(), [RadrootsActorRole::Service])
            .expect("service actor"),
        signer_pubkey.parse().expect("worker pubkey"),
        listing_address(),
        deterministic_event_id("listing-event"),
        deterministic_event_id("request-event"),
        deterministic_event_id("decision-event"),
        Vec::new(),
        explicit_relays(),
    );
    let error = sdk
        .dvm()
        .enqueue_trade_transition_proof_request(invalid)
        .await
        .expect_err("prepare failure");
    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));

    let missing_relays = DvmTradeTransitionProofEnqueueRequest::new(
        RadrootsActorContext::test(signer_pubkey.as_str(), [RadrootsActorRole::Service])
            .expect("service actor"),
        signer_pubkey.parse().expect("worker pubkey"),
        listing_address(),
        deterministic_event_id("listing-event"),
        deterministic_event_id("request-event"),
        deterministic_event_id("decision-event"),
        inventory_bins(),
        TargetPolicy::default_profile(),
    );
    let error = sdk
        .dvm()
        .enqueue_trade_transition_proof_request(missing_relays)
        .await
        .expect_err("missing configured relays");
    assert!(matches!(
        error,
        RadrootsSdkError::EmptyTransportTargets { .. }
    ));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    drop(tempdir);
}

#[tokio::test]
async fn dvm_validation_receipt_ingest_commits_pending_trade_status() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-dvm-ingest", 10);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    let decision_event = signed_order_decision_event("order-dvm-ingest", &request_event_id, 11);
    let decision_event_id =
        RadrootsEventId::parse(decision_event.id.as_str()).expect("decision id");
    for (event, observed_at_ms) in [
        (request_event.clone(), 1_000),
        (decision_event.clone(), 1_100),
    ] {
        store
            .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
            .await
            .expect("ingest order event");
    }
    let pending = sdk
        .trades()
        .status(status_request("order-dvm-ingest"))
        .await
        .expect("pending status");
    assert_eq!(pending.status, TradeStatusKind::AgreedPendingRhi);
    assert!(pending.rhi_receipt_event_id.is_none());

    let listing_event_id = deterministic_event_id("listing-event");
    let receipt_event = signed_validation_receipt_event(
        "order-dvm-ingest",
        &listing_event_id,
        &request_event_id,
        &decision_event_id,
        12,
    );
    let receipt_event_id = RadrootsEventId::parse(receipt_event.id.as_str()).expect("receipt id");
    let ingest = sdk
        .dvm()
        .ingest_validation_receipt(
            DvmValidationReceiptIngestRequest::new(receipt_event)
                .with_expected_order_id(order_id("order-dvm-ingest"))
                .with_expected_listing_event_id(listing_event_id.clone())
                .with_expected_root_event_id(request_event_id.clone())
                .with_expected_target_event_id(decision_event_id.clone())
                .with_observed_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_060)),
        )
        .await
        .expect("ingest validation receipt");

    assert_eq!(ingest.receipt_event_id, receipt_event_id);
    assert_eq!(ingest.order_id.as_str(), "order-dvm-ingest");
    assert_eq!(ingest.listing_event_id, listing_event_id);
    assert_eq!(ingest.root_event_id, request_event_id);
    assert_eq!(ingest.target_event_id, decision_event_id);
    assert_eq!(
        ingest.receipt_type,
        RadrootsValidationReceiptType::TradeTransition
    );
    assert_eq!(ingest.result, RadrootsValidationReceiptResult::Valid);
    assert_eq!(
        ingest.proof_system,
        RadrootsValidationReceiptProofSystem::None
    );
    assert_eq!(ingest.validation_authority, None);
    assert_eq!(
        ingest.commitment_confidence,
        RadrootsTradeCommitmentConfidence::LocalOnly
    );
    assert_eq!(ingest.local_event_seq, 3);
    assert!(ingest.inserted);
    assert_eq!(ingest.refresh.validation_receipts, 1);
    assert_eq!(ingest.refresh.trade_upserts, 1);

    let committed = sdk
        .trades()
        .status(status_request("order-dvm-ingest"))
        .await
        .expect("committed status");
    assert_eq!(committed.status, TradeStatusKind::AgreedPendingRhi);
    assert!(!committed.lifecycle_terminal);
    assert_eq!(
        committed.next_action,
        TradeStatusNextActionKind::AwaitRhiValidation
    );
    assert_eq!(
        committed.rhi_receipt_event_id,
        Some(receipt_event_id.clone())
    );
    assert_eq!(committed.last_event_id, Some(decision_event_id));
    let trust = committed.validation_trust.expect("validation trust");
    assert_eq!(trust.state, RadrootsTradeValidationTrustState::Untrusted);
    assert_eq!(
        trust.reason_code.as_deref(),
        Some("validation_trust_policy_empty")
    );
    assert!(!trust.production_committed);
    assert_eq!(trust.proof_system.as_deref(), Some("none"));
}

#[tokio::test]
async fn dvm_validation_receipt_ingest_rejects_binding_mismatch_without_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let receipt_event = signed_validation_receipt_event(
        "order-dvm-mismatch",
        &deterministic_event_id("listing-event"),
        &deterministic_event_id("request-event"),
        &deterministic_event_id("decision-event"),
        12,
    );

    let error = sdk
        .dvm()
        .ingest_validation_receipt(
            DvmValidationReceiptIngestRequest::new(receipt_event)
                .with_expected_order_id(order_id("other-order")),
        )
        .await
        .expect_err("binding mismatch");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
}

#[tokio::test]
async fn dvm_trade_transition_proof_request_reports_prepare_and_target_errors_without_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let signer = FixtureSigner::new(SERVICE_SECRET_KEY_HEX);
    let actor = service_actor(&signer);
    let invalid = DvmTradeTransitionProofEnqueueRequest::new(
        actor.clone(),
        signer.pubkey().clone(),
        listing_address(),
        deterministic_event_id("listing-event"),
        deterministic_event_id("request-event"),
        deterministic_event_id("decision-event"),
        Vec::new(),
        explicit_relays(),
    );
    let error = sdk
        .dvm()
        .enqueue_trade_transition_proof_request_with_explicit_signer(invalid, &signer)
        .await
        .expect_err("prepare failure");
    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));

    let missing_relays = DvmTradeTransitionProofEnqueueRequest::new(
        actor,
        signer.pubkey().clone(),
        listing_address(),
        deterministic_event_id("listing-event"),
        deterministic_event_id("request-event"),
        deterministic_event_id("decision-event"),
        inventory_bins(),
        TargetPolicy::default_profile(),
    );
    let error = sdk
        .dvm()
        .enqueue_trade_transition_proof_request_with_explicit_signer(missing_relays, &signer)
        .await
        .expect_err("missing configured relays");
    assert!(matches!(
        error,
        RadrootsSdkError::EmptyTransportTargets { .. }
    ));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
}

#[tokio::test]
async fn dvm_validation_receipt_ingest_reports_timestamp_and_refresh_errors() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let overflow_observed = signed_validation_receipt_event(
        "order-dvm-observed-overflow",
        &deterministic_event_id("listing-event"),
        &deterministic_event_id("request-event"),
        &deterministic_event_id("decision-event"),
        12,
    );
    let error = sdk
        .dvm()
        .ingest_validation_receipt(
            DvmValidationReceiptIngestRequest::new(overflow_observed)
                .with_observed_at(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX)),
        )
        .await
        .expect_err("observed overflow");
    assert!(matches!(
        error,
        RadrootsSdkError::TimestampOutOfRange { value } if value == u64::MAX
    ));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );

    let (_tempdir, sdk, store) =
        directory_sdk_and_store_with_clock(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX)).await;
    let default_observed = signed_validation_receipt_event(
        "order-dvm-default-observed-overflow",
        &deterministic_event_id("listing-event"),
        &deterministic_event_id("request-event"),
        &deterministic_event_id("decision-event"),
        12,
    );
    let error = sdk
        .dvm()
        .ingest_validation_receipt(DvmValidationReceiptIngestRequest::new(default_observed))
        .await
        .expect_err("default observed overflow");
    assert!(matches!(
        error,
        RadrootsSdkError::TimestampOutOfRange { value } if value == u64::MAX
    ));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );

    let (_tempdir, sdk, store) =
        directory_sdk_and_store_with_clock(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX)).await;
    let refresh_overflow = signed_validation_receipt_event(
        "order-dvm-refresh-overflow",
        &deterministic_event_id("listing-event"),
        &deterministic_event_id("request-event"),
        &deterministic_event_id("decision-event"),
        12,
    );
    let error = sdk
        .dvm()
        .ingest_validation_receipt(
            DvmValidationReceiptIngestRequest::new(refresh_overflow)
                .with_observed_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000)),
        )
        .await
        .expect_err("refresh overflow");
    assert!(matches!(
        error,
        RadrootsSdkError::TimestampOutOfRange { value } if value == u64::MAX
    ));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        1
    );
}

#[tokio::test]
async fn dvm_prepare_and_ingest_use_sdk_clock_defaults() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let signer = FixtureSigner::new(SERVICE_SECRET_KEY_HEX);
    let plan = sdk
        .dvm()
        .prepare_trade_transition_proof_request(DvmTradeTransitionProofPrepareRequest::new(
            service_actor(&signer),
            signer.pubkey().clone(),
            listing_address(),
            deterministic_event_id("listing-event"),
            deterministic_event_id("request-event"),
            deterministic_event_id("decision-event"),
            inventory_bins(),
        ))
        .expect("default timestamp plan");
    assert_eq!(
        plan.created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000)
    );

    let receipt_event = signed_validation_receipt_event(
        "order-dvm-clock-default",
        &deterministic_event_id("listing-event"),
        &deterministic_event_id("request-event"),
        &deterministic_event_id("decision-event"),
        12,
    );
    let ingest = sdk
        .dvm()
        .ingest_validation_receipt(DvmValidationReceiptIngestRequest::new(receipt_event))
        .await
        .expect("default observed timestamp ingest");

    assert_eq!(ingest.local_event_seq, 1);
}

#[tokio::test]
async fn dvm_validation_receipt_ingest_rejects_invalid_verified_order_id() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let receipt_event = signed_validation_receipt_event(
        "bad order id",
        &deterministic_event_id("listing-event"),
        &deterministic_event_id("request-event"),
        &deterministic_event_id("decision-event"),
        12,
    );

    let error = sdk
        .dvm()
        .ingest_validation_receipt(DvmValidationReceiptIngestRequest::new(receipt_event))
        .await
        .expect_err("invalid order id");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
}

#[tokio::test]
async fn dvm_validation_receipt_ingest_rejects_invalid_receipt_event_id() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let mut receipt_event = signed_validation_receipt_event(
        "order-dvm-bad-receipt-id",
        &deterministic_event_id("listing-event"),
        &deterministic_event_id("request-event"),
        &deterministic_event_id("decision-event"),
        12,
    );
    receipt_event.id = "bad id".to_owned();

    let error = sdk
        .dvm()
        .ingest_validation_receipt(DvmValidationReceiptIngestRequest::new(receipt_event))
        .await
        .expect_err("invalid receipt event id");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
}

async fn directory_sdk_and_store() -> (tempfile::TempDir, RadrootsClient, RadrootsEventStore) {
    directory_sdk_and_store_with_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000)).await
}

async fn directory_sdk_and_store_with_clock(
    timestamp: RadrootsSdkTimestamp,
) -> (tempfile::TempDir, RadrootsClient, RadrootsEventStore) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsClient::builder()
        .storage(RadrootsSdkStorageConfig::Directory(
            tempdir.path().join("sdk"),
        ))
        .fixed_clock(timestamp)
        .build()
        .await
        .expect("sdk");
    let store =
        RadrootsEventStore::open_file(&sdk.storage_paths().expect("paths").event_store_path)
            .await
            .expect("event store");
    (tempdir, sdk, store)
}

#[cfg(feature = "signer-adapters")]
async fn directory_sdk_and_store_with_signer()
-> (tempfile::TempDir, RadrootsClient, RadrootsEventStore) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let signer_keys = keys_from_secret(SERVICE_SECRET_KEY_HEX);
    let sdk = RadrootsClient::builder()
        .storage(RadrootsSdkStorageConfig::Directory(
            tempdir.path().join("sdk"),
        ))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::new(signer_keys).expect("local signer"),
        ))
        .build()
        .await
        .expect("sdk");
    let store =
        RadrootsEventStore::open_file(&sdk.storage_paths().expect("paths").event_store_path)
            .await
            .expect("event store");
    (tempdir, sdk, store)
}

fn service_actor(signer: &FixtureSigner) -> RadrootsActorContext {
    RadrootsActorContext::test(signer.pubkey().as_str(), [RadrootsActorRole::Service])
        .expect("service actor")
}

fn explicit_relays() -> TargetPolicy {
    TargetPolicy::try_nostr_relays([RELAY], NostrRelayUrlPolicy::Public).expect("relay targets")
}

fn inventory_bins() -> Vec<RadrootsTradeInventoryBinWitnessDto> {
    vec![RadrootsTradeInventoryBinWitnessDto {
        bin_id: RadrootsInventoryBinId::parse("bin-1").expect("bin id"),
        listing_capacity: 5,
        previous_reserved: 1,
    }]
}

fn order_id(raw: &str) -> RadrootsOrderId {
    RadrootsOrderId::parse(raw).expect("order id")
}

fn status_request(raw: &str) -> TradeStatusRequest {
    TradeStatusRequest::parse(raw).expect("status request")
}

fn listing_address() -> RadrootsListingAddress {
    RadrootsListingAddress::parse(format!(
        "{KIND_LISTING}:{SELLER_PUBLIC_KEY_HEX}:AAAAAAAAAAAAAAAAAAAAAg"
    ))
    .expect("listing address")
}

fn listing_event_ptr() -> RadrootsEventPtr {
    RadrootsEventPtr {
        id: deterministic_event_id("listing-event").into_string(),
        relays: Some(RELAY.to_owned()),
    }
}

fn deterministic_event_id(raw: &str) -> RadrootsEventId {
    let mut bytes = [0u8; 32];
    for (index, byte) in raw.bytes().enumerate() {
        let primary = index % bytes.len();
        let secondary = (index * 7 + 13) % bytes.len();
        bytes[primary] = bytes[primary]
            .wrapping_add(byte)
            .wrapping_add((index as u8).wrapping_mul(31));
        bytes[secondary] ^= byte.rotate_left((index % 8) as u32);
    }
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        use core::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").expect("write hex");
    }
    RadrootsEventId::parse(hex).expect("event id")
}

fn decimal(raw: &str) -> RadrootsCoreDecimal {
    raw.parse().expect("decimal")
}

fn usd(raw: &str) -> RadrootsCoreMoney {
    RadrootsCoreMoney::new(decimal(raw), RadrootsCoreCurrency::USD)
}

fn economics() -> RadrootsOrderEconomics {
    RadrootsOrderEconomics {
        quote_id: "quote-1".parse().expect("quote id"),
        quote_version: 1,
        pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
        currency: RadrootsCoreCurrency::USD,
        items: vec![RadrootsOrderEconomicItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 2,
            quantity_amount: decimal("1"),
            quantity_unit: RadrootsCoreUnit::Each,
            unit_price_amount: decimal("5"),
            unit_price_currency: RadrootsCoreCurrency::USD,
            line_subtotal: usd("10"),
        }],
        discounts: Vec::<RadrootsOrderEconomicLine>::new(),
        adjustments: Vec::<RadrootsOrderEconomicLine>::new(),
        subtotal: usd("10"),
        discount_total: usd("0"),
        adjustment_total: usd("0"),
        total: usd("10"),
    }
}

fn order_request(raw_order_id: &str) -> RadrootsOrderRequest {
    RadrootsOrderRequest {
        order_id: order_id(raw_order_id),
        listing_addr: listing_address(),
        buyer_pubkey: BUYER_PUBLIC_KEY_HEX.parse().expect("buyer pubkey"),
        seller_pubkey: SELLER_PUBLIC_KEY_HEX.parse().expect("seller pubkey"),
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 2,
        }],
        economics: economics(),
    }
}

fn order_decision(raw_order_id: &str) -> RadrootsOrderDecision {
    RadrootsOrderDecision {
        order_id: order_id(raw_order_id),
        listing_addr: listing_address(),
        buyer_pubkey: BUYER_PUBLIC_KEY_HEX.parse().expect("buyer pubkey"),
        seller_pubkey: SELLER_PUBLIC_KEY_HEX.parse().expect("seller pubkey"),
        decision: RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                bin_id: "bin-1".parse().expect("bin id"),
                bin_count: 2,
            }],
        },
    }
}

fn signed_order_request_event(raw_order_id: &str, created_at: u32) -> RadrootsEventEnvelope {
    let draft = radroots_events_codec::order::order_request_event_build(
        &listing_event_ptr(),
        &order_request(raw_order_id),
    )
    .expect("request draft");
    signed_event(BUYER_SECRET_KEY_HEX, created_at, draft)
}

fn signed_order_decision_event(
    raw_order_id: &str,
    root_event_id: &RadrootsEventId,
    created_at: u32,
) -> RadrootsEventEnvelope {
    let draft = radroots_events_codec::order::order_decision_event_build(
        root_event_id,
        root_event_id,
        &order_decision(raw_order_id),
    )
    .expect("decision draft");
    signed_event(SELLER_SECRET_KEY_HEX, created_at, draft)
}

fn signed_validation_receipt_event(
    raw_order_id: &str,
    listing_event_id: &RadrootsEventId,
    root_event_id: &RadrootsEventId,
    target_event_id: &RadrootsEventId,
    created_at: u32,
) -> RadrootsEventEnvelope {
    let receipt = RadrootsTradeValidationReceipt {
        changed_records_root: hash32('6'),
        domain: "radroots.receipt".to_owned(),
        error_bitmap: "0x00000000000000000000000000000000".to_owned(),
        event_set_root: hash32('c'),
        new_state_root: hash32('4'),
        previous_state_root: hash32('3'),
        proof: RadrootsValidationReceiptProof {
            inline_proof_base64: None,
            mode: None,
            program_hash: None,
            proof_reference: None,
            system: RadrootsValidationReceiptProofSystem::None,
            verifying_key_hash: None,
        },
        public_values_hash: validation_receipt_public_values_hash_hex(br#"{"schema_version":1}"#),
        receipt_type: RadrootsValidationReceiptType::TradeTransition,
        result: RadrootsValidationReceiptResult::Valid,
        statement: RadrootsValidationReceiptStatement {
            listing_event_id: listing_event_id.as_str().to_owned(),
            root_event_id: root_event_id.as_str().to_owned(),
            target_event_id: target_event_id.as_str().to_owned(),
            statement_type: RadrootsValidationReceiptType::TradeTransition,
        },
        version: 1,
    };
    let parts = validation_receipt_event_build(raw_order_id, &receipt).expect("receipt event");
    signed_event(SERVICE_SECRET_KEY_HEX, created_at, parts)
}

fn signed_event(
    secret_key_hex: &str,
    created_at: u32,
    parts: radroots_events_codec::wire::WireEventParts,
) -> RadrootsEventEnvelope {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    let keys = RadrootsNostrKeys::new(secret_key);
    let event = radroots_nostr_build_event(parts.kind, parts.content, parts.tags)
        .expect("event builder")
        .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
        .sign_with_keys(&keys)
        .expect("signed event");
    radroots_event_from_nostr(&event)
}

#[cfg(feature = "signer-adapters")]
fn keys_from_secret(secret_key_hex: &str) -> RadrootsNostrKeys {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    RadrootsNostrKeys::new(secret_key)
}

fn hash32(ch: char) -> String {
    format!("0x{}", ch.to_string().repeat(64))
}

async fn outbox_operation_kind(sdk: &RadrootsClient, operation_id: i64) -> String {
    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    outbox
        .get_operation(operation_id)
        .await
        .expect("outbox operation")
        .expect("outbox operation")
        .operation_kind
}
