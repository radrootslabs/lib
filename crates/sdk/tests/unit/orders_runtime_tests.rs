use super::*;
use crate::{RadrootsClient, RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider};
use radroots_authority::{RadrootsSignerError, RadrootsSignerIdentity};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
};
use radroots_event_store::RadrootsEventStoreError;
use radroots_events::{
    draft::RadrootsSignedNostrEvent,
    kinds::KIND_LISTING,
    order::{
        RadrootsOrderDecisionOutcome, RadrootsOrderEconomicItem, RadrootsOrderEconomicLine,
        RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderPricingBasis,
        RadrootsOrderRevisionOutcome,
    },
};
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, radroots_nostr_sign_frozen_draft,
};
use radroots_trade::{
    identity::RadrootsTradeLocator,
    order::{RadrootsOrderEventDecodeError, RadrootsOrderIssue},
    projection::RadrootsTradeProjectionError,
    workflow::RadrootsTradeWorkflowState,
};

#[path = "../support/serializer_failure.rs"]
mod serializer_failure;

use serializer_failure::assert_struct_serialize_error_paths;

const BUYER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const BUYER_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const SELLER_SECRET_KEY_HEX: &str =
    "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";
const SELLER_PUBLIC_KEY_HEX: &str =
    "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";
const RELAY: &str = "wss://relay.radroots.test";

fn hex_64(character: char) -> String {
    core::iter::repeat_n(character, 64).collect()
}

fn hex_128(character: char) -> String {
    core::iter::repeat_n(character, 128).collect()
}

fn event_id(character: char) -> RadrootsEventId {
    RadrootsEventId::parse(hex_64(character)).expect("event id")
}

fn pubkey(character: char) -> RadrootsPublicKey {
    RadrootsPublicKey::parse(hex_64(character)).expect("pubkey")
}

fn order_id() -> RadrootsOrderId {
    RadrootsOrderId::parse("order-test-1").expect("order id")
}

fn listing_addr(seller_pubkey: &RadrootsPublicKey) -> RadrootsListingAddress {
    RadrootsListingAddress::parse(format!(
        "30402:{}:AAAAAAAAAAAAAAAAAAAAAg",
        seller_pubkey.as_str()
    ))
    .expect("listing address")
}

fn projection(
    order_id: &RadrootsOrderId,
    listing_addr: &RadrootsListingAddress,
    buyer_pubkey: &RadrootsPublicKey,
    seller_pubkey: &RadrootsPublicKey,
    root_event_id: &RadrootsEventId,
    previous_event_id: &RadrootsEventId,
) -> RadrootsOrderProjection {
    RadrootsOrderProjection {
        order_id: order_id.clone(),
        status: RadrootsTradeWorkflowState::Requested,
        request_event_id: Some(root_event_id.clone()),
        decision_event_id: None,
        cancellation_event_id: None,
        validation_receipt_event_id: None,
        lifecycle_terminal: false,
        economics: None,
        agreement_event_id: None,
        pending_revision_event_id: None,
        pending_inventory_reservations: Vec::new(),
        committed_inventory_reservations: Vec::new(),
        listing_addr: Some(listing_addr.clone()),
        buyer_pubkey: Some(buyer_pubkey.clone()),
        seller_pubkey: Some(seller_pubkey.clone()),
        last_event_id: Some(previous_event_id.clone()),
        issues: Vec::new(),
    }
}

fn refs<'a>(
    order_id: &'a RadrootsOrderId,
    listing_addr: &'a RadrootsListingAddress,
    buyer_pubkey: &'a RadrootsPublicKey,
    seller_pubkey: &'a RadrootsPublicKey,
    root_event_id: &'a RadrootsEventId,
    previous_event_id: &'a RadrootsEventId,
) -> OrderLifecycleReferences<'a> {
    OrderLifecycleReferences {
        operation: "order test",
        order_id,
        listing_addr,
        buyer_pubkey,
        seller_pubkey,
        root_event_id,
        previous_event_id,
    }
}

fn ptr(id: String) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr { id, relays: None }
}

fn nostr_event(id: String, kind: u32) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id,
        author: hex_64('c'),
        created_at: 1_700_000_000,
        kind,
        tags: Vec::new(),
        content: "{}".to_owned(),
        sig: hex_128('f'),
    }
}

fn actor(pubkey: &RadrootsPublicKey, role: RadrootsActorRole) -> RadrootsActorContext {
    RadrootsActorContext::test(pubkey.as_str(), [role]).expect("actor")
}

fn buyer_actor() -> RadrootsActorContext {
    actor(&pubkey('c'), RadrootsActorRole::Buyer)
}

fn seller_actor() -> RadrootsActorContext {
    actor(&pubkey('d'), RadrootsActorRole::Seller)
}

fn decimal(value: &str) -> RadrootsCoreDecimal {
    value.parse().expect("decimal")
}

fn usd(value: &str) -> RadrootsCoreMoney {
    RadrootsCoreMoney::new(decimal(value), RadrootsCoreCurrency::USD)
}

fn economics(bin_count: u32, total: &str) -> RadrootsOrderEconomics {
    RadrootsOrderEconomics {
        quote_id: "quote-1".parse().expect("quote id"),
        quote_version: 1,
        pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
        currency: RadrootsCoreCurrency::USD,
        items: vec![RadrootsOrderEconomicItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count,
            quantity_amount: decimal("1"),
            quantity_unit: RadrootsCoreUnit::Each,
            unit_price_amount: decimal("5"),
            unit_price_currency: RadrootsCoreCurrency::USD,
            line_subtotal: usd(total),
        }],
        discounts: Vec::<RadrootsOrderEconomicLine>::new(),
        adjustments: Vec::<RadrootsOrderEconomicLine>::new(),
        subtotal: usd(total),
        discount_total: usd("0"),
        adjustment_total: usd("0"),
        total: usd(total),
    }
}

fn order_request_payload() -> RadrootsOrderRequest {
    let buyer_pubkey = pubkey('c');
    let seller_pubkey = pubkey('d');
    RadrootsOrderRequest {
        order_id: order_id(),
        listing_addr: listing_addr(&seller_pubkey),
        buyer_pubkey,
        seller_pubkey,
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 2,
        }],
        economics: economics(2, "10"),
    }
}

fn order_decision_payload() -> RadrootsOrderDecision {
    let buyer_pubkey = pubkey('c');
    let seller_pubkey = pubkey('d');
    RadrootsOrderDecision {
        order_id: order_id(),
        listing_addr: listing_addr(&seller_pubkey),
        buyer_pubkey,
        seller_pubkey,
        decision: RadrootsOrderDecisionOutcome::Accepted {
            inventory_commitments: vec![RadrootsOrderInventoryCommitment {
                bin_id: "bin-1".parse().expect("bin id"),
                bin_count: 2,
            }],
        },
    }
}

fn revision_proposal_payload(
    root_event_id: &RadrootsEventId,
    previous_event_id: &RadrootsEventId,
) -> RadrootsOrderRevisionProposal {
    let buyer_pubkey = pubkey('c');
    let seller_pubkey = pubkey('d');
    RadrootsOrderRevisionProposal {
        revision_id: "revision-order-test-1".parse().expect("revision id"),
        order_id: order_id(),
        listing_addr: listing_addr(&seller_pubkey),
        buyer_pubkey,
        seller_pubkey,
        root_event_id: root_event_id.clone(),
        prev_event_id: previous_event_id.clone(),
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 3,
        }],
        economics: economics(3, "15"),
        reason: "increase quantity".to_owned(),
    }
}

fn revision_decision_payload(
    proposal: &RadrootsOrderRevisionProposal,
    previous_event_id: &RadrootsEventId,
    decision: RadrootsOrderRevisionOutcome,
) -> RadrootsOrderRevisionDecision {
    RadrootsOrderRevisionDecision {
        revision_id: proposal.revision_id.clone(),
        order_id: proposal.order_id.clone(),
        listing_addr: proposal.listing_addr.clone(),
        buyer_pubkey: proposal.buyer_pubkey.clone(),
        seller_pubkey: proposal.seller_pubkey.clone(),
        root_event_id: proposal.root_event_id.clone(),
        prev_event_id: previous_event_id.clone(),
        decision,
    }
}

fn cancellation_payload() -> RadrootsOrderCancellation {
    let buyer_pubkey = pubkey('c');
    let seller_pubkey = pubkey('d');
    RadrootsOrderCancellation {
        order_id: order_id(),
        listing_addr: listing_addr(&seller_pubkey),
        buyer_pubkey,
        seller_pubkey,
        reason: "buyer changed pickup plan".to_owned(),
    }
}

#[derive(Clone)]
struct OrderFixtureSigner {
    identity: RadrootsSignerIdentity,
    keys: RadrootsNostrKeys,
}

impl OrderFixtureSigner {
    fn new(secret_key_hex: &str) -> Self {
        let keys = keys_from_secret(secret_key_hex);
        let pubkey = keys.public_key().to_hex();
        Self {
            identity: RadrootsSignerIdentity::new(pubkey).expect("identity"),
            keys,
        }
    }
}

fn keys_from_secret(secret_key_hex: &str) -> RadrootsNostrKeys {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    RadrootsNostrKeys::new(secret_key)
}

impl RadrootsEventSigner for OrderFixtureSigner {
    fn pubkey(&self) -> &RadrootsPublicKey {
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

fn fixture_buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(BUYER_PUBLIC_KEY_HEX, [RadrootsActorRole::Buyer]).expect("actor")
}

fn fixture_seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER_PUBLIC_KEY_HEX, [RadrootsActorRole::Seller]).expect("actor")
}

fn fixture_listing_addr() -> RadrootsListingAddress {
    RadrootsListingAddress::parse(format!(
        "{KIND_LISTING}:{SELLER_PUBLIC_KEY_HEX}:AAAAAAAAAAAAAAAAAAAAAg"
    ))
    .expect("listing address")
}

fn fixture_event_ptr(character: char) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: hex_64(character),
        relays: Some(RELAY.to_owned()),
    }
}

fn fixture_order_event_ptr(event_id: &RadrootsEventId) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: event_id.as_str().to_owned(),
        relays: Some(RELAY.to_owned()),
    }
}

fn fixture_order_id(raw: &str) -> RadrootsOrderId {
    RadrootsOrderId::parse(raw).expect("order id")
}

fn fixture_order_request(raw_order_id: &str) -> RadrootsOrderRequest {
    RadrootsOrderRequest {
        order_id: fixture_order_id(raw_order_id),
        listing_addr: fixture_listing_addr(),
        buyer_pubkey: BUYER_PUBLIC_KEY_HEX.parse().expect("buyer pubkey"),
        seller_pubkey: SELLER_PUBLIC_KEY_HEX.parse().expect("seller pubkey"),
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 2,
        }],
        economics: economics(2, "10"),
    }
}

fn fixture_order_decision(raw_order_id: &str) -> RadrootsOrderDecision {
    RadrootsOrderDecision {
        order_id: fixture_order_id(raw_order_id),
        listing_addr: fixture_listing_addr(),
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

fn fixture_revision_proposal(
    raw_order_id: &str,
    root_event_id: &RadrootsEventId,
    previous_event_id: &RadrootsEventId,
) -> RadrootsOrderRevisionProposal {
    RadrootsOrderRevisionProposal {
        revision_id: format!("revision-{raw_order_id}")
            .parse()
            .expect("revision id"),
        order_id: fixture_order_id(raw_order_id),
        listing_addr: fixture_listing_addr(),
        buyer_pubkey: BUYER_PUBLIC_KEY_HEX.parse().expect("buyer pubkey"),
        seller_pubkey: SELLER_PUBLIC_KEY_HEX.parse().expect("seller pubkey"),
        root_event_id: root_event_id.clone(),
        prev_event_id: previous_event_id.clone(),
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 3,
        }],
        economics: economics(3, "15"),
        reason: "increase quantity".to_owned(),
    }
}

fn fixture_revision_decision(
    proposal: &RadrootsOrderRevisionProposal,
    previous_event_id: &RadrootsEventId,
) -> RadrootsOrderRevisionDecision {
    RadrootsOrderRevisionDecision {
        revision_id: proposal.revision_id.clone(),
        order_id: proposal.order_id.clone(),
        listing_addr: proposal.listing_addr.clone(),
        buyer_pubkey: proposal.buyer_pubkey.clone(),
        seller_pubkey: proposal.seller_pubkey.clone(),
        root_event_id: proposal.root_event_id.clone(),
        prev_event_id: previous_event_id.clone(),
        decision: RadrootsOrderRevisionOutcome::Accepted,
    }
}

fn fixture_cancellation(raw_order_id: &str) -> RadrootsOrderCancellation {
    RadrootsOrderCancellation {
        order_id: fixture_order_id(raw_order_id),
        listing_addr: fixture_listing_addr(),
        buyer_pubkey: BUYER_PUBLIC_KEY_HEX.parse().expect("buyer pubkey"),
        seller_pubkey: SELLER_PUBLIC_KEY_HEX.parse().expect("seller pubkey"),
        reason: "buyer changed pickup plan".to_owned(),
    }
}

fn fixture_target_relays() -> TargetPolicy {
    TargetPolicy::try_nostr_relays([RELAY], NostrRelayUrlPolicy::Public).expect("target relays")
}

async fn prepared_order_sdk() -> RadrootsClient {
    RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .build()
        .await
        .expect("sdk")
}

async fn configured_order_sdk(secret_key_hex: &str) -> RadrootsClient {
    RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::new(keys_from_secret(secret_key_hex)).expect("signer"),
        ))
        .build()
        .await
        .expect("sdk")
}

#[tokio::test]
async fn order_configured_local_signer_enqueues_submit_without_explicit_signer() {
    let sdk = configured_order_sdk(BUYER_SECRET_KEY_HEX).await;

    let receipt = sdk
        .trades()
        .enqueue_submit(TradeSubmitEnqueueRequest::new(
            fixture_buyer_actor(),
            fixture_event_ptr('a'),
            fixture_order_request("order-configured-local-1"),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        ))
        .await
        .expect("enqueue submit");

    assert_eq!(receipt.signed_event_id, receipt.expected_event_id);
    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
}

