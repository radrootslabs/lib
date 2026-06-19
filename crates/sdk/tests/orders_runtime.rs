#![cfg(feature = "runtime")]

use radroots_authority::{
    RadrootsActorContext, RadrootsEventSigner, RadrootsSignerError, RadrootsSignerIdentity,
};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
};
use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
use radroots_events::{
    RadrootsNostrEvent,
    contract::RadrootsActorRole,
    draft::{RadrootsFrozenEventDraft, RadrootsSignedNostrEvent},
    ids::{RadrootsEventId, RadrootsOrderId},
    kinds::{KIND_LISTING, KIND_ORDER_DECISION, KIND_ORDER_REQUEST},
};
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_event, radroots_nostr_sign_frozen_draft,
};
use radroots_outbox::{RadrootsOutbox, RadrootsOutboxEventState};
use radroots_relay_transport::RadrootsMockRelayPublishAdapter;
use radroots_sdk::protocol::events::RadrootsNostrEventPtr;
use radroots_sdk::protocol::order::{
    RadrootsListingAddress, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
    RadrootsOrderEconomicItem, RadrootsOrderEconomicLine, RadrootsOrderEconomics,
    RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderPricingBasis,
    RadrootsOrderRequest,
};
use radroots_sdk::protocol::wire::WireEventParts;
use radroots_sdk::{
    ORDER_DECISION_OPERATION_KIND, ORDER_STATUS_DEFAULT_LIMIT, ORDER_STATUS_MAX_LIMIT,
    ORDER_SUBMIT_OPERATION_KIND, OrderDecisionEnqueueRequest, OrderDecisionPrepareRequest,
    OrderPaymentStateKind, OrderRequestEvidenceIngestRequest, OrderSettlementStateKind,
    OrderStatusKind, OrderStatusRequest, OrderSubmitEnqueueRequest, OrderSubmitPrepareRequest,
    PushOutboxEventState, PushOutboxRelayOutcomeKind, PushOutboxRequest, RadrootsSdk,
    RadrootsSdkError, RadrootsSdkPartialLocalMutationFailure, RadrootsSdkRecoveryAction,
    RadrootsSdkTimestamp, SdkMutationState, SdkOrderStatusIssue, SdkOrderStatusIssueKind,
    SdkOrderStatusSource, SdkRelayTargetPolicy, SdkRelayTargetSet, SdkRelayUrlPolicy,
};

const BUYER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const BUYER_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const SELLER_SECRET_KEY_HEX: &str =
    "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";
const SELLER_PUBLIC_KEY_HEX: &str =
    "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";
const OTHER_PUBLIC_KEY_HEX: &str =
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const RELAY: &str = "wss://relay.radroots.test";
const RELAY_B: &str = "wss://relay-b.radroots.test";

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
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &RadrootsFrozenEventDraft,
    ) -> Result<RadrootsSignedNostrEvent, RadrootsSignerError> {
        radroots_nostr_sign_frozen_draft(&self.keys, draft).map_err(|error| {
            RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            }
        })
    }
}

async fn directory_sdk_and_store() -> (tempfile::TempDir, RadrootsSdk, RadrootsEventStore) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsSdk::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .build()
        .await
        .expect("sdk");
    let store =
        RadrootsEventStore::open_file(&sdk.storage_paths().expect("paths").event_store_path)
            .await
            .expect("event store");
    (tempdir, sdk, store)
}

fn order_id(raw: &str) -> RadrootsOrderId {
    RadrootsOrderId::parse(raw).expect("order id")
}

fn status_request(raw: &str) -> OrderStatusRequest {
    OrderStatusRequest::parse(raw).expect("order status request")
}

fn buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(BUYER_PUBLIC_KEY_HEX, [RadrootsActorRole::Buyer]).expect("actor")
}

fn seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER_PUBLIC_KEY_HEX, [RadrootsActorRole::Seller]).expect("actor")
}

fn other_buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(OTHER_PUBLIC_KEY_HEX, [RadrootsActorRole::Buyer]).expect("actor")
}

fn other_seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(OTHER_PUBLIC_KEY_HEX, [RadrootsActorRole::Seller]).expect("actor")
}

fn non_buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(BUYER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer]).expect("actor")
}

fn non_seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER_PUBLIC_KEY_HEX, [RadrootsActorRole::Buyer]).expect("actor")
}

fn listing_address() -> RadrootsListingAddress {
    RadrootsListingAddress::parse(format!(
        "{KIND_LISTING}:{SELLER_PUBLIC_KEY_HEX}:AAAAAAAAAAAAAAAAAAAAAg"
    ))
    .expect("listing address")
}

fn listing_event_ptr() -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
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

fn invalid_listing_event_ptr() -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: String::new(),
        relays: Some(RELAY.to_owned()),
    }
}

#[tokio::test]
async fn order_submit_prepare_is_side_effect_free() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let listing_event = listing_event_ptr();
    let request = OrderSubmitPrepareRequest::new(
        buyer_actor(),
        listing_event.clone(),
        order_request("order-submit-prepare"),
    );

    let prepared = sdk.orders().prepare_submit(request).expect("prepared");

    assert_eq!(prepared.order_id.as_str(), "order-submit-prepare");
    assert_eq!(prepared.listing_addr, listing_address());
    assert_eq!(
        prepared.listing_event_id.as_str(),
        listing_event.id.as_str()
    );
    assert_eq!(prepared.frozen_draft.kind, KIND_ORDER_REQUEST);
    assert_eq!(prepared.created_at.unix_seconds(), 1_700_000_000);
    assert_eq!(
        prepared.expected_event_id,
        prepared.frozen_draft.expected_event_id
    );
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    assert!(
        store
            .get_event(prepared.expected_event_id.as_str())
            .await
            .expect("event lookup")
            .is_none()
    );

    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert!(
        outbox
            .claim_next_ready_event("worker", "claim", 2_000, 1_700_000_000_000)
            .await
            .expect("claim")
            .is_none()
    );
}