#[tokio::test]
async fn order_configured_local_signer_enqueues_lifecycle_wrappers_without_explicit_signers() {
    let seller_sdk = configured_order_sdk(SELLER_SECRET_KEY_HEX).await;
    let decision_submit = enqueue_fixture_submit(&seller_sdk, "order-configured-decision").await;
    let decision = seller_sdk
        .trades()
        .enqueue_decision(TradeDecisionEnqueueRequest::new(
            fixture_seller_actor(),
            fixture_order_event_ptr(&decision_submit.signed_event_id),
            fixture_order_decision("order-configured-decision"),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        ))
        .await
        .expect("configured decision");
    assert_eq!(decision.request_event_id, decision_submit.signed_event_id);
    assert_eq!(decision.state, SdkMutationState::StoredAndQueued);

    let proposal_submit = enqueue_fixture_submit(&seller_sdk, "order-configured-proposal").await;
    let proposal_payload = fixture_revision_proposal(
        "order-configured-proposal",
        &proposal_submit.signed_event_id,
        &proposal_submit.signed_event_id,
    );
    let proposal = seller_sdk
        .trades()
        .enqueue_revision_proposal(TradeRevisionProposalEnqueueRequest::new(
            fixture_seller_actor(),
            fixture_order_event_ptr(&proposal_submit.signed_event_id),
            fixture_order_event_ptr(&proposal_submit.signed_event_id),
            proposal_payload,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        ))
        .await
        .expect("configured revision proposal");
    assert_eq!(proposal.root_event_id, proposal_submit.signed_event_id);
    assert_eq!(proposal.state, SdkMutationState::StoredAndQueued);

    let buyer_sdk = configured_order_sdk(BUYER_SECRET_KEY_HEX).await;
    let revision_submit =
        enqueue_fixture_submit(&buyer_sdk, "order-configured-revision-decision").await;
    let revision_proposal_payload = fixture_revision_proposal(
        "order-configured-revision-decision",
        &revision_submit.signed_event_id,
        &revision_submit.signed_event_id,
    );
    let revision_proposal = buyer_sdk
        .trades()
        .enqueue_revision_proposal_with_explicit_signer(
            TradeRevisionProposalEnqueueRequest::new(
                fixture_seller_actor(),
                fixture_order_event_ptr(&revision_submit.signed_event_id),
                fixture_order_event_ptr(&revision_submit.signed_event_id),
                revision_proposal_payload.clone(),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ),
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("explicit revision proposal");
    let revision_decision_payload = fixture_revision_decision(
        &revision_proposal_payload,
        &revision_proposal.signed_event_id,
    );
    let revision_decision = buyer_sdk
        .trades()
        .enqueue_revision_decision(TradeRevisionDecisionEnqueueRequest::new(
            fixture_buyer_actor(),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            fixture_order_event_ptr(&revision_proposal.signed_event_id),
            revision_decision_payload,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        ))
        .await
        .expect("configured revision decision");
    assert_eq!(
        revision_decision.root_event_id,
        revision_submit.signed_event_id
    );
    assert_eq!(
        revision_decision.previous_event_id,
        revision_proposal.signed_event_id
    );
    assert_eq!(revision_decision.state, SdkMutationState::StoredAndQueued);

    let cancel_submit = enqueue_fixture_submit(&buyer_sdk, "order-configured-cancel").await;
    let cancellation = buyer_sdk
        .trades()
        .enqueue_cancellation(TradeCancellationEnqueueRequest::new(
            fixture_buyer_actor(),
            fixture_order_event_ptr(&cancel_submit.signed_event_id),
            fixture_order_event_ptr(&cancel_submit.signed_event_id),
            fixture_cancellation("order-configured-cancel"),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        ))
        .await
        .expect("configured cancellation");
    assert_eq!(cancellation.root_event_id, cancel_submit.signed_event_id);
    assert_eq!(
        cancellation.previous_event_id,
        cancel_submit.signed_event_id
    );
    assert_eq!(cancellation.state, SdkMutationState::StoredAndQueued);
}

async fn enqueue_fixture_submit(sdk: &RadrootsClient, raw_order_id: &str) -> TradeSubmitReceipt {
    let buyer = fixture_buyer_actor();
    let plan = sdk
        .trades()
        .prepare_submit(TradeSubmitPrepareRequest::new(
            buyer.clone(),
            fixture_event_ptr('a'),
            fixture_order_request(raw_order_id),
        ))
        .expect("submit plan");
    sdk.trades()
        .enqueue_prepared_submit_with_explicit_signer(
            &buyer,
            plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue submit")
}

fn event_from_parts(
    parts: WireEventParts,
    contract_id: &str,
    expected_pubkey: &RadrootsPublicKey,
) -> RadrootsNostrEvent {
    let frozen = to_frozen_draft(parts, contract_id, expected_pubkey.as_str(), 1_700_000_000)
        .expect("frozen draft");
    RadrootsNostrEvent {
        id: frozen.expected_event_id,
        author: expected_pubkey.as_str().to_owned(),
        created_at: frozen.created_at,
        kind: frozen.kind,
        tags: frozen.tags,
        content: frozen.content,
        sig: hex_128('f'),
    }
}

fn request_event() -> RadrootsNostrEvent {
    let listing_event = ptr(event_id('a').as_str().to_owned());
    let request = order_request_payload();
    event_from_parts(
        order::build_order_request_draft(&listing_event, &request)
            .expect("request draft")
            .into_wire_parts(),
        TRADE_SUBMIT_CONTRACT_ID,
        &request.buyer_pubkey,
    )
}

fn order_request_evidence_error(
    result: Result<TradeRequestEvidence, RadrootsSdkError>,
) -> RadrootsSdkError {
    result.err().expect("expected order request evidence error")
}

fn invalid_request_message(error: RadrootsSdkError) -> String {
    match error {
        RadrootsSdkError::InvalidRequest { message } => message,
        other => panic!("expected invalid request error, got {other:?}"),
    }
}

fn parsed_order_evidence_error(
    result: Result<ParsedOrderEvidence, RadrootsSdkError>,
) -> RadrootsSdkError {
    result.err().expect("expected parse error")
}

fn projection_message(error: RadrootsSdkError) -> String {
    match error {
        RadrootsSdkError::Projection { message } => message,
        other => panic!("expected projection error, got {other:?}"),
    }
}

fn assert_error_display<T: core::fmt::Debug>(result: Result<T, RadrootsSdkError>, expected: &str) {
    assert!(result.unwrap_err().to_string().contains(expected));
}

fn assert_outbox_preflight_error(error: RadrootsSdkError) {
    assert!(matches!(error, RadrootsSdkError::Outbox { .. }));
}

async fn local_event_count(sdk: &RadrootsClient) -> i64 {
    sdk._event_store
        .status_summary()
        .await
        .expect("event store summary")
        .total_events
}

#[test]
fn workflow_plan_builders_cover_success_and_actor_mismatch_paths() {
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000);
    let buyer_actor = buyer_actor();
    let seller_actor = seller_actor();
    let listing_event_id = event_id('a');
    let submit_plan = order_submit_plan(
        &buyer_actor,
        ptr(listing_event_id.as_str().to_owned()),
        order_request_payload(),
        created_at,
    )
    .expect("submit plan");
    assert_eq!(submit_plan.workflow.kind, TradeWorkflowKind::Submit);
    assert_eq!(submit_plan.listing_event_id, listing_event_id);

    let request_event = ptr(submit_plan.expected_event_id.as_str().to_owned());
    let decision_plan = order_decision_plan(
        &seller_actor,
        request_event.clone(),
        order_decision_payload(),
        created_at,
    )
    .expect("decision plan");
    assert_eq!(decision_plan.workflow.kind, TradeWorkflowKind::Decision);

    let proposal = revision_proposal_payload(
        &submit_plan.expected_event_id,
        &decision_plan.expected_event_id,
    );
    let proposal_plan = order_revision_proposal_plan(
        &seller_actor,
        request_event.clone(),
        ptr(decision_plan.expected_event_id.as_str().to_owned()),
        proposal.clone(),
        created_at,
    )
    .expect("revision proposal plan");
    assert_eq!(
        proposal_plan.workflow.kind,
        TradeWorkflowKind::RevisionProposal
    );

    let revision_decision = revision_decision_payload(
        &proposal,
        &proposal_plan.expected_event_id,
        RadrootsOrderRevisionOutcome::Accepted,
    );
    let revision_decision_plan = order_revision_decision_plan(
        &buyer_actor,
        request_event.clone(),
        ptr(proposal_plan.expected_event_id.as_str().to_owned()),
        revision_decision,
        created_at,
    )
    .expect("revision decision plan");
    assert_eq!(
        revision_decision_plan.workflow.kind,
        TradeWorkflowKind::RevisionDecision
    );

    let cancellation_plan = order_cancellation_plan(
        &buyer_actor,
        request_event,
        ptr(decision_plan.expected_event_id.as_str().to_owned()),
        cancellation_payload(),
        created_at,
    )
    .expect("cancellation plan");
    assert_eq!(
        cancellation_plan.workflow.kind,
        TradeWorkflowKind::Cancellation
    );

    let mut wrong_submit = order_request_payload();
    wrong_submit.buyer_pubkey = pubkey('e');
    assert!(matches!(
        order_submit_plan(
            &buyer_actor,
            ptr(listing_event_id.as_str().to_owned()),
            wrong_submit,
            created_at,
        ),
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));

    let mut wrong_proposal = proposal;
    wrong_proposal.seller_pubkey = pubkey('e');
    assert!(matches!(
        order_revision_proposal_plan(
            &seller_actor,
            ptr(submit_plan.expected_event_id.as_str().to_owned()),
            ptr(decision_plan.expected_event_id.as_str().to_owned()),
            wrong_proposal,
            created_at,
        ),
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));

    let mut wrong_proposal_refs = revision_proposal_payload(
        &submit_plan.expected_event_id,
        &decision_plan.expected_event_id,
    );
    wrong_proposal_refs.prev_event_id = event_id('f');
    assert!(
        invalid_request_message(
            order_revision_proposal_plan(
                &seller_actor,
                ptr(submit_plan.expected_event_id.as_str().to_owned()),
                ptr(decision_plan.expected_event_id.as_str().to_owned()),
                wrong_proposal_refs,
                created_at,
            )
            .unwrap_err()
        )
        .contains("prev_event_id")
    );

    let mut wrong_revision_decision = revision_decision_payload(
        &revision_proposal_payload(
            &submit_plan.expected_event_id,
            &decision_plan.expected_event_id,
        ),
        &proposal_plan.expected_event_id,
        RadrootsOrderRevisionOutcome::Declined {
            reason: "not workable".to_owned(),
        },
    );
    wrong_revision_decision.buyer_pubkey = pubkey('e');
    assert!(matches!(
        order_revision_decision_plan(
            &buyer_actor,
            ptr(submit_plan.expected_event_id.as_str().to_owned()),
            ptr(proposal_plan.expected_event_id.as_str().to_owned()),
            wrong_revision_decision,
            created_at,
        ),
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));

    let mut wrong_revision_decision_refs = revision_decision_payload(
        &revision_proposal_payload(
            &submit_plan.expected_event_id,
            &decision_plan.expected_event_id,
        ),
        &proposal_plan.expected_event_id,
        RadrootsOrderRevisionOutcome::Accepted,
    );
    wrong_revision_decision_refs.root_event_id = event_id('f');
    assert!(
        invalid_request_message(
            order_revision_decision_plan(
                &buyer_actor,
                ptr(submit_plan.expected_event_id.as_str().to_owned()),
                ptr(proposal_plan.expected_event_id.as_str().to_owned()),
                wrong_revision_decision_refs,
                created_at,
            )
            .unwrap_err()
        )
        .contains("root_event_id")
    );
    assert!(matches!(
        order_revision_decision_plan(
            &buyer_actor,
            ptr("not-hex".to_owned()),
            ptr(proposal_plan.expected_event_id.as_str().to_owned()),
            revision_decision_payload(
                &revision_proposal_payload(
                    &submit_plan.expected_event_id,
                    &decision_plan.expected_event_id,
                ),
                &proposal_plan.expected_event_id,
                RadrootsOrderRevisionOutcome::Accepted,
            ),
            created_at,
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    assert!(matches!(
        order_revision_decision_plan(
            &buyer_actor,
            ptr(submit_plan.expected_event_id.as_str().to_owned()),
            ptr("not-hex".to_owned()),
            revision_decision_payload(
                &revision_proposal_payload(
                    &submit_plan.expected_event_id,
                    &decision_plan.expected_event_id,
                ),
                &proposal_plan.expected_event_id,
                RadrootsOrderRevisionOutcome::Accepted,
            ),
            created_at,
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let mut wrong_cancellation = cancellation_payload();
    wrong_cancellation.buyer_pubkey = pubkey('e');
    assert!(matches!(
        order_cancellation_plan(
            &buyer_actor,
            ptr(submit_plan.expected_event_id.as_str().to_owned()),
            ptr(decision_plan.expected_event_id.as_str().to_owned()),
            wrong_cancellation,
            created_at,
        ),
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
    assert!(matches!(
        order_cancellation_plan(
            &buyer_actor,
            ptr("not-hex".to_owned()),
            ptr(decision_plan.expected_event_id.as_str().to_owned()),
            cancellation_payload(),
            created_at,
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    assert!(matches!(
        order_cancellation_plan(
            &buyer_actor,
            ptr(submit_plan.expected_event_id.as_str().to_owned()),
            ptr("not-hex".to_owned()),
            cancellation_payload(),
            created_at,
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    assert!(matches!(
        sdk_timestamp_ms(RadrootsSdkTimestamp::from_unix_seconds(u64::MAX)),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        sdk_timestamp_ms(RadrootsSdkTimestamp::from_unix_seconds(
            (i64::MAX as u64 / 1_000) + 1
        )),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));

    let mut invalid_decision = order_decision_payload();
    invalid_decision.decision = RadrootsOrderDecisionOutcome::Accepted {
        inventory_commitments: Vec::new(),
    };
    assert!(
        invalid_request_message(
            validate_order_payload(&invalid_decision, "order decision").unwrap_err()
        )
        .contains("payload is invalid")
    );
    assert!(matches!(
        order_decision_plan(
            &seller_actor,
            ptr(submit_plan.expected_event_id.as_str().to_owned()),
            invalid_decision,
            created_at,
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));

    let mut invalid_proposal = revision_proposal_payload(
        &submit_plan.expected_event_id,
        &decision_plan.expected_event_id,
    );
    invalid_proposal.reason.clear();
    assert!(
        invalid_request_message(
            order_revision_proposal_plan(
                &seller_actor,
                ptr(submit_plan.expected_event_id.as_str().to_owned()),
                ptr(decision_plan.expected_event_id.as_str().to_owned()),
                invalid_proposal,
                created_at,
            )
            .unwrap_err()
        )
        .contains("payload is invalid")
    );

    let invalid_revision_decision = revision_decision_payload(
        &revision_proposal_payload(
            &submit_plan.expected_event_id,
            &decision_plan.expected_event_id,
        ),
        &proposal_plan.expected_event_id,
        RadrootsOrderRevisionOutcome::Declined {
            reason: " ".to_owned(),
        },
    );
    assert!(
        invalid_request_message(
            order_revision_decision_plan(
                &buyer_actor,
                ptr(submit_plan.expected_event_id.as_str().to_owned()),
                ptr(proposal_plan.expected_event_id.as_str().to_owned()),
                invalid_revision_decision,
                created_at,
            )
            .unwrap_err()
        )
        .contains("payload is invalid")
    );

    let mut invalid_cancellation = cancellation_payload();
    invalid_cancellation.reason = " ".to_owned();
    assert!(
        invalid_request_message(
            order_cancellation_plan(
                &buyer_actor,
                ptr(submit_plan.expected_event_id.as_str().to_owned()),
                ptr(decision_plan.expected_event_id.as_str().to_owned()),
                invalid_cancellation,
                created_at,
            )
            .unwrap_err()
        )
        .contains("payload is invalid")
    );

    let validation_error = validate_order_payload(
        &RadrootsOrderCancellation {
            reason: String::new(),
            ..cancellation_payload()
        },
        "order cancellation",
    )
    .err()
    .expect("validation error");
    assert!(invalid_request_message(validation_error).contains("payload is invalid"));

    let valid_submit_draft = order::build_order_request_draft(
        &ptr(event_id('d').as_str().to_owned()),
        &order_request_payload(),
    )
    .expect("valid submit draft");
    let (frozen_draft, expected_event_id) = freeze_order_workflow_draft(
        valid_submit_draft.into_wire_parts(),
        TRADE_SUBMIT_CONTRACT_ID,
        pubkey('c').as_str(),
        1_700_000_000,
        "order test",
    );
    assert_eq!(expected_event_id, frozen_draft.expected_event_id);
}

#[test]
fn workflow_plan_builders_cover_timestamp_reference_and_role_errors() {
    let out_of_range = RadrootsSdkTimestamp::from_unix_seconds(u64::MAX);
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000);
    let buyer_actor = buyer_actor();
    let seller_actor = seller_actor();
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');

    assert!(matches!(
        order_submit_plan(
            &buyer_actor,
            ptr(root_event_id.as_str().to_owned()),
            order_request_payload(),
            out_of_range,
        ),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        order_decision_plan(
            &seller_actor,
            ptr(root_event_id.as_str().to_owned()),
            order_decision_payload(),
            out_of_range,
        ),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        order_revision_proposal_plan(
            &seller_actor,
            ptr(root_event_id.as_str().to_owned()),
            ptr(previous_event_id.as_str().to_owned()),
            revision_proposal_payload(&root_event_id, &previous_event_id),
            out_of_range,
        ),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        order_revision_decision_plan(
            &buyer_actor,
            ptr(root_event_id.as_str().to_owned()),
            ptr(previous_event_id.as_str().to_owned()),
            revision_decision_payload(
                &revision_proposal_payload(&root_event_id, &previous_event_id),
                &previous_event_id,
                RadrootsOrderRevisionOutcome::Accepted,
            ),
            out_of_range,
        ),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        order_cancellation_plan(
            &buyer_actor,
            ptr(root_event_id.as_str().to_owned()),
            ptr(previous_event_id.as_str().to_owned()),
            cancellation_payload(),
            out_of_range,
        ),
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));

    assert!(matches!(
        order_decision_plan(
            &seller_actor,
            ptr("not-hex".to_owned()),
            order_decision_payload(),
            created_at,
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    assert!(matches!(
        order_revision_proposal_plan(
            &buyer_actor,
            ptr(root_event_id.as_str().to_owned()),
            ptr(previous_event_id.as_str().to_owned()),
            revision_proposal_payload(&root_event_id, &previous_event_id),
            created_at,
        ),
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
    assert!(matches!(
        order_revision_proposal_plan(
            &seller_actor,
            ptr("not-hex".to_owned()),
            ptr(previous_event_id.as_str().to_owned()),
            revision_proposal_payload(&root_event_id, &previous_event_id),
            created_at,
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    assert!(matches!(
        order_revision_proposal_plan(
            &seller_actor,
            ptr(root_event_id.as_str().to_owned()),
            ptr("not-hex".to_owned()),
            revision_proposal_payload(&root_event_id, &previous_event_id),
            created_at,
        ),
        Err(RadrootsSdkError::InvalidRequest { .. })
    ));
    assert!(matches!(
        order_revision_decision_plan(
            &seller_actor,
            ptr(root_event_id.as_str().to_owned()),
            ptr(previous_event_id.as_str().to_owned()),
            revision_decision_payload(
                &revision_proposal_payload(&root_event_id, &previous_event_id),
                &previous_event_id,
                RadrootsOrderRevisionOutcome::Accepted,
            ),
            created_at,
        ),
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
    assert!(matches!(
        order_cancellation_plan(
            &seller_actor,
            ptr(root_event_id.as_str().to_owned()),
            ptr(previous_event_id.as_str().to_owned()),
            cancellation_payload(),
            created_at,
        ),
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
}

#[test]
fn order_evidence_parses_all_lifecycle_event_kinds() {
    let request_event = request_event();
    let request_evidence = parse_order_evidence(&request_event).expect("request evidence");
    assert_eq!(request_evidence.event_kind, KIND_ORDER_REQUEST);

    let root_event_id = request_evidence.event_id.clone();
    let decision = order_decision_payload();
    let decision_event = event_from_parts(
        order::build_order_decision_draft(&root_event_id, &root_event_id, &decision)
            .expect("decision draft")
            .into_wire_parts(),
        TRADE_DECISION_CONTRACT_ID,
        &decision.seller_pubkey,
    );
    assert_eq!(
        parse_order_evidence(&decision_event)
            .expect("decision evidence")
            .event_kind,
        KIND_ORDER_DECISION
    );

    let decision_event_id =
        RadrootsEventId::parse(decision_event.id.as_str()).expect("decision event id");
    let proposal = revision_proposal_payload(&root_event_id, &decision_event_id);
    let proposal_event = event_from_parts(
        order::build_order_revision_proposal_draft(&root_event_id, &decision_event_id, &proposal)
            .expect("proposal draft")
            .into_wire_parts(),
        TRADE_REVISION_PROPOSAL_CONTRACT_ID,
        &proposal.seller_pubkey,
    );
    assert_eq!(
        parse_order_evidence(&proposal_event)
            .expect("proposal evidence")
            .event_kind,
        KIND_ORDER_REVISION_PROPOSAL
    );

    let proposal_event_id =
        RadrootsEventId::parse(proposal_event.id.as_str()).expect("proposal event id");
    let revision_decision = revision_decision_payload(
        &proposal,
        &proposal_event_id,
        RadrootsOrderRevisionOutcome::Accepted,
    );
    let revision_decision_event = event_from_parts(
        order::build_order_revision_decision_draft(
            &root_event_id,
            &proposal_event_id,
            &revision_decision,
        )
        .expect("revision decision draft")
        .into_wire_parts(),
        TRADE_REVISION_DECISION_CONTRACT_ID,
        &revision_decision.buyer_pubkey,
    );
    assert_eq!(
        parse_order_evidence(&revision_decision_event)
            .expect("revision decision evidence")
            .event_kind,
        KIND_ORDER_REVISION_DECISION
    );

    let cancellation = cancellation_payload();
    let cancellation_event = event_from_parts(
        order::build_order_cancellation_draft(&root_event_id, &decision_event_id, &cancellation)
            .expect("cancellation draft")
            .into_wire_parts(),
        TRADE_CANCELLATION_CONTRACT_ID,
        &cancellation.buyer_pubkey,
    );
    assert_eq!(
        parse_order_evidence(&cancellation_event)
            .expect("cancellation evidence")
            .event_kind,
        KIND_ORDER_CANCELLATION
    );
}

#[test]
fn order_request_evidence_parses_and_rejects_malformed_envelopes() {
    let event = request_event();
    let evidence = parse_order_request_evidence(&event).expect("request evidence");
    assert_eq!(evidence.order_id, order_id());
    assert_eq!(evidence.buyer_pubkey, pubkey('c'));
    assert_eq!(evidence.seller_pubkey, pubkey('d'));

    let mut invalid_id = event.clone();
    invalid_id.id = "not-hex".to_owned();
    assert!(
        invalid_request_message(order_request_evidence_error(parse_order_request_evidence(
            &invalid_id,
        )))
        .contains("event id is invalid")
    );

    let mut invalid_author = event.clone();
    invalid_author.author = "not-hex".to_owned();
    assert!(
        invalid_request_message(order_request_evidence_error(parse_order_request_evidence(
            &invalid_author,
        )))
        .contains("decode failed")
    );

    let request = order_request_payload();
    let author_mismatch = event_from_parts(
        order::build_order_request_draft(&ptr(event_id('a').as_str().to_owned()), &request)
            .expect("request draft")
            .into_wire_parts(),
        TRADE_SUBMIT_CONTRACT_ID,
        &pubkey('d'),
    );
    assert!(
        invalid_request_message(order_request_evidence_error(parse_order_request_evidence(
            &author_mismatch,
        )))
        .contains("decode failed")
    );

    let mut decode_failure = event.clone();
    decode_failure.content = "{}".to_owned();
    assert!(
        invalid_request_message(order_request_evidence_error(parse_order_request_evidence(
            &decode_failure,
        )))
        .contains("decode failed")
    );

    let mut envelope = serde_json::from_str::<serde_json::Value>(event.content.as_str())
        .expect("request envelope");
    envelope["order_id"] = serde_json::Value::String("other-order".to_owned());
    let mut order_mismatch = event.clone();
    order_mismatch.content = serde_json::to_string(&envelope).expect("mismatched envelope");
    assert!(
        invalid_request_message(order_request_evidence_error(parse_order_request_evidence(
            &order_mismatch,
        )))
        .contains("decode failed")
    );

    let mut envelope = serde_json::from_str::<serde_json::Value>(event.content.as_str())
        .expect("request envelope");
    envelope["listing_addr"] = serde_json::Value::String(format!("30402:{}:other", hex_64('d')));
    let mut listing_mismatch = event;
    listing_mismatch.content = serde_json::to_string(&envelope).expect("mismatched envelope");
    assert!(
        invalid_request_message(order_request_evidence_error(parse_order_request_evidence(
            &listing_mismatch,
        )))
        .contains("decode failed")
    );
}

#[test]
fn decision_request_evidence_preflight_covers_all_rejection_branches() {
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000);
    let request_event_id = event_id('a');
    let plan = order_decision_plan(
        &seller_actor(),
        ptr(request_event_id.as_str().to_owned()),
        order_decision_payload(),
        created_at,
    )
    .expect("decision plan");
    let mut projection = projection(
        &plan.order_id,
        &plan.listing_addr,
        &plan.buyer_pubkey,
        &plan.seller_pubkey,
        &request_event_id,
        &request_event_id,
    );
    assert!(require_decision_request_evidence(&plan, &projection).is_ok());

    projection.request_event_id = Some(event_id('b'));
    assert!(
        invalid_request_message(require_decision_request_evidence(&plan, &projection).unwrap_err())
            .contains("does not match local request")
    );

    projection.request_event_id = Some(request_event_id.clone());
    projection.status = RadrootsTradeWorkflowState::AgreedPendingRhi;
    assert!(
        invalid_request_message(require_decision_request_evidence(&plan, &projection).unwrap_err())
            .contains("requires requested local state")
    );

    projection.status = RadrootsTradeWorkflowState::Requested;
    projection.pending_revision_event_id = Some(event_id('c'));
    assert!(
        invalid_request_message(require_decision_request_evidence(&plan, &projection).unwrap_err())
            .contains("cannot follow pending revision")
    );

    projection.pending_revision_event_id = None;
    projection.issues = vec![RadrootsOrderIssue::MissingRequest];
    assert!(
        invalid_request_message(require_decision_request_evidence(&plan, &projection).unwrap_err())
            .contains("reducer issue")
    );

    projection.issues.clear();
    projection.seller_pubkey = None;
    assert!(
        invalid_request_message(require_decision_request_evidence(&plan, &projection).unwrap_err())
            .contains("missing seller_pubkey")
    );

    projection.seller_pubkey = Some(plan.seller_pubkey.clone());
    projection.listing_addr = Some(listing_addr(&pubkey('e')));
    assert!(
        invalid_request_message(require_decision_request_evidence(&plan, &projection).unwrap_err())
            .contains("listing_addr")
    );

    projection.listing_addr = Some(plan.listing_addr.clone());
    projection.buyer_pubkey = Some(pubkey('e'));
    assert!(
        invalid_request_message(require_decision_request_evidence(&plan, &projection).unwrap_err())
            .contains("buyer_pubkey")
    );
}

#[test]
fn order_status_next_action_covers_revision_and_fallback_terminal_paths() {
    let order_id = order_id();
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');
    let buyer_pubkey = pubkey('c');
    let seller_pubkey = pubkey('d');
    let listing_addr = listing_addr(&seller_pubkey);
    let mut projection = projection(
        &order_id,
        &listing_addr,
        &buyer_pubkey,
        &seller_pubkey,
        &root_event_id,
        &previous_event_id,
    );
    projection.status = RadrootsTradeWorkflowState::RevisionProposed;
    projection.pending_revision_event_id = Some(previous_event_id.clone());

    let eligibility = TradeStatusEligibility::from_projection(&projection);
    assert!(eligibility.can_decide_revision);
    assert_eq!(
        TradeStatusNextActionKind::from_projection(&projection, &eligibility),
        TradeStatusNextActionKind::DecideRevision
    );

    let receipt = TradeStatusReceipt::from_projection(
        RadrootsTradeLocator::from_order_id(order_id.clone())
            .with_root_event_id(root_event_id.clone()),
        Some(root_event_id.clone()),
        Vec::new(),
        projection.clone(),
        2,
        10,
        vec![root_event_id.clone(), previous_event_id.clone()],
    );
    assert_eq!(
        receipt.next_action,
        TradeStatusNextActionKind::DecideRevision
    );
    assert!(receipt.evidence.has_pending_revision);

    projection.pending_revision_event_id = None;
    projection.status = RadrootsTradeWorkflowState::Requested;
    let fallback = TradeStatusNextActionKind::from_projection(
        &projection,
        &TradeStatusEligibility {
            can_decide: false,
            can_propose_revision: true,
            can_decide_revision: false,
            can_cancel: true,
        },
    );
    assert_eq!(fallback, TradeStatusNextActionKind::Terminal);

    projection.lifecycle_terminal = true;
    let terminal_eligibility = TradeStatusEligibility::from_projection(&projection);
    assert_eq!(
        TradeStatusNextActionKind::from_projection(&projection, &terminal_eligibility),
        TradeStatusNextActionKind::Terminal
    );

    projection.lifecycle_terminal = false;
    projection.issues = vec![RadrootsOrderIssue::ForkedLifecycle {
        event_ids: vec![event_id('e')],
    }];
    let issue_eligibility = TradeStatusEligibility::from_projection(&projection);
    assert_eq!(
        TradeStatusNextActionKind::from_projection(&projection, &issue_eligibility),
        TradeStatusNextActionKind::InspectEvidenceIssues
    );
}

#[test]
fn order_issue_mapping_covers_every_trade_issue_variant() {
    let one = event_id('a');
    let two = event_id('b');
    let many = vec![one.clone(), two.clone()];

    macro_rules! single {
        ($issue:ident, $kind:ident) => {
            (
                RadrootsOrderIssue::$issue {
                    event_id: one.clone(),
                },
                SdkTradeStatusIssueKind::$kind,
                1,
            )
        };
    }

    let cases = vec![
        (
            RadrootsOrderIssue::MissingRequest,
            SdkTradeStatusIssueKind::MissingRequest,
            0,
        ),
        (
            RadrootsOrderIssue::MultipleRequests {
                event_ids: many.clone(),
            },
            SdkTradeStatusIssueKind::MultipleRequests,
            2,
        ),
        single!(RequestPayloadInvalid, RequestPayloadInvalid),
        single!(RequestOrderIdMismatch, RequestOrderIdMismatch),
        single!(RequestAuthorMismatch, RequestAuthorMismatch),
        single!(RequestListingAddressInvalid, RequestListingAddressInvalid),
        single!(RequestSellerListingMismatch, RequestSellerListingMismatch),
        single!(DecisionPayloadInvalid, DecisionPayloadInvalid),
        single!(DecisionOrderIdMismatch, DecisionOrderIdMismatch),
        single!(DecisionAuthorMismatch, DecisionAuthorMismatch),
        single!(DecisionCounterpartyMismatch, DecisionCounterpartyMismatch),
        single!(DecisionBuyerMismatch, DecisionBuyerMismatch),
        single!(DecisionSellerMismatch, DecisionSellerMismatch),
        single!(DecisionListingAddressInvalid, DecisionListingAddressInvalid),
        single!(DecisionListingMismatch, DecisionListingMismatch),
        single!(DecisionRootMismatch, DecisionRootMismatch),
        single!(DecisionPreviousMismatch, DecisionPreviousMismatch),
        single!(
            DecisionMissingInventoryCommitments,
            DecisionMissingInventoryCommitments
        ),
        single!(
            DecisionInventoryCommitmentMismatch,
            DecisionInventoryCommitmentMismatch
        ),
        single!(DecisionMissingReason, DecisionMissingReason),
        (
            RadrootsOrderIssue::ConflictingDecisions {
                event_ids: many.clone(),
            },
            SdkTradeStatusIssueKind::ConflictingDecisions,
            2,
        ),
        single!(
            RevisionProposalPayloadInvalid,
            RevisionProposalPayloadInvalid
        ),
        single!(
            RevisionProposalOrderIdMismatch,
            RevisionProposalOrderIdMismatch
        ),
        single!(
            RevisionProposalAuthorMismatch,
            RevisionProposalAuthorMismatch
        ),
        single!(
            RevisionProposalCounterpartyMismatch,
            RevisionProposalCounterpartyMismatch
        ),
        single!(RevisionProposalBuyerMismatch, RevisionProposalBuyerMismatch),
        single!(
            RevisionProposalSellerMismatch,
            RevisionProposalSellerMismatch
        ),
        single!(
            RevisionProposalListingAddressInvalid,
            RevisionProposalListingAddressInvalid
        ),
        single!(
            RevisionProposalListingMismatch,
            RevisionProposalListingMismatch
        ),
        single!(RevisionProposalRootMismatch, RevisionProposalRootMismatch),
        single!(
            RevisionProposalPreviousMismatch,
            RevisionProposalPreviousMismatch
        ),
        single!(
            RevisionDecisionWithoutProposal,
            RevisionDecisionWithoutProposal
        ),
        single!(
            RevisionDecisionPayloadInvalid,
            RevisionDecisionPayloadInvalid
        ),
        single!(
            RevisionDecisionOrderIdMismatch,
            RevisionDecisionOrderIdMismatch
        ),
        single!(
            RevisionDecisionAuthorMismatch,
            RevisionDecisionAuthorMismatch
        ),
        single!(
            RevisionDecisionCounterpartyMismatch,
            RevisionDecisionCounterpartyMismatch
        ),
        single!(RevisionDecisionBuyerMismatch, RevisionDecisionBuyerMismatch),
        single!(
            RevisionDecisionSellerMismatch,
            RevisionDecisionSellerMismatch
        ),
        single!(
            RevisionDecisionListingAddressInvalid,
            RevisionDecisionListingAddressInvalid
        ),
        single!(
            RevisionDecisionListingMismatch,
            RevisionDecisionListingMismatch
        ),
        single!(RevisionDecisionRootMismatch, RevisionDecisionRootMismatch),
        single!(
            RevisionDecisionPreviousMismatch,
            RevisionDecisionPreviousMismatch
        ),
        single!(
            RevisionDecisionRevisionIdMismatch,
            RevisionDecisionRevisionIdMismatch
        ),
        single!(
            CancellationWithoutCancellableOrder,
            CancellationWithoutCancellableOrder
        ),
        single!(CancellationPayloadInvalid, CancellationPayloadInvalid),
        single!(CancellationOrderIdMismatch, CancellationOrderIdMismatch),
        single!(CancellationAuthorMismatch, CancellationAuthorMismatch),
        single!(
            CancellationCounterpartyMismatch,
            CancellationCounterpartyMismatch
        ),
        single!(CancellationBuyerMismatch, CancellationBuyerMismatch),
        single!(CancellationSellerMismatch, CancellationSellerMismatch),
        single!(
            CancellationListingAddressInvalid,
            CancellationListingAddressInvalid
        ),
        single!(CancellationListingMismatch, CancellationListingMismatch),
        single!(CancellationRootMismatch, CancellationRootMismatch),
        single!(CancellationPreviousMismatch, CancellationPreviousMismatch),
        (
            RadrootsOrderIssue::ForkedLifecycle {
                event_ids: many.clone(),
            },
            SdkTradeStatusIssueKind::ForkedLifecycle,
            2,
        ),
        single!(
            ValidationReceiptWithoutPendingAgreement,
            ValidationReceiptWithoutPendingAgreement
        ),
        single!(
            ValidationReceiptOrderIdMismatch,
            ValidationReceiptOrderIdMismatch
        ),
        single!(ValidationReceiptTypeMismatch, ValidationReceiptTypeMismatch),
        single!(ValidationReceiptRootMismatch, ValidationReceiptRootMismatch),
        single!(
            ValidationReceiptTargetMismatch,
            ValidationReceiptTargetMismatch
        ),
        single!(
            ValidationReceiptListingMismatch,
            ValidationReceiptListingMismatch
        ),
        (
            RadrootsOrderIssue::ConflictingValidationReceipts {
                event_ids: many.clone(),
            },
            SdkTradeStatusIssueKind::ConflictingValidationReceipts,
            2,
        ),
        (
            RadrootsOrderIssue::DeterministicValidationFailure {
                event_id: one.clone(),
                reason: "fixture validation failed".to_owned(),
            },
            SdkTradeStatusIssueKind::DeterministicValidationFailure,
            1,
        ),
        (
            RadrootsOrderIssue::StaleListingEvent {
                expected_event_id: one,
                current_event_id: two,
            },
            SdkTradeStatusIssueKind::StaleListingEvent,
            2,
        ),
    ];

    for (issue, expected_kind, expected_event_count) in cases {
        let sdk_issue = SdkTradeStatusIssue::from(issue);
        assert_eq!(sdk_issue.kind, expected_kind);
        assert_eq!(sdk_issue.event_ids.len(), expected_event_count);
        assert_eq!(sdk_issue.code(), expected_kind.code());
    }
}

#[test]
fn lifecycle_projection_helpers_reject_unclean_mismatched_and_stale_state() {
    let order_id = order_id();
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');
    let buyer_pubkey = pubkey('c');
    let seller_pubkey = pubkey('d');
    let listing_addr = listing_addr(&seller_pubkey);
    let refs = refs(
        &order_id,
        &listing_addr,
        &buyer_pubkey,
        &seller_pubkey,
        &root_event_id,
        &previous_event_id,
    );
    let clean = projection(
        &order_id,
        &listing_addr,
        &buyer_pubkey,
        &seller_pubkey,
        &root_event_id,
        &previous_event_id,
    );
    assert!(require_clean_lifecycle_projection(refs, &clean).is_ok());
    assert!(require_lifecycle_status(&refs, &clean, RadrootsTradeWorkflowState::Requested).is_ok());
    assert!(require_no_lifecycle_terminal(&refs, &clean).is_ok());
    assert!(require_no_pending_revision(&refs, &clean).is_ok());
    assert!(require_lifecycle_previous_is_current(&refs, &clean).is_ok());

    let mut missing_request = clean.clone();
    missing_request.request_event_id = None;
    assert!(
        invalid_request_message(
            require_clean_lifecycle_projection(refs, &missing_request).unwrap_err()
        )
        .contains("requires local order request evidence")
    );

    let mut wrong_request = clean.clone();
    wrong_request.request_event_id = Some(event_id('e'));
    assert!(
        invalid_request_message(
            require_clean_lifecycle_projection(refs, &wrong_request).unwrap_err()
        )
        .contains("root event")
    );

    let mut issued = clean.clone();
    issued.issues = vec![RadrootsOrderIssue::ForkedLifecycle {
        event_ids: vec![event_id('f')],
    }];
    assert!(
        invalid_request_message(require_clean_lifecycle_projection(refs, &issued).unwrap_err())
            .contains("reducer issue")
    );

    let mut missing_listing = clean.clone();
    missing_listing.listing_addr = None;
    assert!(
        invalid_request_message(
            require_clean_lifecycle_projection(refs, &missing_listing).unwrap_err()
        )
        .contains("missing listing_addr")
    );

    let mut wrong_buyer = clean.clone();
    wrong_buyer.buyer_pubkey = Some(pubkey('e'));
    assert!(
        invalid_request_message(
            require_clean_lifecycle_projection(refs, &wrong_buyer).unwrap_err()
        )
        .contains("buyer_pubkey")
    );

    let mut accepted = clean.clone();
    accepted.status = RadrootsTradeWorkflowState::AgreedPendingRhi;
    assert!(
        invalid_request_message(
            require_lifecycle_status(&refs, &accepted, RadrootsTradeWorkflowState::Requested)
                .unwrap_err()
        )
        .contains("requires Requested")
    );

    let mut terminal = clean.clone();
    terminal.lifecycle_terminal = true;
    assert!(
        invalid_request_message(require_no_lifecycle_terminal(&refs, &terminal).unwrap_err())
            .contains("non-terminal")
    );

    assert!(
        invalid_request_message(require_pending_revision(&refs, &clean).unwrap_err())
            .contains("requires pending revision proposal")
    );

    let mut wrong_pending = clean.clone();
    wrong_pending.pending_revision_event_id = Some(event_id('e'));
    assert!(
        invalid_request_message(require_pending_revision(&refs, &wrong_pending).unwrap_err())
            .contains("does not match pending revision")
    );

    let mut pending = clean.clone();
    pending.pending_revision_event_id = Some(previous_event_id.clone());
    assert!(require_pending_revision(&refs, &pending).is_ok());
    assert!(
        invalid_request_message(require_no_pending_revision(&refs, &pending).unwrap_err())
            .contains("cannot follow pending revision")
    );

    let mut wrong_previous = clean.clone();
    wrong_previous.last_event_id = Some(event_id('e'));
    assert!(
        invalid_request_message(
            require_lifecycle_previous_is_current(&refs, &wrong_previous).unwrap_err()
        )
        .contains("does not match current lifecycle event")
    );

    let mut missing_previous = clean;
    missing_previous.last_event_id = None;
    assert!(
        invalid_request_message(
            require_lifecycle_previous_is_current(&refs, &missing_previous).unwrap_err()
        )
        .contains("requires current lifecycle event evidence")
    );
}

#[test]
fn lifecycle_plan_state_wrappers_reject_invalid_status_terminal_and_pending_state() {
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000);
    let buyer_actor = buyer_actor();
    let seller_actor = seller_actor();
    let listing_event_id = event_id('a');
    let submit_plan = order_submit_plan(
        &buyer_actor,
        ptr(listing_event_id.as_str().to_owned()),
        order_request_payload(),
        created_at,
    )
    .expect("submit plan");
    let decision_plan = order_decision_plan(
        &seller_actor,
        ptr(submit_plan.expected_event_id.as_str().to_owned()),
        order_decision_payload(),
        created_at,
    )
    .expect("decision plan");
    let proposal_payload = revision_proposal_payload(
        &submit_plan.expected_event_id,
        &decision_plan.expected_event_id,
    );
    let proposal_plan = order_revision_proposal_plan(
        &seller_actor,
        ptr(submit_plan.expected_event_id.as_str().to_owned()),
        ptr(decision_plan.expected_event_id.as_str().to_owned()),
        proposal_payload.clone(),
        created_at,
    )
    .expect("proposal plan");
    let revision_plan = order_revision_decision_plan(
        &buyer_actor,
        ptr(submit_plan.expected_event_id.as_str().to_owned()),
        ptr(proposal_plan.expected_event_id.as_str().to_owned()),
        revision_decision_payload(
            &proposal_payload,
            &proposal_plan.expected_event_id,
            RadrootsOrderRevisionOutcome::Accepted,
        ),
        created_at,
    )
    .expect("revision decision plan");
    let cancellation_plan = order_cancellation_plan(
        &buyer_actor,
        ptr(submit_plan.expected_event_id.as_str().to_owned()),
        ptr(decision_plan.expected_event_id.as_str().to_owned()),
        cancellation_payload(),
        created_at,
    )
    .expect("cancellation plan");
    let base = projection(
        &proposal_plan.order_id,
        &proposal_plan.listing_addr,
        &proposal_plan.buyer_pubkey,
        &proposal_plan.seller_pubkey,
        &submit_plan.expected_event_id,
        &decision_plan.expected_event_id,
    );

    assert!(require_revision_proposal_state(&proposal_plan, &base).is_ok());

    let mut accepted = base.clone();
    accepted.status = RadrootsTradeWorkflowState::AgreedPendingRhi;
    assert!(
        invalid_request_message(
            require_revision_proposal_state(&proposal_plan, &accepted).unwrap_err()
        )
        .contains("requires Requested")
    );

    let mut terminal = base.clone();
    terminal.lifecycle_terminal = true;
    assert!(
        invalid_request_message(
            require_revision_proposal_state(&proposal_plan, &terminal).unwrap_err()
        )
        .contains("non-terminal")
    );

    let mut proposal_pending = base.clone();
    proposal_pending.pending_revision_event_id = Some(proposal_plan.expected_event_id.clone());
    assert!(
        invalid_request_message(
            require_revision_proposal_state(&proposal_plan, &proposal_pending).unwrap_err()
        )
        .contains("cannot follow pending revision")
    );

    let mut revision_ready = base.clone();
    revision_ready.status = RadrootsTradeWorkflowState::RevisionProposed;
    revision_ready.last_event_id = Some(proposal_plan.expected_event_id.clone());
    revision_ready.pending_revision_event_id = Some(proposal_plan.expected_event_id.clone());
    assert!(require_revision_decision_state(&revision_plan, &revision_ready).is_ok());

    let mut revision_terminal = revision_ready.clone();
    revision_terminal.lifecycle_terminal = true;
    assert!(
        invalid_request_message(
            require_revision_decision_state(&revision_plan, &revision_terminal).unwrap_err()
        )
        .contains("non-terminal")
    );

    let mut revision_without_pending = revision_ready;
    revision_without_pending.pending_revision_event_id = None;
    assert!(
        invalid_request_message(
            require_revision_decision_state(&revision_plan, &revision_without_pending).unwrap_err()
        )
        .contains("requires pending revision proposal")
    );

    assert!(require_cancellation_state(&cancellation_plan, &base).is_ok());

    let mut cancellation_accepted = base.clone();
    cancellation_accepted.status = RadrootsTradeWorkflowState::AgreedPendingRhi;
    assert!(
        invalid_request_message(
            require_cancellation_state(&cancellation_plan, &cancellation_accepted).unwrap_err()
        )
        .contains("cancellation requires requested")
    );

    let mut cancellation_terminal = base.clone();
    cancellation_terminal.lifecycle_terminal = true;
    assert!(
        invalid_request_message(
            require_cancellation_state(&cancellation_plan, &cancellation_terminal).unwrap_err()
        )
        .contains("non-terminal")
    );

    let mut cancellation_pending = base;
    cancellation_pending.pending_revision_event_id = Some(proposal_plan.expected_event_id);
    assert!(
        invalid_request_message(
            require_cancellation_state(&cancellation_plan, &cancellation_pending).unwrap_err()
        )
        .contains("cannot follow pending revision")
    );
}

#[test]
fn evidence_reference_helpers_reject_invalid_ids_and_payload_mismatches() {
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');
    let alternate_event_id = event_id('c');

    assert_eq!(
        request_event_id(&ptr(root_event_id.as_str().to_owned())).expect("request event"),
        root_event_id
    );
    assert_eq!(
        order_reference_event_id(&ptr(previous_event_id.as_str().to_owned()), "decision")
            .expect("reference event"),
        previous_event_id
    );
    assert!(
        invalid_request_message(request_event_id(&ptr("not-hex".to_owned())).unwrap_err())
            .contains("order request evidence event id is invalid")
    );
    assert!(
        invalid_request_message(
            order_reference_event_id(&ptr("not-hex".to_owned()), "decision").unwrap_err()
        )
        .contains("order decision evidence event id is invalid")
    );

    assert!(
        require_payload_event_refs(
            "order decision",
            &root_event_id,
            &previous_event_id,
            &root_event_id,
            &previous_event_id,
        )
        .is_ok()
    );
    assert!(
        invalid_request_message(
            require_payload_event_refs(
                "order decision",
                &alternate_event_id,
                &previous_event_id,
                &root_event_id,
                &previous_event_id,
            )
            .unwrap_err()
        )
        .contains("root_event_id")
    );
    assert!(
        invalid_request_message(
            require_payload_event_refs(
                "order decision",
                &root_event_id,
                &alternate_event_id,
                &root_event_id,
                &previous_event_id,
            )
            .unwrap_err()
        )
        .contains("prev_event_id")
    );
}

#[test]
fn parse_order_evidence_reports_invalid_ids_unsupported_kinds_and_decode_errors() {
    assert!(
        invalid_request_message(parsed_order_evidence_error(parse_order_evidence(
            &nostr_event("not-hex".to_owned(), KIND_ORDER_REQUEST,)
        )))
        .contains("order evidence event id is invalid")
    );
    assert!(
        invalid_request_message(parsed_order_evidence_error(parse_order_evidence(
            &nostr_event(hex_64('a'), 1),
        )))
        .contains("order evidence event kind 1 is not supported")
    );
    assert!(
        invalid_request_message(parsed_order_evidence_error(parse_order_evidence(
            &nostr_event(hex_64('a'), KIND_ORDER_REQUEST),
        )))
        .contains("order evidence event is invalid")
    );
    for kind in [
        KIND_ORDER_DECISION,
        KIND_ORDER_REVISION_PROPOSAL,
        KIND_ORDER_REVISION_DECISION,
        KIND_ORDER_CANCELLATION,
    ] {
        assert!(
            invalid_request_message(parsed_order_evidence_error(parse_order_evidence(
                &nostr_event(hex_64('a'), kind),
            )))
            .contains("order evidence event is invalid")
        );
    }
}

#[test]
fn projection_error_maps_store_tag_and_decode_errors() {
    assert_eq!(
        projection_message(projection_error(RadrootsOrderStoreQueryError::Store(
            RadrootsEventStoreError::MissingEvent("event-a".to_owned())
        ))),
        "order status store query failed"
    );
    assert_eq!(
        projection_message(projection_error(
            RadrootsOrderStoreQueryError::InvalidStoredTagsJson {
                event_id: "event-a".to_owned(),
                source: serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
            }
        )),
        "stored order event tags could not be decoded"
    );
    assert_eq!(
        projection_message(projection_error(RadrootsOrderStoreQueryError::Decode {
            event_id: "event-a".to_owned(),
            source: RadrootsOrderEventDecodeError::UnsupportedKind { kind: 999 },
        })),
        "stored order event could not decode as order record"
    );
    let invalid_limit = projection_error(RadrootsOrderStoreQueryError::Projection(
        RadrootsTradeProjectionError::InvalidLimit { max: 1000 },
    ));
    assert!(matches!(
        invalid_limit,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[test]
fn order_enqueue_request_mutators_reject_invalid_relays_and_idempotency_keys() {
    let buyer = buyer_actor();
    let seller = seller_actor();
    let listing_event = ptr(event_id('a').as_str().to_owned());
    let request_event = ptr(event_id('b').as_str().to_owned());
    let previous_event = ptr(event_id('c').as_str().to_owned());
    let submit_payload = order_request_payload();
    let decision_payload = order_decision_payload();
    let proposal_payload = revision_proposal_payload(&event_id('b'), &event_id('c'));
    let revision_decision_payload = revision_decision_payload(
        &proposal_payload,
        &event_id('d'),
        RadrootsOrderRevisionOutcome::Accepted,
    );
    let cancellation_payload = cancellation_payload();
    let policy = TargetPolicy::UseConfiguredProfile;

    assert_error_display(
        TradeSubmitEnqueueRequest::new(
            buyer.clone(),
            listing_event.clone(),
            submit_payload.clone(),
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_target_relays(Vec::<String>::new(), NostrRelayUrlPolicy::Public),
        "target relays",
    );
    assert_error_display(
        TradeSubmitEnqueueRequest::new(
            buyer.clone(),
            listing_event,
            submit_payload,
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_idempotency_key(""),
        "idempotency key",
    );

    assert_error_display(
        TradeDecisionEnqueueRequest::new(
            seller.clone(),
            request_event.clone(),
            decision_payload.clone(),
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_target_relays(Vec::<String>::new(), NostrRelayUrlPolicy::Public),
        "target relays",
    );
    assert_error_display(
        TradeDecisionEnqueueRequest::new(
            seller.clone(),
            request_event.clone(),
            decision_payload,
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_idempotency_key(" leading"),
        "idempotency key",
    );

    assert_error_display(
        TradeRevisionProposalEnqueueRequest::new(
            seller.clone(),
            request_event.clone(),
            previous_event.clone(),
            proposal_payload.clone(),
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_target_relays(Vec::<String>::new(), NostrRelayUrlPolicy::Public),
        "target relays",
    );
    assert_error_display(
        TradeRevisionProposalEnqueueRequest::new(
            seller.clone(),
            request_event.clone(),
            previous_event.clone(),
            proposal_payload.clone(),
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_idempotency_key("trailing "),
        "idempotency key",
    );

    assert_error_display(
        TradeRevisionDecisionEnqueueRequest::new(
            buyer.clone(),
            request_event.clone(),
            previous_event.clone(),
            revision_decision_payload.clone(),
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_target_relays(Vec::<String>::new(), NostrRelayUrlPolicy::Public),
        "target relays",
    );
    assert_error_display(
        TradeRevisionDecisionEnqueueRequest::new(
            buyer.clone(),
            request_event.clone(),
            previous_event.clone(),
            revision_decision_payload,
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_idempotency_key("invalid\nkey"),
        "idempotency key",
    );

    assert_error_display(
        TradeCancellationEnqueueRequest::new(
            buyer.clone(),
            request_event.clone(),
            previous_event.clone(),
            cancellation_payload.clone(),
            policy.clone(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_target_relays(Vec::<String>::new(), NostrRelayUrlPolicy::Public),
        "target relays",
    );
    assert_error_display(
        TradeCancellationEnqueueRequest::new(
            buyer,
            request_event,
            previous_event,
            cancellation_payload,
            policy,
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
        )
        .try_with_idempotency_key(""),
        "idempotency key",
    );
}

#[test]
fn trade_enqueue_policy_rejects_publish_modes_without_matching_side_effects() {
    assert!(matches!(
        validate_trade_enqueue_policy(PublishMode::DryRun, SatisfactionPolicy::NoWait),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "trade dry-run publish mode must use a prepare request"
    ));
    assert!(matches!(
        validate_trade_enqueue_policy(PublishMode::EnqueueOnly, SatisfactionPolicy::AtLeastOneTarget),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "trade enqueue-only publish mode only supports no-wait acknowledgement"
    ));
    assert!(matches!(
        validate_trade_enqueue_policy(PublishMode::EnqueueAndPublish, SatisfactionPolicy::NoWait),
        Err(RadrootsSdkError::InvalidRequest { ref message })
            if message == "trade enqueue-and-publish requires a relay acknowledgement policy"
    ));
    assert!(
        validate_trade_enqueue_policy(PublishMode::EnqueueOnly, SatisfactionPolicy::NoWait).is_ok()
    );
    assert!(
        validate_trade_enqueue_policy(
            PublishMode::EnqueueAndPublish,
            SatisfactionPolicy::AtLeastOneTarget
        )
        .is_ok()
    );
}

#[tokio::test]
async fn orders_client_prepare_methods_resolve_request_created_at() {
    let sdk = crate::RadrootsClient::builder()
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_010))
        .build()
        .await
        .expect("sdk");
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');
    let root_event = ptr(root_event_id.as_str().to_owned());
    let previous_event = ptr(previous_event_id.as_str().to_owned());

    assert_eq!(
        sdk.trades()
            .prepare_submit(TradeSubmitPrepareRequest::new(
                buyer_actor(),
                root_event.clone(),
                order_request_payload(),
            ))
            .expect("submit plan")
            .created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_010)
    );
    assert_eq!(
        sdk.trades()
            .prepare_decision(
                TradeDecisionPrepareRequest::new(
                    seller_actor(),
                    root_event.clone(),
                    order_decision_payload(),
                )
                .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_011)),
            )
            .expect("decision plan")
            .created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_011)
    );

    let proposal = revision_proposal_payload(&root_event_id, &previous_event_id);
    assert_eq!(
        sdk.trades()
            .prepare_revision_proposal(
                TradeRevisionProposalPrepareRequest::new(
                    seller_actor(),
                    root_event.clone(),
                    previous_event.clone(),
                    proposal.clone(),
                )
                .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_012)),
            )
            .expect("proposal plan")
            .created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_012)
    );
    assert_eq!(
        sdk.trades()
            .prepare_revision_decision(
                TradeRevisionDecisionPrepareRequest::new(
                    buyer_actor(),
                    root_event.clone(),
                    ptr(event_id('c').as_str().to_owned()),
                    revision_decision_payload(
                        &proposal,
                        &event_id('c'),
                        RadrootsOrderRevisionOutcome::Accepted,
                    ),
                )
                .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_013)),
            )
            .expect("revision decision plan")
            .created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_013)
    );
    assert_eq!(
        sdk.trades()
            .prepare_cancellation(
                TradeCancellationPrepareRequest::new(
                    buyer_actor(),
                    root_event,
                    previous_event,
                    cancellation_payload(),
                )
                .with_created_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_014)),
            )
            .expect("cancellation plan")
            .created_at,
        RadrootsSdkTimestamp::from_unix_seconds(1_700_000_014)
    );
}

#[tokio::test]
async fn prepared_submit_and_decision_enqueue_cover_source_attached_success_path() {
    let sdk = prepared_order_sdk().await;
    let submit = enqueue_fixture_submit(&sdk, "order-prepared-decision");
    let submit = submit.await;
    assert_eq!(submit.signed_event_id, submit.expected_event_id);
    assert_eq!(submit.workflow.kind, TradeWorkflowKind::Submit);

    let seller = fixture_seller_actor();
    let decision_plan = sdk
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&submit.signed_event_id),
            fixture_order_decision("order-prepared-decision"),
        ))
        .expect("decision plan");
    let decision = sdk
        .trades()
        .enqueue_prepared_decision_with_explicit_signer(
            &seller,
            decision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            Some(SdkIdempotencyKey::new("prepared-decision").expect("idempotency")),
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue decision");

    assert_eq!(decision.signed_event_id, decision.expected_event_id);
    assert_eq!(decision.workflow.kind, TradeWorkflowKind::Decision);
    assert_eq!(decision.request_event_id, submit.signed_event_id);
}

#[tokio::test]
async fn prepared_revision_lifecycle_enqueue_cover_source_attached_success_paths() {
    let sdk = prepared_order_sdk().await;
    let submit = enqueue_fixture_submit(&sdk, "order-prepared-revision").await;
    let seller = fixture_seller_actor();
    let proposal_payload = fixture_revision_proposal(
        "order-prepared-revision",
        &submit.signed_event_id,
        &submit.signed_event_id,
    );
    let proposal_plan = sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&submit.signed_event_id),
            fixture_order_event_ptr(&submit.signed_event_id),
            proposal_payload.clone(),
        ))
        .expect("proposal plan");
    let proposal = sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            proposal_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            Some(SdkIdempotencyKey::new("prepared-proposal").expect("idempotency")),
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue proposal");

    assert_eq!(proposal.signed_event_id, proposal.expected_event_id);
    assert_eq!(proposal.workflow.kind, TradeWorkflowKind::RevisionProposal);
    assert_eq!(proposal.root_event_id, submit.signed_event_id);
    assert_eq!(proposal.previous_event_id, submit.signed_event_id);

    let buyer = fixture_buyer_actor();
    let revision_decision = fixture_revision_decision(&proposal_payload, &proposal.signed_event_id);
    let revision_decision_plan = sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            fixture_order_event_ptr(&submit.signed_event_id),
            fixture_order_event_ptr(&proposal.signed_event_id),
            revision_decision,
        ))
        .expect("revision decision plan");
    let revision = sdk
        .trades()
        .enqueue_prepared_revision_decision_with_explicit_signer(
            &buyer,
            revision_decision_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue revision decision");

    assert_eq!(revision.signed_event_id, revision.expected_event_id);
    assert_eq!(revision.workflow.kind, TradeWorkflowKind::RevisionDecision);
    assert_eq!(revision.root_event_id, submit.signed_event_id);
    assert_eq!(revision.previous_event_id, proposal.signed_event_id);
}

#[tokio::test]
async fn prepared_cancellation_enqueue_covers_source_attached_success_path() {
    let sdk = prepared_order_sdk().await;
    let submit = enqueue_fixture_submit(&sdk, "order-prepared-cancellation").await;
    let buyer = fixture_buyer_actor();
    let cancellation_plan = sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer.clone(),
            fixture_order_event_ptr(&submit.signed_event_id),
            fixture_order_event_ptr(&submit.signed_event_id),
            fixture_cancellation("order-prepared-cancellation"),
        ))
        .expect("cancellation plan");
    let cancellation = sdk
        .trades()
        .enqueue_prepared_cancellation_with_explicit_signer(
            &buyer,
            cancellation_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            Some(SdkIdempotencyKey::new("prepared-cancellation").expect("idempotency")),
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue cancellation");

    assert_eq!(cancellation.signed_event_id, cancellation.expected_event_id);
    assert_eq!(cancellation.workflow.kind, TradeWorkflowKind::Cancellation);
    assert_eq!(cancellation.root_event_id, submit.signed_event_id);
    assert_eq!(cancellation.previous_event_id, submit.signed_event_id);
}

#[tokio::test]
async fn convenience_order_enqueue_methods_cover_source_attached_wrappers() {
    let sdk = prepared_order_sdk().await;
    let decision_submit = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(
            TradeSubmitEnqueueRequest::new(
                fixture_buyer_actor(),
                fixture_event_ptr('b'),
                fixture_order_request("order-wrapper-decision"),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ),
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue submit");
    let decision = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(
            TradeDecisionEnqueueRequest::new(
                fixture_seller_actor(),
                fixture_order_event_ptr(&decision_submit.signed_event_id),
                fixture_order_decision("order-wrapper-decision"),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ),
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue decision");
    assert_eq!(decision.request_event_id, decision_submit.signed_event_id);

    let revision_submit = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(
            TradeSubmitEnqueueRequest::new(
                fixture_buyer_actor(),
                fixture_event_ptr('c'),
                fixture_order_request("order-wrapper-revision"),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ),
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue revision submit");
    let proposal_payload = fixture_revision_proposal(
        "order-wrapper-revision",
        &revision_submit.signed_event_id,
        &revision_submit.signed_event_id,
    );
    let proposal = sdk
        .trades()
        .enqueue_revision_proposal_with_explicit_signer(
            TradeRevisionProposalEnqueueRequest::new(
                fixture_seller_actor(),
                fixture_order_event_ptr(&revision_submit.signed_event_id),
                fixture_order_event_ptr(&revision_submit.signed_event_id),
                proposal_payload.clone(),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ),
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue proposal");
    let revision = sdk
        .trades()
        .enqueue_revision_decision_with_explicit_signer(
            TradeRevisionDecisionEnqueueRequest::new(
                fixture_buyer_actor(),
                fixture_order_event_ptr(&revision_submit.signed_event_id),
                fixture_order_event_ptr(&proposal.signed_event_id),
                fixture_revision_decision(&proposal_payload, &proposal.signed_event_id),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ),
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue revision decision");
    assert_eq!(revision.previous_event_id, proposal.signed_event_id);

    let cancellation_submit = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(
            TradeSubmitEnqueueRequest::new(
                fixture_buyer_actor(),
                fixture_event_ptr('d'),
                fixture_order_request("order-wrapper-cancellation"),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ),
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue cancellation submit");
    let cancellation = sdk
        .trades()
        .enqueue_cancellation_with_explicit_signer(
            TradeCancellationEnqueueRequest::new(
                fixture_buyer_actor(),
                fixture_order_event_ptr(&cancellation_submit.signed_event_id),
                fixture_order_event_ptr(&cancellation_submit.signed_event_id),
                fixture_cancellation("order-wrapper-cancellation"),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ),
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue cancellation");
    assert_eq!(
        cancellation.previous_event_id,
        cancellation_submit.signed_event_id
    );
}

#[tokio::test]
async fn prepared_lifecycle_enqueues_report_missing_and_closed_preflight_errors() {
    let sdk = prepared_order_sdk().await;
    let buyer = fixture_buyer_actor();
    let seller = fixture_seller_actor();
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');
    let root = fixture_order_event_ptr(&root_event_id);
    let previous = fixture_order_event_ptr(&previous_event_id);
    let proposal =
        fixture_revision_proposal("order-preflight-errors", &root_event_id, &previous_event_id);

    let decision_plan = sdk
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
            seller.clone(),
            root.clone(),
            fixture_order_decision("order-preflight-errors"),
        ))
        .expect("decision plan");
    let decision_missing = sdk
        .trades()
        .enqueue_prepared_decision_with_explicit_signer(
            &seller,
            decision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("missing decision evidence");
    assert!(matches!(
        decision_missing,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let proposal_plan = sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            root.clone(),
            previous.clone(),
            proposal.clone(),
        ))
        .expect("proposal plan");
    let proposal_missing = sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            proposal_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("missing proposal evidence");
    assert!(matches!(
        proposal_missing,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let revision_decision_plan = sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            root.clone(),
            previous.clone(),
            fixture_revision_decision(&proposal, &previous_event_id),
        ))
        .expect("revision decision plan");
    let revision_missing = sdk
        .trades()
        .enqueue_prepared_revision_decision_with_explicit_signer(
            &buyer,
            revision_decision_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("missing revision evidence");
    assert!(matches!(
        revision_missing,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let cancellation_plan = sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer.clone(),
            root,
            previous,
            fixture_cancellation("order-preflight-errors"),
        ))
        .expect("cancellation plan");
    let cancellation_missing = sdk
        .trades()
        .enqueue_prepared_cancellation_with_explicit_signer(
            &buyer,
            cancellation_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("missing cancellation evidence");
    assert!(matches!(
        cancellation_missing,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let closed_sdk = prepared_order_sdk().await;
    let closed_root_event_id = event_id('c');
    let closed_previous_event_id = event_id('d');
    let closed_root = fixture_order_event_ptr(&closed_root_event_id);
    let closed_previous = fixture_order_event_ptr(&closed_previous_event_id);
    let closed_proposal = fixture_revision_proposal(
        "order-closed-preflight",
        &closed_root_event_id,
        &closed_previous_event_id,
    );
    let closed_plan = closed_sdk
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
            seller.clone(),
            closed_root.clone(),
            fixture_order_decision("order-closed-preflight"),
        ))
        .expect("closed decision plan");
    let closed_proposal_plan = closed_sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            closed_root.clone(),
            closed_previous.clone(),
            closed_proposal.clone(),
        ))
        .expect("closed proposal plan");
    let closed_revision_plan = closed_sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            closed_root.clone(),
            closed_previous.clone(),
            fixture_revision_decision(&closed_proposal, &closed_previous_event_id),
        ))
        .expect("closed revision decision plan");
    let closed_cancellation_plan = closed_sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer.clone(),
            closed_root,
            closed_previous,
            fixture_cancellation("order-closed-preflight"),
        ))
        .expect("closed cancellation plan");
    closed_sdk._event_store.pool().close().await;
    let closed_error = closed_sdk
        .trades()
        .enqueue_prepared_decision_with_explicit_signer(
            &seller,
            closed_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("closed preflight");
    assert!(matches!(closed_error, RadrootsSdkError::EventStore { .. }));
    let closed_proposal_error = closed_sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            closed_proposal_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("closed proposal preflight");
    assert!(matches!(
        closed_proposal_error,
        RadrootsSdkError::EventStore { .. }
    ));
    let closed_revision_error = closed_sdk
        .trades()
        .enqueue_prepared_revision_decision_with_explicit_signer(
            &buyer,
            closed_revision_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("closed revision preflight");
    assert!(matches!(
        closed_revision_error,
        RadrootsSdkError::EventStore { .. }
    ));
    let closed_cancellation_error = closed_sdk
        .trades()
        .enqueue_prepared_cancellation_with_explicit_signer(
            &buyer,
            closed_cancellation_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("closed cancellation preflight");
    assert!(matches!(
        closed_cancellation_error,
        RadrootsSdkError::EventStore { .. }
    ));
}

#[tokio::test]
async fn configured_prepared_lifecycle_enqueues_run_preflight_guards() {
    let buyer = fixture_buyer_actor();
    let seller = fixture_seller_actor();
    let root_event_id = event_id('e');
    let previous_event_id = event_id('f');
    let root = fixture_order_event_ptr(&root_event_id);
    let previous = fixture_order_event_ptr(&previous_event_id);
    let proposal = fixture_revision_proposal(
        "order-configured-preflight",
        &root_event_id,
        &previous_event_id,
    );

    let seller_sdk = configured_order_sdk(SELLER_SECRET_KEY_HEX).await;
    let decision_plan = seller_sdk
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
            seller.clone(),
            root.clone(),
            fixture_order_decision("order-configured-preflight"),
        ))
        .expect("decision plan");
    let decision_missing = seller_sdk
        .trades()
        .enqueue_prepared_decision(
            &seller,
            decision_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
        )
        .await
        .expect_err("configured missing decision evidence");
    assert!(matches!(
        decision_missing,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let proposal_plan = seller_sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            root.clone(),
            previous.clone(),
            proposal.clone(),
        ))
        .expect("proposal plan");
    let proposal_missing = seller_sdk
        .trades()
        .enqueue_prepared_revision_proposal(
            &seller,
            proposal_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
        )
        .await
        .expect_err("configured missing proposal evidence");
    assert!(matches!(
        proposal_missing,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let buyer_sdk = configured_order_sdk(BUYER_SECRET_KEY_HEX).await;
    let revision_plan = buyer_sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            root.clone(),
            previous.clone(),
            fixture_revision_decision(&proposal, &previous_event_id),
        ))
        .expect("revision decision plan");
    let revision_missing = buyer_sdk
        .trades()
        .enqueue_prepared_revision_decision(
            &buyer,
            revision_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
        )
        .await
        .expect_err("configured missing revision decision evidence");
    assert!(matches!(
        revision_missing,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let cancellation_plan = buyer_sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer.clone(),
            root,
            previous,
            fixture_cancellation("order-configured-preflight"),
        ))
        .expect("cancellation plan");
    let cancellation_missing = buyer_sdk
        .trades()
        .enqueue_prepared_cancellation(
            &buyer,
            cancellation_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
        )
        .await
        .expect_err("configured missing cancellation evidence");
    assert!(matches!(
        cancellation_missing,
        RadrootsSdkError::InvalidRequest { .. }
    ));
}

#[tokio::test]
async fn configured_prepared_lifecycle_enqueues_report_existing_event_lookup_errors() {
    let buyer = fixture_buyer_actor();
    let seller = fixture_seller_actor();
    let root_event_id = event_id('e');
    let previous_event_id = event_id('f');
    let root = fixture_order_event_ptr(&root_event_id);
    let previous = fixture_order_event_ptr(&previous_event_id);
    let proposal = fixture_revision_proposal(
        "order-configured-existing-lookup",
        &root_event_id,
        &previous_event_id,
    );

    let seller_sdk = configured_order_sdk(SELLER_SECRET_KEY_HEX).await;
    let decision_plan = seller_sdk
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
            seller.clone(),
            root.clone(),
            fixture_order_decision("order-configured-existing-lookup"),
        ))
        .expect("decision plan");
    let proposal_plan = seller_sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            root.clone(),
            previous.clone(),
            proposal.clone(),
        ))
        .expect("proposal plan");
    seller_sdk._event_store.pool().close().await;
    assert!(matches!(
        seller_sdk
            .trades()
            .enqueue_prepared_decision(
                &seller,
                decision_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None
            )
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));
    assert!(matches!(
        seller_sdk
            .trades()
            .enqueue_prepared_revision_proposal(
                &seller,
                proposal_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None,
            )
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));

    let buyer_sdk = configured_order_sdk(BUYER_SECRET_KEY_HEX).await;
    let revision_plan = buyer_sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            root.clone(),
            previous.clone(),
            fixture_revision_decision(&proposal, &previous_event_id),
        ))
        .expect("revision decision plan");
    let cancellation_plan = buyer_sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer.clone(),
            root,
            previous,
            fixture_cancellation("order-configured-existing-lookup"),
        ))
        .expect("cancellation plan");
    buyer_sdk._event_store.pool().close().await;
    assert!(matches!(
        buyer_sdk
            .trades()
            .enqueue_prepared_revision_decision(
                &buyer,
                revision_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None
            )
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));
    assert!(matches!(
        buyer_sdk
            .trades()
            .enqueue_prepared_cancellation(
                &buyer,
                cancellation_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None
            )
            .await,
        Err(RadrootsSdkError::EventStore { .. })
    ));
}

#[tokio::test]
async fn configured_enqueue_wrappers_report_prepare_errors_before_signing() {
    let seller_sdk = configured_order_sdk(SELLER_SECRET_KEY_HEX).await;
    let buyer_sdk = configured_order_sdk(BUYER_SECRET_KEY_HEX).await;
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');
    let proposal = fixture_revision_proposal(
        "order-configured-prepare-errors",
        &root_event_id,
        &previous_event_id,
    );

    assert!(matches!(
        buyer_sdk
            .trades()
            .enqueue_submit(TradeSubmitEnqueueRequest::new(
                fixture_seller_actor(),
                fixture_event_ptr('a'),
                fixture_order_request("order-configured-prepare-submit"),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ))
            .await,
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
    assert!(matches!(
        seller_sdk
            .trades()
            .enqueue_decision(TradeDecisionEnqueueRequest::new(
                fixture_buyer_actor(),
                fixture_event_ptr('a'),
                fixture_order_decision("order-configured-prepare-decision"),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ))
            .await,
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
    assert!(matches!(
        seller_sdk
            .trades()
            .enqueue_revision_proposal(TradeRevisionProposalEnqueueRequest::new(
                fixture_buyer_actor(),
                fixture_order_event_ptr(&root_event_id),
                fixture_order_event_ptr(&previous_event_id),
                proposal.clone(),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ))
            .await,
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
    assert!(matches!(
        buyer_sdk
            .trades()
            .enqueue_revision_decision(TradeRevisionDecisionEnqueueRequest::new(
                fixture_seller_actor(),
                fixture_order_event_ptr(&root_event_id),
                fixture_order_event_ptr(&previous_event_id),
                fixture_revision_decision(&proposal, &previous_event_id),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ))
            .await,
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
    assert!(matches!(
        buyer_sdk
            .trades()
            .enqueue_cancellation(TradeCancellationEnqueueRequest::new(
                fixture_seller_actor(),
                fixture_order_event_ptr(&root_event_id),
                fixture_order_event_ptr(&previous_event_id),
                fixture_cancellation("order-configured-prepare-cancel"),
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            ))
            .await,
        Err(RadrootsSdkError::UnauthorizedActor { .. })
    ));
}

#[tokio::test]
async fn configured_prepared_methods_report_missing_configured_signer_after_preflight() {
    let sdk = prepared_order_sdk().await;
    let buyer = fixture_buyer_actor();
    let seller = fixture_seller_actor();

    let submit_plan = sdk
        .trades()
        .prepare_submit(TradeSubmitPrepareRequest::new(
            buyer.clone(),
            fixture_event_ptr('a'),
            fixture_order_request("order-missing-configured-submit"),
        ))
        .expect("submit plan");
    assert!(matches!(
        sdk.trades()
            .enqueue_prepared_submit(
                &buyer,
                submit_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None
            )
            .await,
        Err(RadrootsSdkError::SignerUnavailable { .. })
    ));

    let decision_submit = enqueue_fixture_submit(&sdk, "order-missing-configured-decision").await;
    let decision_plan = sdk
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&decision_submit.signed_event_id),
            fixture_order_decision("order-missing-configured-decision"),
        ))
        .expect("decision plan");
    assert!(matches!(
        sdk.trades()
            .enqueue_prepared_decision(
                &seller,
                decision_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None
            )
            .await,
        Err(RadrootsSdkError::SignerUnavailable { .. })
    ));

    let proposal_submit = enqueue_fixture_submit(&sdk, "order-missing-configured-proposal").await;
    let proposal_payload = fixture_revision_proposal(
        "order-missing-configured-proposal",
        &proposal_submit.signed_event_id,
        &proposal_submit.signed_event_id,
    );
    let proposal_plan = sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&proposal_submit.signed_event_id),
            fixture_order_event_ptr(&proposal_submit.signed_event_id),
            proposal_payload.clone(),
        ))
        .expect("proposal plan");
    assert!(matches!(
        sdk.trades()
            .enqueue_prepared_revision_proposal(
                &seller,
                proposal_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None
            )
            .await,
        Err(RadrootsSdkError::SignerUnavailable { .. })
    ));

    let revision_submit = enqueue_fixture_submit(&sdk, "order-missing-configured-revision").await;
    let revision_proposal_payload = fixture_revision_proposal(
        "order-missing-configured-revision",
        &revision_submit.signed_event_id,
        &revision_submit.signed_event_id,
    );
    let revision_proposal_plan = sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            revision_proposal_payload.clone(),
        ))
        .expect("revision proposal plan");
    let revision_proposal = sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            revision_proposal_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("revision proposal evidence");
    let revision_plan = sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            fixture_order_event_ptr(&revision_proposal.signed_event_id),
            fixture_revision_decision(
                &revision_proposal_payload,
                &revision_proposal.signed_event_id,
            ),
        ))
        .expect("revision plan");
    assert!(matches!(
        sdk.trades()
            .enqueue_prepared_revision_decision(
                &buyer,
                revision_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None
            )
            .await,
        Err(RadrootsSdkError::SignerUnavailable { .. })
    ));

    let cancellation_submit = enqueue_fixture_submit(&sdk, "order-missing-configured-cancel").await;
    let cancellation_plan = sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer.clone(),
            fixture_order_event_ptr(&cancellation_submit.signed_event_id),
            fixture_order_event_ptr(&cancellation_submit.signed_event_id),
            fixture_cancellation("order-missing-configured-cancel"),
        ))
        .expect("cancellation plan");
    assert!(matches!(
        sdk.trades()
            .enqueue_prepared_cancellation(
                &buyer,
                cancellation_plan,
                fixture_target_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                None
            )
            .await,
        Err(RadrootsSdkError::SignerUnavailable { .. })
    ));
}

#[tokio::test]
async fn lifecycle_preflight_helpers_map_projection_query_failures() {
    let sdk = prepared_order_sdk().await;
    let client = sdk.trades();
    let buyer = fixture_buyer_actor();
    let seller = fixture_seller_actor();
    let root_event_id = event_id('e');
    let previous_event_id = event_id('f');
    let root = fixture_order_event_ptr(&root_event_id);
    let previous = fixture_order_event_ptr(&previous_event_id);
    let proposal = fixture_revision_proposal(
        "order-preflight-query-failure",
        &root_event_id,
        &previous_event_id,
    );

    let decision_plan = client
        .prepare_decision(TradeDecisionPrepareRequest::new(
            seller.clone(),
            root.clone(),
            fixture_order_decision("order-preflight-query-failure"),
        ))
        .expect("decision plan");
    let proposal_plan = client
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller,
            root.clone(),
            previous.clone(),
            proposal.clone(),
        ))
        .expect("proposal plan");
    let revision_plan = client
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            root.clone(),
            previous.clone(),
            fixture_revision_decision(&proposal, &previous_event_id),
        ))
        .expect("revision plan");
    let cancellation_plan = client
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer,
            root,
            previous,
            fixture_cancellation("order-preflight-query-failure"),
        ))
        .expect("cancellation plan");

    sdk._event_store.pool().close().await;
    assert!(matches!(
        client.require_decision_preflight(&decision_plan).await,
        Err(RadrootsSdkError::Projection { .. })
    ));
    assert!(matches!(
        client
            .require_revision_proposal_preflight(&proposal_plan)
            .await,
        Err(RadrootsSdkError::Projection { .. })
    ));
    assert!(matches!(
        client
            .require_revision_decision_preflight(&revision_plan)
            .await,
        Err(RadrootsSdkError::Projection { .. })
    ));
    assert!(matches!(
        client
            .require_cancellation_preflight(&cancellation_plan)
            .await,
        Err(RadrootsSdkError::Projection { .. })
    ));
}

#[tokio::test]
async fn prepared_lifecycle_enqueues_report_closed_outbox_after_preflight() {
    let proposal_sdk = prepared_order_sdk().await;
    let proposal_submit =
        enqueue_fixture_submit(&proposal_sdk, "order-closed-outbox-proposal").await;
    let seller = fixture_seller_actor();
    let proposal_payload = fixture_revision_proposal(
        "order-closed-outbox-proposal",
        &proposal_submit.signed_event_id,
        &proposal_submit.signed_event_id,
    );
    let proposal_plan = proposal_sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&proposal_submit.signed_event_id),
            fixture_order_event_ptr(&proposal_submit.signed_event_id),
            proposal_payload,
        ))
        .expect("proposal plan");
    let proposal_events_before = local_event_count(&proposal_sdk).await;
    proposal_sdk._outbox.pool().close().await;
    let proposal_error = proposal_sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            proposal_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("closed outbox proposal");
    assert_outbox_preflight_error(proposal_error);
    assert_eq!(
        local_event_count(&proposal_sdk).await,
        proposal_events_before
    );

    let revision_sdk = prepared_order_sdk().await;
    let revision_submit =
        enqueue_fixture_submit(&revision_sdk, "order-closed-outbox-revision").await;
    let proposal_payload = fixture_revision_proposal(
        "order-closed-outbox-revision",
        &revision_submit.signed_event_id,
        &revision_submit.signed_event_id,
    );
    let proposal_plan = revision_sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            proposal_payload.clone(),
        ))
        .expect("revision proposal plan");
    let proposal = revision_sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            proposal_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue proposal");
    let buyer = fixture_buyer_actor();
    let revision_plan = revision_sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            fixture_order_event_ptr(&proposal.signed_event_id),
            fixture_revision_decision(&proposal_payload, &proposal.signed_event_id),
        ))
        .expect("revision plan");
    let revision_events_before = local_event_count(&revision_sdk).await;
    revision_sdk._outbox.pool().close().await;
    let revision_error = revision_sdk
        .trades()
        .enqueue_prepared_revision_decision_with_explicit_signer(
            &buyer,
            revision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("closed outbox revision");
    assert_outbox_preflight_error(revision_error);
    assert_eq!(
        local_event_count(&revision_sdk).await,
        revision_events_before
    );

    let cancellation_sdk = prepared_order_sdk().await;
    let cancellation_submit =
        enqueue_fixture_submit(&cancellation_sdk, "order-closed-outbox-cancellation").await;
    let cancellation_plan = cancellation_sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer.clone(),
            fixture_order_event_ptr(&cancellation_submit.signed_event_id),
            fixture_order_event_ptr(&cancellation_submit.signed_event_id),
            fixture_cancellation("order-closed-outbox-cancellation"),
        ))
        .expect("cancellation plan");
    let cancellation_events_before = local_event_count(&cancellation_sdk).await;
    cancellation_sdk._outbox.pool().close().await;
    let cancellation_error = cancellation_sdk
        .trades()
        .enqueue_prepared_cancellation_with_explicit_signer(
            &buyer,
            cancellation_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("closed outbox cancellation");
    assert_outbox_preflight_error(cancellation_error);
    assert_eq!(
        local_event_count(&cancellation_sdk).await,
        cancellation_events_before
    );
}