#[tokio::test]
async fn order_submit_prepare_rejects_missing_listing_evidence() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request = OrderSubmitPrepareRequest::new(
        buyer_actor(),
        invalid_listing_event_ptr(),
        order_request("order-submit-missing-listing"),
    );

    let error = sdk
        .orders()
        .prepare_submit(request)
        .expect_err("missing listing evidence");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
}

#[tokio::test]
async fn order_submit_prepare_rejects_invalid_actor_or_payload() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;

    let non_buyer = sdk
        .orders()
        .prepare_submit(OrderSubmitPrepareRequest::new(
            non_buyer_actor(),
            listing_event_ptr(),
            order_request("order-submit-non-buyer"),
        ))
        .expect_err("non buyer");
    assert!(matches!(
        non_buyer,
        RadrootsSdkError::UnauthorizedActor { .. }
    ));

    let wrong_actor = sdk
        .orders()
        .prepare_submit(OrderSubmitPrepareRequest::new(
            other_buyer_actor(),
            listing_event_ptr(),
            order_request("order-submit-wrong-actor"),
        ))
        .expect_err("wrong actor");
    assert!(matches!(
        wrong_actor,
        RadrootsSdkError::UnauthorizedActor { .. }
    ));

    let mut seller_mismatch = order_request("order-submit-seller-mismatch");
    seller_mismatch.seller_pubkey = OTHER_PUBLIC_KEY_HEX.parse().expect("seller pubkey");
    let seller_error = sdk
        .orders()
        .prepare_submit(OrderSubmitPrepareRequest::new(
            buyer_actor(),
            listing_event_ptr(),
            seller_mismatch,
        ))
        .expect_err("seller mismatch");
    assert!(matches!(
        seller_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let mut empty_items = order_request("order-submit-empty-items");
    empty_items.items.clear();
    let empty_items_error = sdk
        .orders()
        .prepare_submit(OrderSubmitPrepareRequest::new(
            buyer_actor(),
            listing_event_ptr(),
            empty_items,
        ))
        .expect_err("empty items");
    assert!(matches!(
        empty_items_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let mut empty_economics = order_request("order-submit-empty-economics");
    empty_economics.economics.items.clear();
    let empty_economics_error = sdk
        .orders()
        .prepare_submit(OrderSubmitPrepareRequest::new(
            buyer_actor(),
            listing_event_ptr(),
            empty_economics,
        ))
        .expect_err("empty economics");
    assert!(matches!(
        empty_economics_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[tokio::test]
async fn order_submit_enqueue_stores_event_queues_outbox_and_status_sees_request() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let order = order_request("order-submit-enqueue");
    let prepared = sdk
        .orders()
        .prepare_submit(OrderSubmitPrepareRequest::new(
            buyer_actor(),
            listing_event_ptr(),
            order.clone(),
        ))
        .expect("prepared");
    let request = OrderSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order,
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays")
    .try_with_idempotency_key("order-submit-enqueue-idempotency")
    .expect("idempotency key");

    let receipt = sdk
        .orders()
        .enqueue_submit(request, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("enqueue");

    assert_eq!(receipt.order_id, prepared.order_id);
    assert_eq!(receipt.listing_addr, prepared.listing_addr);
    assert_eq!(receipt.listing_event_id, prepared.listing_event_id);
    assert_eq!(receipt.expected_event_id, prepared.expected_event_id);
    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.local_event_seq, 1);
    assert_eq!(receipt.outbox_operation_id, 1);
    assert_eq!(receipt.outbox_event_id, 1);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
    assert!(receipt.idempotency_digest_prefix.is_some());

    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        1
    );
    let stored_event = store
        .get_event(receipt.signed_event_id.as_str())
        .await
        .expect("event lookup")
        .expect("stored event");
    assert_eq!(stored_event.kind, KIND_ORDER_REQUEST);
    assert_eq!(
        stored_event.contract_id.as_deref(),
        Some("radroots.order.request.v1")
    );

    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let outbox_event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("outbox event")
        .expect("outbox event");
    assert_eq!(outbox_event.state, RadrootsOutboxEventState::Signed);
    assert_eq!(outbox_event.draft.kind, KIND_ORDER_REQUEST);
    assert!(outbox_event.signed_event.is_some());

    let status = sdk
        .orders()
        .status(status_request("order-submit-enqueue"))
        .await
        .expect("status");
    assert!(status.found);
    assert_eq!(status.status, OrderStatusKind::Requested);
    assert_eq!(status.event_count, 1);
    assert_eq!(
        status
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(receipt.signed_event_id.as_str())
    );
}

#[tokio::test]
async fn order_submit_enqueue_returns_sanitized_signer_errors_before_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request = OrderSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-wrong-signer"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let error = sdk
        .orders()
        .enqueue_submit(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect_err("signer error");
    let message = error.to_string();

    assert!(matches!(
        error,
        RadrootsSdkError::SignerPubkeyMismatch { .. }
    ));
    assert!(!message.contains("raw"));
    assert!(!message.contains("ffff"));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );

    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert!(
        outbox
            .claim_next_ready_event("worker", "claim", 2_000, 1_700_000_000_000)
            .await
            .expect("claim")
            .is_none()
    );
}

#[tokio::test]
async fn order_submit_enqueue_derives_order_independent_idempotency_key() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let first = OrderSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-idempotent"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY_B, RELAY, RELAY], SdkRelayUrlPolicy::Public)
    .expect("first target relays");
    let second = OrderSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-idempotent"),
        SdkRelayTargetPolicy::explicit(
            SdkRelayTargetSet::new([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
                .expect("second target relays"),
        ),
    );

    let first_receipt = sdk
        .orders()
        .enqueue_submit(first, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("first enqueue");
    let second_receipt = sdk
        .orders()
        .enqueue_submit(second, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("second enqueue");

    assert_eq!(
        first_receipt.outbox_event_id,
        second_receipt.outbox_event_id
    );
    assert_eq!(
        first_receipt.idempotency_digest_prefix,
        second_receipt.idempotency_digest_prefix
    );
    assert_eq!(second_receipt.state, SdkMutationState::AlreadyQueued);

    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let relay_urls = outbox
        .relay_statuses(first_receipt.outbox_event_id)
        .await
        .expect("relay statuses")
        .into_iter()
        .map(|status| status.relay_url)
        .collect::<Vec<_>>();
    assert_eq!(relay_urls, vec![RELAY_B.to_owned(), RELAY.to_owned()]);
}

#[tokio::test]
async fn order_submit_enqueue_pushes_queued_event_with_mock_relay_sync() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let enqueue_request = OrderSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-sync"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");
    let enqueue_receipt = sdk
        .orders()
        .enqueue_submit(enqueue_request, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("enqueue");
    let adapter = RadrootsMockRelayPublishAdapter::new();

    let push_receipt = sdk
        .sync()
        .push_outbox_with_adapter(&adapter, PushOutboxRequest::new().with_limit(1))
        .await
        .expect("push");

    assert_eq!(push_receipt.attempted_events, 1);
    assert_eq!(push_receipt.published_events, 1);
    assert_eq!(push_receipt.retryable_events, 0);
    assert_eq!(push_receipt.terminal_events, 0);
    assert_eq!(push_receipt.events.len(), 1);
    let event = &push_receipt.events[0];
    assert_eq!(event.event_id, enqueue_receipt.signed_event_id);
    assert_eq!(event.outbox_event_id, enqueue_receipt.outbox_event_id);
    assert_eq!(event.final_state, PushOutboxEventState::Published);
    assert_eq!(event.attempted_count, 1);
    assert_eq!(event.accepted_count, 1);
    assert_eq!(event.retryable_count, 0);
    assert_eq!(event.terminal_count, 0);
    assert_eq!(event.quorum, 1);
    assert!(event.quorum_met);
    assert_eq!(event.relays.len(), 1);
    assert_eq!(event.relays[0].relay_url, RELAY);
    assert_eq!(
        event.relays[0].outcome_kind,
        PushOutboxRelayOutcomeKind::Accepted
    );
    assert_eq!(adapter.captured_raw_events().len(), 1);

    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
        .await
        .expect("outbox");
    let stored = outbox
        .get_event(enqueue_receipt.outbox_event_id)
        .await
        .expect("stored")
        .expect("stored");
    assert_eq!(stored.state, RadrootsOutboxEventState::Published);
}

#[tokio::test]
async fn order_submit_enqueue_reports_partial_local_mutation_after_outbox_conflict() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let first = OrderSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-conflict-a"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("first target relays")
    .try_with_idempotency_key("order-submit-conflict-idempotency")
    .expect("first idempotency key");
    sdk.orders()
        .enqueue_submit(first, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("first enqueue");

    let second = OrderSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-conflict-b"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("second target relays")
    .try_with_idempotency_key("order-submit-conflict-idempotency")
    .expect("second idempotency key");
    let error = sdk
        .orders()
        .enqueue_submit(second, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect_err("partial");

    assert!(matches!(
        error,
        RadrootsSdkError::PartialLocalMutation(ref partial)
            if partial.stored
                && !partial.queued
                && partial.event_id.is_some()
                && partial.operation_kind == ORDER_SUBMIT_OPERATION_KIND
                && partial.idempotency_digest_prefix.is_some()
                && partial.failure == RadrootsSdkPartialLocalMutationFailure::OutboxIdempotencyConflict
                && partial.recovery == RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey
    ));
    assert!(
        !error
            .to_string()
            .contains("order-submit-conflict-idempotency")
    );
}

#[tokio::test]
async fn order_submit_runtime_dtos_serialize_deterministically() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_123);
    let prepare_request = OrderSubmitPrepareRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-serialized"),
    )
    .with_created_at(created_at);
    let prepare_json = serde_json::to_value(&prepare_request).expect("prepare request json");

    assert_eq!(
        prepare_json["actor"],
        serde_json::json!({
            "pubkey": BUYER_PUBLIC_KEY_HEX,
            "roles": ["buyer"],
            "account_id": null,
            "source": "test"
        })
    );
    assert_eq!(
        prepare_json["listing_event"],
        serde_json::json!({
            "id": deterministic_event_id("listing-event").as_str(),
            "relays": RELAY
        })
    );
    assert_eq!(prepare_json["order"]["order_id"], "order-submit-serialized");
    assert_eq!(
        prepare_json["order"]["listing_addr"],
        listing_address().as_str()
    );
    assert_eq!(prepare_json["order"]["buyer_pubkey"], BUYER_PUBLIC_KEY_HEX);
    assert_eq!(
        prepare_json["order"]["seller_pubkey"],
        SELLER_PUBLIC_KEY_HEX
    );
    assert_eq!(prepare_json["order"]["items"][0]["bin_id"], "bin-1");
    assert_eq!(prepare_json["order"]["items"][0]["bin_count"], 2);
    assert_eq!(prepare_json["created_at"], 1_700_000_123);

    let enqueue_request = OrderSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-serialized-enqueue"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("relay targets")
    .try_with_idempotency_key("order-serialized-idempotency")
    .expect("idempotency")
    .with_created_at(created_at);
    let enqueue_json = serde_json::to_value(&enqueue_request).expect("enqueue request json");

    assert_eq!(
        enqueue_json["target_relays"],
        serde_json::json!({
            "kind": "explicit",
            "relays": [RELAY, RELAY_B],
            "canonical_relays": [RELAY_B, RELAY]
        })
    );
    assert_eq!(
        enqueue_json["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 28 })
    );
    assert_eq!(enqueue_json["created_at"], 1_700_000_123);
    assert!(
        !enqueue_json
            .to_string()
            .contains("order-serialized-idempotency")
    );

    let receipt = sdk
        .orders()
        .enqueue_submit(enqueue_request, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("enqueue");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(
        receipt_json,
        serde_json::json!({
            "order_id": receipt.order_id.as_str(),
            "listing_addr": receipt.listing_addr.as_str(),
            "listing_event_id": receipt.listing_event_id.as_str(),
            "expected_event_id": receipt.expected_event_id.as_str(),
            "signed_event_id": receipt.signed_event_id.as_str(),
            "local_event_seq": 1,
            "outbox_operation_id": 1,
            "outbox_event_id": 1,
            "state": "stored_and_queued",
            "idempotency_digest_prefix": receipt.idempotency_digest_prefix.as_deref()
        })
    );
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

fn signed_event(
    secret_key_hex: &str,
    created_at: u32,
    parts: WireEventParts,
) -> RadrootsNostrEvent {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    let keys = RadrootsNostrKeys::new(secret_key);
    let event = radroots_nostr_build_event(parts.kind, parts.content, parts.tags)
        .expect("event builder")
        .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
        .sign_with_keys(&keys)
        .expect("signed event");
    radroots_event_from_nostr(&event)
}

fn signed_order_request_event(raw_order_id: &str, created_at: u32) -> RadrootsNostrEvent {
    let draft = radroots_sdk::protocol::order::build_order_request_draft(
        &listing_event_ptr(),
        &order_request(raw_order_id),
    )
    .expect("request draft");
    signed_event(BUYER_SECRET_KEY_HEX, created_at, draft.into_wire_parts())
}

fn request_event_ptr(event: &RadrootsNostrEvent) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: event.id.clone(),
        relays: Some(RELAY.to_owned()),
    }
}

fn signed_order_decision_event(
    raw_order_id: &str,
    root_event_id: &RadrootsEventId,
    created_at: u32,
) -> RadrootsNostrEvent {
    let draft = radroots_sdk::protocol::order::build_order_decision_draft(
        root_event_id,
        root_event_id,
        &order_decision(raw_order_id),
    )
    .expect("decision draft");
    signed_event(SELLER_SECRET_KEY_HEX, created_at, draft.into_wire_parts())
}

#[tokio::test]
async fn order_request_evidence_ingest_stores_request_and_enables_decision_enqueue() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-decision-ingested", 39);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    let ingest_request = OrderRequestEvidenceIngestRequest::new(request_event.clone())
        .with_observed_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_039));

    let ingest_receipt = sdk
        .orders()
        .ingest_request_evidence(ingest_request)
        .await
        .expect("ingest request evidence");

    assert_eq!(ingest_receipt.order_id.as_str(), "order-decision-ingested");
    assert_eq!(ingest_receipt.listing_addr, listing_address());
    assert_eq!(ingest_receipt.buyer_pubkey.as_str(), BUYER_PUBLIC_KEY_HEX);
    assert_eq!(ingest_receipt.seller_pubkey.as_str(), SELLER_PUBLIC_KEY_HEX);
    assert_eq!(ingest_receipt.request_event_id, request_event_id);
    assert_eq!(ingest_receipt.local_event_seq, 1);
    assert!(ingest_receipt.inserted);

    let request = OrderDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        order_decision("order-decision-ingested"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");
    let receipt = sdk
        .orders()
        .enqueue_decision(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect("enqueue decision");

    assert_eq!(receipt.local_event_seq, 2);
    let duplicate_receipt = sdk
        .orders()
        .ingest_request_evidence(OrderRequestEvidenceIngestRequest::new(
            request_event.clone(),
        ))
        .await
        .expect("duplicate request evidence");
    assert_eq!(duplicate_receipt.local_event_seq, 1);
    assert!(!duplicate_receipt.inserted);
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        2
    );
}

#[tokio::test]
async fn order_request_evidence_ingest_rejects_non_request_events() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let root_event_id = deterministic_event_id("non-request-root");
    let decision_event = signed_order_decision_event("non-request-root", &root_event_id, 40);

    let error = sdk
        .orders()
        .ingest_request_evidence(OrderRequestEvidenceIngestRequest::new(decision_event))
        .await
        .expect_err("non request event");

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
async fn order_decision_prepare_accept_and_decline_are_side_effect_free() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event_id = deterministic_event_id("order-decision-prepare-request");
    let request_event = RadrootsNostrEventPtr {
        id: request_event_id.as_str().to_owned(),
        relays: Some(RELAY.to_owned()),
    };
    let accepted_request = OrderDecisionPrepareRequest::new(
        seller_actor(),
        request_event.clone(),
        order_decision("order-decision-prepare-accept"),
    );

    let accepted = sdk
        .orders()
        .prepare_decision(accepted_request)
        .expect("accepted plan");

    assert_eq!(accepted.order_id.as_str(), "order-decision-prepare-accept");
    assert_eq!(accepted.listing_addr, listing_address());
    assert_eq!(accepted.buyer_pubkey.as_str(), BUYER_PUBLIC_KEY_HEX);
    assert_eq!(accepted.seller_pubkey.as_str(), SELLER_PUBLIC_KEY_HEX);
    assert_eq!(accepted.request_event_id, request_event_id);
    assert_eq!(accepted.frozen_draft.kind, KIND_ORDER_DECISION);
    assert_eq!(accepted.created_at.unix_seconds(), 1_700_000_000);
    assert_eq!(
        accepted.expected_event_id,
        accepted.frozen_draft.expected_event_id
    );

    let mut declined_payload = order_decision("order-decision-prepare-decline");
    declined_payload.decision = RadrootsOrderDecisionOutcome::Declined {
        reason: " out of stock ".to_owned(),
    };
    let declined = sdk
        .orders()
        .prepare_decision(OrderDecisionPrepareRequest::new(
            seller_actor(),
            request_event,
            declined_payload,
        ))
        .expect("declined plan");

    assert_eq!(declined.order_id.as_str(), "order-decision-prepare-decline");
    assert_eq!(declined.frozen_draft.kind, KIND_ORDER_DECISION);
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );

    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert!(
        outbox
            .claim_next_ready_event("worker", "claim", 2_000, 1_700_000_000_000)
            .await
            .expect("claim")
            .is_none()
    );
}