#[tokio::test]
async fn prepared_lifecycle_enqueues_skip_preflight_for_existing_events() {
    let decision_sdk = configured_order_sdk(SELLER_SECRET_KEY_HEX).await;
    let decision_submit = enqueue_fixture_submit(&decision_sdk, "order-existing-decision").await;
    let seller = fixture_seller_actor();
    let decision_plan = decision_sdk
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&decision_submit.signed_event_id),
            fixture_order_decision("order-existing-decision"),
        ))
        .expect("decision plan");
    decision_sdk
        .trades()
        .enqueue_prepared_decision_with_explicit_signer(
            &seller,
            decision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue decision");
    let decision_repeat = decision_sdk
        .trades()
        .enqueue_prepared_decision_with_explicit_signer(
            &seller,
            decision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("repeat decision replay");
    assert_eq!(
        decision_repeat.workflow.state,
        SdkMutationState::AlreadyQueued
    );
    assert!(
        decision_repeat
            .workflow
            .idempotency
            .replayed_existing_operation
    );
    let configured_decision_repeat = decision_sdk
        .trades()
        .enqueue_prepared_decision(
            &seller,
            decision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
        )
        .await
        .expect("configured decision replay");
    assert_eq!(
        configured_decision_repeat.workflow.state,
        SdkMutationState::AlreadyQueued
    );
    assert!(
        configured_decision_repeat
            .workflow
            .idempotency
            .replayed_existing_operation
    );

    let proposal_sdk = configured_order_sdk(SELLER_SECRET_KEY_HEX).await;
    let proposal_submit = enqueue_fixture_submit(&proposal_sdk, "order-existing-proposal").await;
    let proposal_payload = fixture_revision_proposal(
        "order-existing-proposal",
        &proposal_submit.signed_event_id,
        &proposal_submit.signed_event_id,
    );
    let proposal_plan = proposal_sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&proposal_submit.signed_event_id),
            fixture_order_event_ptr(&proposal_submit.signed_event_id),
            proposal_payload,
        ))
        .expect("proposal plan");
    proposal_sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            proposal_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue proposal");
    let proposal_repeat = proposal_sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            proposal_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("repeat proposal replay");
    assert_eq!(
        proposal_repeat.workflow.state,
        SdkMutationState::AlreadyQueued
    );
    assert!(
        proposal_repeat
            .workflow
            .idempotency
            .replayed_existing_operation
    );
    let configured_proposal_repeat = proposal_sdk
        .trades()
        .enqueue_prepared_revision_proposal(
            &seller,
            proposal_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
        )
        .await
        .expect("configured proposal replay");
    assert_eq!(
        configured_proposal_repeat.workflow.state,
        SdkMutationState::AlreadyQueued
    );
    assert!(
        configured_proposal_repeat
            .workflow
            .idempotency
            .replayed_existing_operation
    );

    let revision_sdk = configured_order_sdk(BUYER_SECRET_KEY_HEX).await;
    let revision_submit = enqueue_fixture_submit(&revision_sdk, "order-existing-revision").await;
    let proposal_payload = fixture_revision_proposal(
        "order-existing-revision",
        &revision_submit.signed_event_id,
        &revision_submit.signed_event_id,
    );
    let proposal_plan = revision_sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            seller.clone(),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            proposal_payload.clone(),
        ))
        .expect("revision proposal plan");
    let proposal = revision_sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &seller,
            proposal_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue proposal");
    let buyer = fixture_buyer_actor();
    let revision_plan = revision_sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            buyer.clone(),
            fixture_order_event_ptr(&revision_submit.signed_event_id),
            fixture_order_event_ptr(&proposal.signed_event_id),
            fixture_revision_decision(&proposal_payload, &proposal.signed_event_id),
        ))
        .expect("revision plan");
    revision_sdk
        .trades()
        .enqueue_prepared_revision_decision_with_explicit_signer(
            &buyer,
            revision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue revision");
    let revision_repeat = revision_sdk
        .trades()
        .enqueue_prepared_revision_decision_with_explicit_signer(
            &buyer,
            revision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("repeat revision replay");
    assert_eq!(
        revision_repeat.workflow.state,
        SdkMutationState::AlreadyQueued
    );
    assert!(
        revision_repeat
            .workflow
            .idempotency
            .replayed_existing_operation
    );
    let configured_revision_repeat = revision_sdk
        .trades()
        .enqueue_prepared_revision_decision(
            &buyer,
            revision_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
        )
        .await
        .expect("configured revision replay");
    assert_eq!(
        configured_revision_repeat.workflow.state,
        SdkMutationState::AlreadyQueued
    );
    assert!(
        configured_revision_repeat
            .workflow
            .idempotency
            .replayed_existing_operation
    );

    let cancellation_sdk = configured_order_sdk(BUYER_SECRET_KEY_HEX).await;
    let cancellation_submit =
        enqueue_fixture_submit(&cancellation_sdk, "order-existing-cancellation").await;
    let cancellation_plan = cancellation_sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            buyer.clone(),
            fixture_order_event_ptr(&cancellation_submit.signed_event_id),
            fixture_order_event_ptr(&cancellation_submit.signed_event_id),
            fixture_cancellation("order-existing-cancellation"),
        ))
        .expect("cancellation plan");
    cancellation_sdk
        .trades()
        .enqueue_prepared_cancellation_with_explicit_signer(
            &buyer,
            cancellation_plan.clone(),
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
            &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue cancellation");
    let cancellation_repeat = cancellation_sdk
        .trades()
        .enqueue_prepared_cancellation(
            &buyer,
            cancellation_plan,
            fixture_target_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            None,
        )
        .await
        .expect("configured cancellation replay");
    assert_eq!(
        cancellation_repeat.workflow.state,
        SdkMutationState::AlreadyQueued
    );
    assert!(
        cancellation_repeat
            .workflow
            .idempotency
            .replayed_existing_operation
    );
}