#[tokio::test]
async fn order_decision_prepare_rejects_invalid_actor_evidence_and_payload() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request_event = RadrootsNostrEventPtr {
        id: deterministic_event_id("order-decision-invalid-request")
            .as_str()
            .to_owned(),
        relays: Some(RELAY.to_owned()),
    };

    let non_seller = sdk
        .orders()
        .prepare_decision(OrderDecisionPrepareRequest::new(
            non_seller_actor(),
            request_event.clone(),
            order_decision("order-decision-non-seller"),
        ))
        .expect_err("non seller");
    assert!(matches!(
        non_seller,
        RadrootsSdkError::UnauthorizedActor { .. }
    ));

    let wrong_actor = sdk
        .orders()
        .prepare_decision(OrderDecisionPrepareRequest::new(
            other_seller_actor(),
            request_event.clone(),
            order_decision("order-decision-wrong-seller"),
        ))
        .expect_err("wrong seller");
    assert!(matches!(
        wrong_actor,
        RadrootsSdkError::UnauthorizedActor { .. }
    ));

    let invalid_evidence = sdk
        .orders()
        .prepare_decision(OrderDecisionPrepareRequest::new(
            seller_actor(),
            RadrootsNostrEventPtr {
                id: String::new(),
                relays: Some(RELAY.to_owned()),
            },
            order_decision("order-decision-invalid-evidence"),
        ))
        .expect_err("invalid evidence");
    assert!(matches!(
        invalid_evidence,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let mut empty_commitments = order_decision("order-decision-empty-commitments");
    empty_commitments.decision = RadrootsOrderDecisionOutcome::Accepted {
        inventory_commitments: Vec::new(),
    };
    let commitment_error = sdk
        .orders()
        .prepare_decision(OrderDecisionPrepareRequest::new(
            seller_actor(),
            request_event.clone(),
            empty_commitments,
        ))
        .expect_err("missing commitments");
    assert!(matches!(
        commitment_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let mut missing_reason = order_decision("order-decision-missing-reason");
    missing_reason.decision = RadrootsOrderDecisionOutcome::Declined {
        reason: " ".to_owned(),
    };
    let reason_error = sdk
        .orders()
        .prepare_decision(OrderDecisionPrepareRequest::new(
            seller_actor(),
            request_event,
            missing_reason,
        ))
        .expect_err("missing reason");
    assert!(matches!(
        reason_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[tokio::test]
async fn order_decision_runtime_dtos_serialize_deterministically() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_321);
    let prepare_event_id = deterministic_event_id("order-decision-serialized-request");
    let prepare_request = OrderDecisionPrepareRequest::new(
        seller_actor(),
        RadrootsNostrEventPtr {
            id: prepare_event_id.as_str().to_owned(),
            relays: Some(RELAY.to_owned()),
        },
        order_decision("order-decision-serialized"),
    )
    .with_created_at(created_at);
    let prepare_json = serde_json::to_value(&prepare_request).expect("prepare request json");

    assert_eq!(
        prepare_json["actor"],
        serde_json::json!({
            "pubkey": SELLER_PUBLIC_KEY_HEX,
            "roles": ["seller"],
            "account_id": null,
            "source": "test"
        })
    );
    assert_eq!(
        prepare_json["request_event"],
        serde_json::json!({
            "id": prepare_event_id.as_str(),
            "relays": RELAY
        })
    );
    assert_eq!(
        prepare_json["decision"]["order_id"],
        "order-decision-serialized"
    );
    assert_eq!(
        prepare_json["decision"]["seller_pubkey"],
        SELLER_PUBLIC_KEY_HEX
    );
    assert_eq!(prepare_json["created_at"], 1_700_000_321);

    let request_event = signed_order_request_event("order-decision-serialized-enqueue", 45);
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 4_500))
        .await
        .expect("ingest request");
    let enqueue_request = OrderDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        order_decision("order-decision-serialized-enqueue"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("target relays")
    .try_with_idempotency_key("order-decision-serialized-idempotency")
    .expect("idempotency")
    .with_created_at(created_at);
    let enqueue_json = serde_json::to_value(&enqueue_request).expect("enqueue request json");

    assert_eq!(
        enqueue_json["target_relays"],
        serde_json::json!({
            "kind": "explicit",
            "relays": [RELAY, RELAY_B],
            "canonical_relays": [RELAY_B, RELAY]
        })
    );
    assert_eq!(
        enqueue_json["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 37 })
    );
    assert_eq!(enqueue_json["created_at"], 1_700_000_321);
    assert!(
        !enqueue_json
            .to_string()
            .contains("order-decision-serialized-idempotency")
    );

    let receipt = sdk
        .orders()
        .enqueue_decision(enqueue_request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect("enqueue");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(
        receipt_json,
        serde_json::json!({
            "order_id": receipt.order_id.as_str(),
            "listing_addr": receipt.listing_addr.as_str(),
            "buyer_pubkey": BUYER_PUBLIC_KEY_HEX,
            "seller_pubkey": SELLER_PUBLIC_KEY_HEX,
            "request_event_id": request_event.id.as_str(),
            "expected_event_id": receipt.expected_event_id.as_str(),
            "signed_event_id": receipt.signed_event_id.as_str(),
            "local_event_seq": 2,
            "outbox_operation_id": 1,
            "outbox_event_id": 1,
            "state": "stored_and_queued",
            "idempotency_digest_prefix": receipt.idempotency_digest_prefix.as_deref()
        })
    );
}

#[tokio::test]
async fn order_decision_enqueue_accept_stores_event_queues_outbox_and_updates_status() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-decision-accept", 40);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 4_000))
        .await
        .expect("ingest request");
    let request = OrderDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        order_decision("order-decision-accept"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays")
    .try_with_idempotency_key("order-decision-accept-idempotency")
    .expect("idempotency");

    let receipt = sdk
        .orders()
        .enqueue_decision(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect("enqueue");

    assert_eq!(receipt.order_id.as_str(), "order-decision-accept");
    assert_eq!(receipt.listing_addr, listing_address());
    assert_eq!(receipt.buyer_pubkey.as_str(), BUYER_PUBLIC_KEY_HEX);
    assert_eq!(receipt.seller_pubkey.as_str(), SELLER_PUBLIC_KEY_HEX);
    assert_eq!(receipt.request_event_id, request_event_id);
    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.local_event_seq, 2);
    assert_eq!(receipt.outbox_operation_id, 1);
    assert_eq!(receipt.outbox_event_id, 1);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
    assert!(receipt.idempotency_digest_prefix.is_some());

    let stored_event = store
        .get_event(receipt.signed_event_id.as_str())
        .await
        .expect("event lookup")
        .expect("stored event");
    assert_eq!(stored_event.kind, KIND_ORDER_DECISION);
    assert_eq!(
        stored_event.contract_id.as_deref(),
        Some("radroots.order.decision.v1")
    );

    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    let operation = outbox
        .get_operation(receipt.outbox_operation_id)
        .await
        .expect("outbox operation")
        .expect("outbox operation");
    assert_eq!(operation.operation_kind, ORDER_DECISION_OPERATION_KIND);
    let outbox_event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("outbox event")
        .expect("outbox event");
    assert_eq!(outbox_event.state, RadrootsOutboxEventState::Signed);
    assert_eq!(outbox_event.draft.kind, KIND_ORDER_DECISION);
    assert!(outbox_event.signed_event.is_some());

    let status = sdk
        .orders()
        .status(status_request("order-decision-accept"))
        .await
        .expect("status");
    assert!(status.found);
    assert_eq!(status.status, OrderStatusKind::Accepted);
    assert_eq!(status.event_count, 2);
    assert_eq!(
        status
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(request_event.id.as_str())
    );
    assert_eq!(
        status
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(receipt.signed_event_id.as_str())
    );
    assert!(status.issues.is_empty());
}

#[tokio::test]
async fn order_decision_enqueue_decline_stores_event_and_status_sees_declined() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-decision-decline", 41);
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 4_100))
        .await
        .expect("ingest request");
    let mut decision = order_decision("order-decision-decline");
    decision.decision = RadrootsOrderDecisionOutcome::Declined {
        reason: " unavailable ".to_owned(),
    };
    let request = OrderDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        decision,
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let receipt = sdk
        .orders()
        .enqueue_decision(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect("enqueue");

    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
    let status = sdk
        .orders()
        .status(status_request("order-decision-decline"))
        .await
        .expect("status");
    assert_eq!(status.status, OrderStatusKind::Declined);
    assert_eq!(
        status
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(receipt.signed_event_id.as_str())
    );
    assert!(status.issues.is_empty());
}

#[tokio::test]
async fn order_decision_enqueue_rejects_missing_request_evidence_before_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let missing_request = RadrootsNostrEventPtr {
        id: deterministic_event_id("missing-order-request")
            .as_str()
            .to_owned(),
        relays: Some(RELAY.to_owned()),
    };
    let request = OrderDecisionEnqueueRequest::new(
        seller_actor(),
        missing_request,
        order_decision("order-decision-missing-request"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let error = sdk
        .orders()
        .enqueue_decision(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect_err("missing request evidence");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    let paths = sdk.storage_paths().expect("paths");
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path)
        .await
        .expect("outbox");
    assert!(
        outbox
            .claim_next_ready_event("worker", "claim", 2_000, 1_700_000_000_000)
            .await
            .expect("claim")
            .is_none()
    );
}

#[tokio::test]
async fn order_decision_enqueue_returns_sanitized_signer_errors_before_decision_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-decision-wrong-signer", 42);
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 4_200))
        .await
        .expect("ingest request");
    let request = OrderDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        order_decision("order-decision-wrong-signer"),
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let error = sdk
        .orders()
        .enqueue_decision(request, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect_err("signer error");
    let message = error.to_string();

    assert!(matches!(
        error,
        RadrootsSdkError::SignerPubkeyMismatch { .. }
    ));
    assert!(!message.contains("raw"));
    assert!(!message.contains("ffff"));
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
async fn order_decision_enqueue_rejects_existing_decision_state_before_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-decision-conflict", 43);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    let decision_event =
        signed_order_decision_event("order-decision-conflict", &request_event_id, 44);
    for (event, observed_at_ms) in [
        (request_event.clone(), 4_300),
        (decision_event.clone(), 4_400),
    ] {
        store
            .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
            .await
            .expect("ingest");
    }
    let mut decline = order_decision("order-decision-conflict");
    decline.decision = RadrootsOrderDecisionOutcome::Declined {
        reason: "too late".to_owned(),
    };
    let request = OrderDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        decline,
        SdkRelayTargetPolicy::UseConfiguredRelays,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let error = sdk
        .orders()
        .enqueue_decision(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect_err("existing decision");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        2
    );
    let status = sdk
        .orders()
        .status(status_request("order-decision-conflict"))
        .await
        .expect("status");
    assert_eq!(status.status, OrderStatusKind::Accepted);
    assert_eq!(
        status
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(decision_event.id.as_str())
    );
}

#[tokio::test]
async fn order_status_returns_not_found_for_missing_local_order() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request = status_request("order-1");

    assert_eq!(request.limit, ORDER_STATUS_DEFAULT_LIMIT);

    let receipt = sdk.orders().status(request).await.expect("status");

    assert!(!receipt.found);
    assert_eq!(receipt.order_id.as_str(), "order-1");
    assert_eq!(receipt.source, SdkOrderStatusSource::LocalEventStore);
    assert_eq!(receipt.event_count, 0);
    assert_eq!(receipt.limit_applied, ORDER_STATUS_DEFAULT_LIMIT);
    assert!(receipt.event_ids.is_empty());
    assert_eq!(receipt.status, OrderStatusKind::Missing);
    assert_eq!(receipt.payment_state, OrderPaymentStateKind::NotRecorded);
    assert_eq!(
        receipt.settlement_state,
        OrderSettlementStateKind::NotRequired
    );
    assert!(receipt.issues.is_empty());
}

#[tokio::test]
async fn order_status_rejects_invalid_limits_before_querying() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;

    let zero = sdk
        .orders()
        .status(status_request("order-1").with_limit(0))
        .await
        .expect_err("zero limit");
    let too_large = sdk
        .orders()
        .status(status_request("order-1").with_limit(ORDER_STATUS_MAX_LIMIT + 1))
        .await
        .expect_err("too large");

    assert!(matches!(
        zero,
        RadrootsSdkError::OrderStatusLimitInvalid {
            limit: 0,
            min: 1,
            max: ORDER_STATUS_MAX_LIMIT
        }
    ));
    assert!(matches!(
        too_large,
        RadrootsSdkError::OrderStatusLimitInvalid {
            limit,
            min: 1,
            max: ORDER_STATUS_MAX_LIMIT
        } if limit == ORDER_STATUS_MAX_LIMIT + 1
    ));
}

#[test]
fn order_status_parse_rejects_invalid_order_ids() {
    let error = OrderStatusRequest::parse("bad order id").expect_err("invalid order id");

    assert!(matches!(error, RadrootsSdkError::InvalidOrderId { .. }));
}

#[tokio::test]
async fn order_status_contract_dtos_serialize_deterministically() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request = status_request("order-1").with_limit(25);
    let request_json = serde_json::to_value(&request).expect("request json");

    assert_eq!(
        request_json,
        serde_json::json!({
            "order_id": "order-1",
            "limit": 25
        })
    );

    let receipt = sdk.orders().status(request).await.expect("status");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(receipt_json["source"], "local_event_store");
    assert_eq!(receipt_json["status"], "missing");
    assert_eq!(receipt_json["payment_state"], "not_recorded");
    assert_eq!(receipt_json["settlement_state"], "not_required");

    let issue = SdkOrderStatusIssue {
        kind: SdkOrderStatusIssueKind::DecisionPayloadInvalid,
        event_ids: vec![deterministic_event_id("issue-event")],
    };
    assert_eq!(issue.code(), "decision_payload_invalid");
    assert_eq!(
        serde_json::to_value(issue).expect("issue json"),
        serde_json::json!({
            "code": "decision_payload_invalid",
            "kind": "decision_payload_invalid",
            "event_ids": [deterministic_event_id("issue-event")]
        })
    );
}