#[tokio::test]
async fn order_ingest_and_enqueue_wrappers_report_prepare_timestamp_errors() {
    let sdk = prepared_order_sdk().await;
    let out_of_range = RadrootsSdkTimestamp::from_unix_seconds(u64::MAX);
    let buyer = fixture_buyer_actor();
    let seller = fixture_seller_actor();

    assert!(matches!(
        sdk.trades()
            .ingest_evidence(
                TradeEvidenceIngestRequest::new(request_event()).with_observed_at(out_of_range,)
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        sdk.trades()
            .ingest_request_evidence(
                TradeRequestEvidenceIngestRequest::new(request_event())
                    .with_observed_at(out_of_range,),
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        sdk.trades()
            .enqueue_submit_with_explicit_signer(
                TradeSubmitEnqueueRequest::new(
                    buyer.clone(),
                    fixture_event_ptr('a'),
                    fixture_order_request("order-wrapper-submit-error"),
                    fixture_target_relays(),
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .with_created_at(out_of_range),
                &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        sdk.trades()
            .enqueue_decision_with_explicit_signer(
                TradeDecisionEnqueueRequest::new(
                    seller.clone(),
                    fixture_event_ptr('b'),
                    fixture_order_decision("order-wrapper-decision-error"),
                    fixture_target_relays(),
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .with_created_at(out_of_range),
                &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));

    let root_event_id = event_id('c');
    let previous_event_id = event_id('d');
    let proposal_payload = fixture_revision_proposal(
        "order-wrapper-proposal-error",
        &root_event_id,
        &previous_event_id,
    );
    assert!(matches!(
        sdk.trades()
            .enqueue_revision_proposal_with_explicit_signer(
                TradeRevisionProposalEnqueueRequest::new(
                    seller.clone(),
                    ptr(root_event_id.as_str().to_owned()),
                    ptr(previous_event_id.as_str().to_owned()),
                    proposal_payload.clone(),
                    fixture_target_relays(),
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .with_created_at(out_of_range),
                &OrderFixtureSigner::new(SELLER_SECRET_KEY_HEX),
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        sdk.trades()
            .enqueue_revision_decision_with_explicit_signer(
                TradeRevisionDecisionEnqueueRequest::new(
                    buyer.clone(),
                    ptr(root_event_id.as_str().to_owned()),
                    ptr(previous_event_id.as_str().to_owned()),
                    fixture_revision_decision(&proposal_payload, &previous_event_id),
                    fixture_target_relays(),
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .with_created_at(out_of_range),
                &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
    assert!(matches!(
        sdk.trades()
            .enqueue_cancellation_with_explicit_signer(
                TradeCancellationEnqueueRequest::new(
                    buyer,
                    ptr(root_event_id.as_str().to_owned()),
                    ptr(previous_event_id.as_str().to_owned()),
                    fixture_cancellation("order-wrapper-cancellation-error"),
                    fixture_target_relays(),
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .with_created_at(out_of_range),
                &OrderFixtureSigner::new(BUYER_SECRET_KEY_HEX),
            )
            .await,
        Err(RadrootsSdkError::TimestampOutOfRange { .. })
    ));
}

#[tokio::test]
async fn order_default_timestamp_paths_report_clock_errors() {
    let clock_error_sdk = crate::RadrootsClient::builder()
        .clock(crate::RadrootsSdkClock::BeforeUnixEpoch)
        .build()
        .await
        .expect("sdk");
    let buyer = buyer_actor();
    let seller = seller_actor();
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');
    let proposal = revision_proposal_payload(&root_event_id, &previous_event_id);

    assert!(matches!(
        clock_error_sdk
            .trades()
            .ingest_evidence(TradeEvidenceIngestRequest::new(request_event()))
            .await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert!(matches!(
        clock_error_sdk
            .trades()
            .ingest_request_evidence(TradeRequestEvidenceIngestRequest::new(request_event()))
            .await,
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert!(matches!(
        clock_error_sdk
            .trades()
            .prepare_submit(TradeSubmitPrepareRequest::new(
                buyer.clone(),
                ptr(root_event_id.as_str().to_owned()),
                order_request_payload(),
            )),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert!(matches!(
        clock_error_sdk
            .trades()
            .prepare_decision(TradeDecisionPrepareRequest::new(
                seller.clone(),
                ptr(root_event_id.as_str().to_owned()),
                order_decision_payload(),
            )),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert!(matches!(
        clock_error_sdk.trades().prepare_revision_proposal(
            TradeRevisionProposalPrepareRequest::new(
                seller,
                ptr(root_event_id.as_str().to_owned()),
                ptr(previous_event_id.as_str().to_owned()),
                proposal.clone(),
            ),
        ),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert!(matches!(
        clock_error_sdk.trades().prepare_revision_decision(
            TradeRevisionDecisionPrepareRequest::new(
                buyer.clone(),
                ptr(root_event_id.as_str().to_owned()),
                ptr(previous_event_id.as_str().to_owned()),
                revision_decision_payload(
                    &proposal,
                    &previous_event_id,
                    RadrootsOrderRevisionOutcome::Accepted,
                ),
            ),
        ),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
    assert!(matches!(
        clock_error_sdk
            .trades()
            .prepare_cancellation(TradeCancellationPrepareRequest::new(
                buyer,
                ptr(root_event_id.as_str().to_owned()),
                ptr(previous_event_id.as_str().to_owned()),
                cancellation_payload(),
            )),
        Err(RadrootsSdkError::ClockBeforeUnixEpoch)
    ));
}

#[test]
fn order_runtime_request_builders_and_serializers_cover_source_attached_paths() {
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_321);
    let root_event_id = event_id('a');
    let previous_event_id = event_id('b');
    let root_event = ptr(root_event_id.as_str().to_owned());
    let previous_event = ptr(previous_event_id.as_str().to_owned());
    let proposal = revision_proposal_payload(&root_event_id, &previous_event_id);
    let revision_decision = revision_decision_payload(
        &proposal,
        &event_id('c'),
        RadrootsOrderRevisionOutcome::Declined {
            reason: "not workable".to_owned(),
        },
    );
    let policy = TargetPolicy::UseConfiguredProfile;

    let submit_prepare =
        TradeSubmitPrepareRequest::new(buyer_actor(), root_event.clone(), order_request_payload())
            .with_created_at(created_at);
    assert_struct_serialize_error_paths(&submit_prepare, 4);
    assert_eq!(
        serde_json::to_value(&submit_prepare).expect("submit prepare json")["created_at"],
        1_700_000_321
    );

    let submit_enqueue = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        root_event.clone(),
        order_request_payload(),
        policy.clone(),
        PublishMode::EnqueueOnly,
        SatisfactionPolicy::NoWait,
    )
    .try_with_target_relays(["wss://relay-a.radroots.test"], NostrRelayUrlPolicy::Public)
    .expect("submit relays")
    .with_idempotency_key(SdkIdempotencyKey::new("submit-unit-key").expect("key"))
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&submit_enqueue, 6);

    let request_ingest =
        TradeRequestEvidenceIngestRequest::new(request_event()).with_observed_at(created_at);
    assert_struct_serialize_error_paths(&request_ingest, 2);
    let evidence_ingest =
        TradeEvidenceIngestRequest::new(request_event()).with_observed_at(created_at);
    assert_struct_serialize_error_paths(&evidence_ingest, 2);

    let decision_prepare = TradeDecisionPrepareRequest::new(
        seller_actor(),
        root_event.clone(),
        order_decision_payload(),
    )
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&decision_prepare, 4);

    let decision_enqueue = TradeDecisionEnqueueRequest::new(
        seller_actor(),
        root_event.clone(),
        order_decision_payload(),
        policy.clone(),
        PublishMode::EnqueueOnly,
        SatisfactionPolicy::NoWait,
    )
    .try_with_target_relays(["wss://relay-b.radroots.test"], NostrRelayUrlPolicy::Public)
    .expect("decision relays")
    .with_idempotency_key(SdkIdempotencyKey::new("decision-unit-key").expect("key"))
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&decision_enqueue, 6);

    let proposal_prepare = TradeRevisionProposalPrepareRequest::new(
        seller_actor(),
        root_event.clone(),
        previous_event.clone(),
        proposal.clone(),
    )
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&proposal_prepare, 5);

    let proposal_enqueue = TradeRevisionProposalEnqueueRequest::new(
        seller_actor(),
        root_event.clone(),
        previous_event.clone(),
        proposal.clone(),
        policy.clone(),
        PublishMode::EnqueueOnly,
        SatisfactionPolicy::NoWait,
    )
    .try_with_target_relays(["wss://relay-c.radroots.test"], NostrRelayUrlPolicy::Public)
    .expect("proposal relays")
    .with_idempotency_key(SdkIdempotencyKey::new("proposal-unit-key").expect("key"))
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&proposal_enqueue, 7);

    let revision_decision_prepare = TradeRevisionDecisionPrepareRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        revision_decision.clone(),
    )
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&revision_decision_prepare, 5);

    let revision_decision_enqueue = TradeRevisionDecisionEnqueueRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        revision_decision,
        policy.clone(),
        PublishMode::EnqueueOnly,
        SatisfactionPolicy::NoWait,
    )
    .try_with_target_relays(["wss://relay-d.radroots.test"], NostrRelayUrlPolicy::Public)
    .expect("revision decision relays")
    .with_idempotency_key(SdkIdempotencyKey::new("revision-decision-unit-key").expect("key"))
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&revision_decision_enqueue, 7);

    let cancellation_prepare = TradeCancellationPrepareRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        cancellation_payload(),
    )
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&cancellation_prepare, 5);

    let cancellation_enqueue = TradeCancellationEnqueueRequest::new(
        buyer_actor(),
        root_event,
        previous_event,
        cancellation_payload(),
        policy,
        PublishMode::EnqueueOnly,
        SatisfactionPolicy::NoWait,
    )
    .try_with_target_relays(["wss://relay-e.radroots.test"], NostrRelayUrlPolicy::Public)
    .expect("cancellation relays")
    .with_idempotency_key(SdkIdempotencyKey::new("cancellation-unit-key").expect("key"))
    .with_created_at(created_at);
    assert_struct_serialize_error_paths(&cancellation_enqueue, 7);

    let parsed_status = TradeStatusRequest::parse(order_id().as_str())
        .expect("status request")
        .with_limit(TRADE_STATUS_DEFAULT_LIMIT);
    parsed_status.validate().expect("status validates");
    assert_eq!(
        serde_json::to_value(&parsed_status).expect("status json")["limit"],
        TRADE_STATUS_DEFAULT_LIMIT
    );

    let issue =
        SdkTradeStatusIssue::single(SdkTradeStatusIssueKind::ForkedLifecycle, event_id('f'));
    assert_eq!(issue.code(), "forked_lifecycle");
    assert_struct_serialize_error_paths(&issue, 3);
}

#[tokio::test]
async fn closed_event_store_errors_are_mapped_for_ingest_and_prepared_lookup() {
    let sdk = crate::RadrootsClient::builder().build().await.expect("sdk");
    sdk._event_store.pool().close().await;
    let ingest_error = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(request_event()))
        .await
        .expect_err("closed ingest evidence");
    assert!(matches!(ingest_error, RadrootsSdkError::EventStore { .. }));

    let sdk = crate::RadrootsClient::builder().build().await.expect("sdk");
    sdk._event_store.pool().close().await;
    let request_ingest_error = sdk
        .trades()
        .ingest_request_evidence(TradeRequestEvidenceIngestRequest::new(request_event()))
        .await
        .expect_err("closed request evidence ingest");
    assert!(matches!(
        request_ingest_error,
        RadrootsSdkError::EventStore { .. }
    ));

    let sdk = crate::RadrootsClient::builder().build().await.expect("sdk");
    sdk._event_store.pool().close().await;
    let lookup_error = sdk
        .trades()
        .prepared_order_event_exists(&event_id('a'))
        .await
        .expect_err("closed prepared lookup");
    assert!(matches!(lookup_error, RadrootsSdkError::EventStore { .. }));
}

#[tokio::test]
async fn order_status_and_evidence_ingest_cover_source_attached_success_paths() {
    let sdk = prepared_order_sdk().await;
    let request_event = request_event();
    let request_receipt = sdk
        .trades()
        .ingest_request_evidence(TradeRequestEvidenceIngestRequest::new(
            request_event.clone(),
        ))
        .await
        .expect("request evidence ingest");
    assert!(request_receipt.inserted);
    assert_eq!(request_receipt.order_id, order_id());

    let submit = enqueue_fixture_submit(&sdk, "order-status-source-attached").await;
    let status = sdk
        .trades()
        .status(TradeStatusRequest::parse(submit.order_id.as_str()).expect("status request"))
        .await
        .expect("order status");
    assert_eq!(status.status, TradeStatusKind::Requested);
    assert!(status.evidence.has_request);

    let duplicate_receipt = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(request_event))
        .await
        .expect("order evidence ingest");
    assert!(!duplicate_receipt.inserted);
    assert_eq!(
        duplicate_receipt.local_event_seq,
        request_receipt.local_event_seq
    );
}