#[tokio::test]
async fn order_status_projects_local_request_and_decision_events() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-1", 20);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    let decision_event = signed_order_decision_event("order-1", &request_event_id, 21);

    for (event, observed_at_ms) in [
        (request_event.clone(), 2_000),
        (decision_event.clone(), 2_100),
    ] {
        store
            .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
            .await
            .expect("ingest");
    }

    let receipt = sdk
        .orders()
        .status(status_request("order-1").with_limit(1_000))
        .await
        .expect("status");

    assert!(receipt.found);
    assert_eq!(receipt.order_id.as_str(), "order-1");
    assert_eq!(receipt.source, SdkOrderStatusSource::LocalEventStore);
    assert_eq!(receipt.event_count, 2);
    assert_eq!(receipt.limit_applied, 1_000);
    assert_eq!(
        receipt
            .event_ids
            .iter()
            .map(RadrootsEventId::as_str)
            .collect::<Vec<_>>(),
        vec![request_event.id.as_str(), decision_event.id.as_str()]
    );
    assert_eq!(receipt.status, OrderStatusKind::Accepted);
    assert_eq!(
        receipt
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(request_event.id.as_str())
    );
    assert_eq!(
        receipt
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(decision_event.id.as_str())
    );
    assert_eq!(
        receipt.last_event_id.as_ref().map(RadrootsEventId::as_str),
        Some(decision_event.id.as_str())
    );
    assert!(receipt.issues.is_empty());
    assert!(!receipt.lifecycle_terminal);
}

#[tokio::test]
async fn order_status_reports_limited_local_results() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-1", 25);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    let decision_event = signed_order_decision_event("order-1", &request_event_id, 26);

    for (event, observed_at_ms) in [(request_event.clone(), 2_500), (decision_event, 2_600)] {
        store
            .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
            .await
            .expect("ingest");
    }

    let receipt = sdk
        .orders()
        .status(status_request("order-1").with_limit(1))
        .await
        .expect("status");

    assert!(receipt.found);
    assert_eq!(receipt.status, OrderStatusKind::Requested);
    assert_eq!(receipt.event_count, 1);
    assert_eq!(receipt.limit_applied, 1);
    assert_eq!(
        receipt
            .event_ids
            .iter()
            .map(RadrootsEventId::as_str)
            .collect::<Vec<_>>(),
        vec![request_event.id.as_str()]
    );
    assert_eq!(
        receipt
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(request_event.id.as_str())
    );
    assert!(receipt.decision_event_id.is_none());
    assert_eq!(
        receipt.last_event_id.as_ref().map(RadrootsEventId::as_str),
        Some(request_event.id.as_str())
    );
    assert!(receipt.issues.is_empty());
}

#[tokio::test]
async fn order_status_reports_typed_reducer_issues() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let first_request_event = signed_order_request_event("order-1", 27);
    let second_request_event = signed_order_request_event("order-1", 28);

    for (event, observed_at_ms) in [
        (first_request_event.clone(), 2_700),
        (second_request_event.clone(), 2_800),
    ] {
        store
            .ingest_event(RadrootsEventIngest::new(event, observed_at_ms))
            .await
            .expect("ingest");
    }

    let receipt = sdk
        .orders()
        .status(status_request("order-1"))
        .await
        .expect("status");

    assert!(receipt.found);
    assert_eq!(receipt.status, OrderStatusKind::Invalid);
    assert_eq!(receipt.event_count, 2);
    assert_eq!(
        receipt
            .event_ids
            .iter()
            .map(RadrootsEventId::as_str)
            .collect::<Vec<_>>(),
        vec![
            first_request_event.id.as_str(),
            second_request_event.id.as_str()
        ]
    );
    let issue = receipt
        .issues
        .iter()
        .find(|issue| issue.kind == SdkOrderStatusIssueKind::MultipleRequests)
        .expect("multiple request issue");
    assert_eq!(
        issue
            .event_ids
            .iter()
            .map(RadrootsEventId::as_str)
            .collect::<Vec<_>>(),
        vec![
            first_request_event.id.as_str(),
            second_request_event.id.as_str()
        ]
    );
}

#[tokio::test]
async fn order_status_maps_malformed_local_data_to_sanitized_error() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-1", 30);
    let raw_event_json = serde_json::to_string(&request_event).expect("raw event json");
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 3_000))
        .await
        .expect("ingest");
    sqlx::query("UPDATE nostr_event SET tags_json = '[' WHERE event_id = ?")
        .bind(request_event.id.as_str())
        .execute(store.pool())
        .await
        .expect("corrupt tags");

    let error = sdk
        .orders()
        .status(status_request("order-1"))
        .await
        .expect_err("projection error");
    let message = error.to_string();

    assert!(matches!(error, RadrootsSdkError::Projection { .. }));
    assert!(message.contains("stored order event tags could not be decoded"));
    assert!(!message.contains(raw_event_json.as_str()));
    assert!(!message.contains(request_event.sig.as_str()));
    assert!(!message.contains("\"tags\""));
    assert!(!message.contains("\"content\""));
}
