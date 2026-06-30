#![cfg(feature = "runtime")]

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
use std::path::Path;
use std::time::{Duration, Instant};

use radroots_authority::RadrootsActorContext;
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
};
use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
use radroots_events::{
    RadrootsNostrEvent, RadrootsNostrEventPtr,
    contract::RadrootsActorRole,
    ids::{
        RadrootsEventId, RadrootsListingAddress, RadrootsOrderId, RadrootsOrderRevisionId,
        RadrootsPublicKey,
    },
    kinds::{KIND_LISTING, KIND_ORDER_DECISION, KIND_ORDER_REQUEST},
    order::{
        RadrootsOrderDecision, RadrootsOrderDecisionOutcome, RadrootsOrderEconomicItem,
        RadrootsOrderEconomicLine, RadrootsOrderEconomics, RadrootsOrderInventoryCommitment,
        RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
        RadrootsOrderRevisionOutcome,
    },
};
use radroots_events_codec::wire::WireEventParts;
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_event,
};
use radroots_outbox::RadrootsOutbox;
use radroots_sdk::{
    AckPolicy, DvmValidationReceiptIngestRequest, PublishMode, RadrootsClient, RadrootsSdkError,
    RadrootsSdkPartialLocalMutationFailure, RadrootsSdkRecoveryAction, RadrootsSdkTimestamp,
    RelayResolutionPolicy, SdkMutationState, SdkRelayTargetSet, SdkRelayUrlPolicy,
    SdkTradeStatusIssue, SdkTradeStatusIssueKind, SdkTradeStatusSource, TRADE_STATUS_DEFAULT_LIMIT,
    TRADE_STATUS_MAX_LIMIT, TRADE_SUBMIT_OPERATION_KIND, TradeAcceptRequest, TradeCancelRequest,
    TradeDeclineRequest, TradeEvidenceIngestRequest, TradeMutationOutcome, TradeProposeRequest,
    TradeRequestEvidenceIngestRequest, TradeResyncRequest, TradeRevisionDecisionRequest,
    TradeRevisionProposalRequest, TradeSellerInboxRequest, TradeStatusKind,
    TradeStatusNextActionKind, TradeStatusRequest,
};
use radroots_sdk::{PrivacyPreflightConfirmation, PrivacyPreflightStatus, ProductSensitivityField};
#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
use radroots_sdk::{RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider};
use radroots_trade::order::RadrootsOrderIssue;
use radroots_trade::validation_receipt::{
    RadrootsTradeValidationReceipt, RadrootsValidationReceiptProof,
    RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
    RadrootsValidationReceiptStatement, RadrootsValidationReceiptType,
    validation_receipt_event_build, validation_receipt_public_values_hash_hex,
};
use serde::Serialize;
use serde::ser::{self, SerializeStruct};

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
#[cfg(any())]
const OTHER_PUBLIC_KEY_HEX: &str =
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
#[cfg(any())]
const RELAY_B: &str = "wss://relay-b.radroots.test";
const PERF_TOTAL_LOCAL_EVENTS: i64 = 100_000;
const PERF_TRADE_RELEVANT_EVENTS: i64 = 25_000;
const PERF_ACTIVE_TRADES: usize = 1_000;
const PERF_STATUS_P95_TARGET: Duration = Duration::from_millis(50);

#[derive(Clone, Copy)]
enum FailingSerializeFailure {
    Start,
    Field(usize),
    End,
}

struct FailingStructSerializer {
    failure: FailingSerializeFailure,
}

struct FailingSerializeStruct {
    field_index: usize,
    failure: FailingSerializeFailure,
}

#[derive(Debug)]
struct FailingSerializeError;

impl core::fmt::Display for FailingSerializeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("intentional serializer failure")
    }
}

impl std::error::Error for FailingSerializeError {}

impl ser::Error for FailingSerializeError {
    fn custom<T>(_message: T) -> Self
    where
        T: core::fmt::Display,
    {
        Self
    }
}

impl FailingStructSerializer {
    fn start() -> Self {
        Self {
            failure: FailingSerializeFailure::Start,
        }
    }

    fn field(field_index: usize) -> Self {
        Self {
            failure: FailingSerializeFailure::Field(field_index),
        }
    }

    fn end() -> Self {
        Self {
            failure: FailingSerializeFailure::End,
        }
    }
}

impl ser::Serializer for FailingStructSerializer {
    type Ok = ();
    type Error = FailingSerializeError;
    type SerializeSeq = ser::Impossible<(), FailingSerializeError>;
    type SerializeTuple = ser::Impossible<(), FailingSerializeError>;
    type SerializeTupleStruct = ser::Impossible<(), FailingSerializeError>;
    type SerializeTupleVariant = ser::Impossible<(), FailingSerializeError>;
    type SerializeMap = ser::Impossible<(), FailingSerializeError>;
    type SerializeStruct = FailingSerializeStruct;
    type SerializeStructVariant = ser::Impossible<(), FailingSerializeError>;

    fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_str(self, _value: &str) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(FailingSerializeError)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(FailingSerializeError)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(FailingSerializeError)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(FailingSerializeError)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        match self.failure {
            FailingSerializeFailure::Start => Err(FailingSerializeError),
            failure => Ok(FailingSerializeStruct {
                field_index: 0,
                failure,
            }),
        }
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(FailingSerializeError)
    }
}

impl SerializeStruct for FailingSerializeStruct {
    type Ok = ();
    type Error = FailingSerializeError;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.field_index += 1;
        match self.failure {
            FailingSerializeFailure::Field(field) if self.field_index == field => {
                Err(FailingSerializeError)
            }
            _ => Ok(()),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self.failure {
            FailingSerializeFailure::End => Err(FailingSerializeError),
            _ => Ok(()),
        }
    }
}

fn assert_struct_serialize_error_paths<T>(value: &T, field_count: usize)
where
    T: Serialize,
{
    value
        .serialize(FailingStructSerializer::start())
        .expect_err("struct start failure");
    for field_index in 1..=field_count {
        value
            .serialize(FailingStructSerializer::field(field_index))
            .expect_err("struct field failure");
    }
    value
        .serialize(FailingStructSerializer::end())
        .expect_err("struct end failure");
}

#[cfg(any())]
#[derive(Clone)]
struct FixtureSigner {
    identity: radroots_authority::RadrootsSignerIdentity,
    keys: RadrootsNostrKeys,
}

#[cfg(any())]
impl FixtureSigner {
    fn new(secret_key_hex: &str) -> Self {
        let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
        let keys = RadrootsNostrKeys::new(secret_key);
        let pubkey = keys.public_key().to_hex();
        Self {
            identity: radroots_authority::RadrootsSignerIdentity::new(pubkey).expect("identity"),
            keys,
        }
    }
}

#[cfg(any())]
impl radroots_authority::RadrootsEventSigner for FixtureSigner {
    fn pubkey(&self) -> &radroots_events::ids::RadrootsPublicKey {
        self.identity.pubkey()
    }

    fn sign_frozen_draft(
        &self,
        draft: &radroots_events::draft::RadrootsFrozenEventDraft,
    ) -> Result<
        radroots_events::draft::RadrootsSignedNostrEvent,
        radroots_authority::RadrootsSignerError,
    > {
        radroots_nostr::prelude::radroots_nostr_sign_frozen_draft(&self.keys, draft).map_err(
            |error| radroots_authority::RadrootsSignerError::SigningFailed {
                message: error.to_string(),
            },
        )
    }
}

async fn directory_sdk_and_store() -> (tempfile::TempDir, RadrootsClient, RadrootsEventStore) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsClient::builder()
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

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
async fn directory_sdk_with_signer(storage_root: &Path, secret_key_hex: &str) -> RadrootsClient {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    let signer_keys = RadrootsNostrKeys::new(secret_key);
    RadrootsClient::builder()
        .directory_storage(storage_root)
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::new(signer_keys).expect("local signer"),
        ))
        .build()
        .await
        .expect("sdk")
}

fn order_id(raw: &str) -> RadrootsOrderId {
    RadrootsOrderId::parse(raw).expect("order id")
}

fn status_request(raw: &str) -> TradeStatusRequest {
    TradeStatusRequest::parse(raw).expect("order status request")
}

fn buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(BUYER_PUBLIC_KEY_HEX, [RadrootsActorRole::Buyer]).expect("actor")
}

fn seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER_PUBLIC_KEY_HEX, [RadrootsActorRole::Seller]).expect("actor")
}

#[cfg(any())]
fn other_buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(OTHER_PUBLIC_KEY_HEX, [RadrootsActorRole::Buyer]).expect("actor")
}

#[cfg(any())]
fn other_seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(OTHER_PUBLIC_KEY_HEX, [RadrootsActorRole::Seller]).expect("actor")
}

#[cfg(any())]
fn non_buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(BUYER_PUBLIC_KEY_HEX, [RadrootsActorRole::Farmer]).expect("actor")
}

#[cfg(any())]
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

fn explicit_trade_relays() -> RelayResolutionPolicy {
    RelayResolutionPolicy::explicit(
        SdkRelayTargetSet::new([RELAY], SdkRelayUrlPolicy::Public).expect("target relays"),
    )
}

fn public_note_confirmation() -> PrivacyPreflightConfirmation {
    PrivacyPreflightConfirmation::new().confirm(ProductSensitivityField::PublicButSensitiveNotes)
}

fn expect_enqueued<Plan, Receipt>(outcome: TradeMutationOutcome<Plan, Receipt>) -> Receipt {
    match outcome {
        TradeMutationOutcome::Enqueued { receipt } => receipt,
        TradeMutationOutcome::DryRun { .. } => panic!("expected enqueue outcome"),
        TradeMutationOutcome::Published { .. } => panic!("expected enqueue outcome"),
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

#[cfg(any())]
fn invalid_listing_event_ptr() -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: String::new(),
        relays: Some(RELAY.to_owned()),
    }
}

#[cfg(any())]
#[tokio::test]
async fn order_submit_prepare_is_side_effect_free() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let listing_event = listing_event_ptr();
    let request = TradeSubmitPrepareRequest::new(
        buyer_actor(),
        listing_event.clone(),
        order_request("order-submit-prepare"),
    );

    let prepared = sdk.trades().prepare_submit(request).expect("prepared");

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

#[cfg(any())]
#[tokio::test]
async fn order_submit_prepare_rejects_missing_listing_evidence() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request = TradeSubmitPrepareRequest::new(
        buyer_actor(),
        invalid_listing_event_ptr(),
        order_request("order-submit-missing-listing"),
    );

    let error = sdk
        .trades()
        .prepare_submit(request)
        .expect_err("missing listing evidence");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
}

#[cfg(any())]
#[tokio::test]
async fn order_submit_prepare_rejects_invalid_actor_or_payload() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;

    let non_buyer = sdk
        .trades()
        .prepare_submit(TradeSubmitPrepareRequest::new(
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
        .trades()
        .prepare_submit(TradeSubmitPrepareRequest::new(
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
        .trades()
        .prepare_submit(TradeSubmitPrepareRequest::new(
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
        .trades()
        .prepare_submit(TradeSubmitPrepareRequest::new(
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
        .trades()
        .prepare_submit(TradeSubmitPrepareRequest::new(
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

#[cfg(any())]
#[tokio::test]
async fn order_submit_enqueue_stores_event_queues_outbox_and_status_sees_request() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let order = order_request("order-submit-enqueue");
    let prepared = sdk
        .trades()
        .prepare_submit(TradeSubmitPrepareRequest::new(
            buyer_actor(),
            listing_event_ptr(),
            order.clone(),
        ))
        .expect("prepared");
    assert_eq!(prepared.workflow.kind, TradeWorkflowKind::Submit);
    assert_eq!(
        prepared.workflow.operation_kind,
        TRADE_SUBMIT_OPERATION_KIND
    );
    assert_eq!(prepared.workflow.contract_id, "radroots.order.request.v1");
    assert_eq!(
        prepared.workflow.expected_event_id,
        prepared.expected_event_id
    );
    assert_eq!(prepared.workflow.created_at, prepared.created_at);
    let request = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order,
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays")
    .try_with_idempotency_key("order-submit-enqueue-idempotency")
    .expect("idempotency key");

    let receipt = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(request, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("enqueue");

    assert_eq!(receipt.order_id, prepared.order_id);
    assert_eq!(receipt.listing_addr, prepared.listing_addr);
    assert_eq!(receipt.listing_event_id, prepared.listing_event_id);
    assert_eq!(receipt.workflow.kind, TradeWorkflowKind::Submit);
    assert_eq!(receipt.workflow.operation_kind, TRADE_SUBMIT_OPERATION_KIND);
    assert_eq!(
        receipt.workflow.expected_event_id,
        prepared.expected_event_id
    );
    assert_eq!(receipt.workflow.signed_event_id, receipt.signed_event_id);
    assert_eq!(receipt.workflow.local_event_seq, receipt.local_event_seq);
    assert_eq!(
        receipt.workflow.outbox_operation_id,
        receipt.outbox_operation_id
    );
    assert_eq!(receipt.workflow.outbox_event_id, receipt.outbox_event_id);
    assert_eq!(receipt.workflow.state, receipt.state);
    assert_eq!(
        receipt.workflow.idempotency_digest_prefix,
        receipt.idempotency_digest_prefix
    );
    assert_eq!(
        receipt.workflow.idempotency.digest_prefix,
        receipt.idempotency_digest_prefix
    );
    assert!(!receipt.workflow.idempotency.replayed_existing_operation);
    assert!(
        receipt
            .workflow
            .idempotency
            .safe_to_retry_with_same_idempotency_key
    );
    assert!(!receipt.workflow.retry.retryable_after_error);
    assert!(
        receipt
            .workflow
            .retry
            .safe_to_retry_enqueue_with_same_idempotency_key
    );
    assert!(receipt.workflow.retry.recovery_actions.is_empty());
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
        .trades()
        .status(status_request("order-submit-enqueue"))
        .await
        .expect("status");
    assert!(status.found);
    assert_eq!(status.status, TradeStatusKind::Requested);
    assert_eq!(status.event_count, 1);
    assert_eq!(
        status
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(receipt.signed_event_id.as_str())
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_clients_propose_inbox_accept_status_and_resync() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                TradeProposeRequest::new(
                    buyer_actor(),
                    listing_event_ptr(),
                    order_request("trade-product-facade-flow"),
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-facade-propose")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );

    assert_eq!(
        propose_receipt.order_id.as_str(),
        "trade-product-facade-flow"
    );
    assert_eq!(
        propose_receipt
            .locator
            .root_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(propose_receipt.signed_event_id.as_str())
    );
    assert_eq!(
        propose_receipt.locator.listing_addr,
        Some(listing_address())
    );
    assert_eq!(
        propose_receipt
            .locator
            .seller_pubkey
            .as_ref()
            .map(RadrootsPublicKey::as_str),
        Some(SELLER_PUBLIC_KEY_HEX)
    );

    let seller_sdk = directory_sdk_with_signer(storage_root.as_path(), SELLER_SECRET_KEY_HEX).await;
    let inbox = seller_sdk
        .trades()
        .seller()
        .inbox(TradeSellerInboxRequest::new(seller_actor()))
        .await
        .expect("seller inbox");
    assert_eq!(inbox.seller_pubkey.as_str(), SELLER_PUBLIC_KEY_HEX);
    assert_eq!(inbox.statuses.len(), 1);
    assert_eq!(inbox.statuses[0].status, TradeStatusKind::Requested);
    assert_eq!(
        inbox.statuses[0]
            .root_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(propose_receipt.signed_event_id.as_str())
    );

    let accept_receipt = expect_enqueued(
        seller_sdk
            .trades()
            .seller()
            .accept_trade(
                TradeAcceptRequest::new(
                    seller_actor(),
                    propose_receipt.locator.clone(),
                    vec![RadrootsOrderInventoryCommitment {
                        bin_id: "bin-1".parse().expect("bin id"),
                        bin_count: 2,
                    }],
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-facade-accept")
                .expect("accept idempotency"),
            )
            .await
            .expect("accept trade"),
    );

    assert_eq!(accept_receipt.order_id, propose_receipt.order_id);
    assert_eq!(accept_receipt.locator, propose_receipt.locator);
    assert_eq!(
        accept_receipt.request_event_id,
        propose_receipt.signed_event_id
    );
    let status = seller_sdk
        .trades()
        .status_client()
        .status(TradeStatusRequest::new(propose_receipt.locator.clone()))
        .await
        .expect("facade status");
    assert_eq!(status.status, TradeStatusKind::AgreedPendingRhi);
    assert_eq!(
        status
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(accept_receipt.signed_event_id.as_str())
    );

    let resync = seller_sdk
        .trades()
        .resync()
        .resync(TradeResyncRequest::new(propose_receipt.locator))
        .await
        .expect("facade resync");
    assert_eq!(resync.status.status, TradeStatusKind::AgreedPendingRhi);
    assert_eq!(
        resync.status.last_event_id,
        Some(accept_receipt.signed_event_id)
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_clients_resync_committed_after_rhi_validation_receipt() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let seller_sdk = directory_sdk_with_signer(storage_root.as_path(), SELLER_SECRET_KEY_HEX).await;
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                TradeProposeRequest::new(
                    buyer_actor(),
                    listing_event_ptr(),
                    order_request("trade-product-committed-resync"),
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-committed-resync-propose")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );
    let accept_receipt = expect_enqueued(
        seller_sdk
            .trades()
            .seller()
            .accept_trade(
                TradeAcceptRequest::new(
                    seller_actor(),
                    propose_receipt.locator.clone(),
                    vec![RadrootsOrderInventoryCommitment {
                        bin_id: "bin-1".parse().expect("bin id"),
                        bin_count: 2,
                    }],
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-committed-resync-accept")
                .expect("accept idempotency"),
            )
            .await
            .expect("accept trade"),
    );
    let receipt_event = signed_validation_receipt_event(
        "trade-product-committed-resync",
        &propose_receipt.listing_event_id,
        &propose_receipt.signed_event_id,
        &accept_receipt.signed_event_id,
        33,
    );
    let receipt_event_id = RadrootsEventId::parse(receipt_event.id.as_str()).expect("receipt id");

    let ingest = seller_sdk
        .dvm()
        .ingest_validation_receipt(
            DvmValidationReceiptIngestRequest::new(receipt_event)
                .with_expected_order_id(propose_receipt.order_id.clone())
                .with_expected_listing_event_id(propose_receipt.listing_event_id.clone())
                .with_expected_root_event_id(propose_receipt.signed_event_id.clone())
                .with_expected_target_event_id(accept_receipt.signed_event_id.clone()),
        )
        .await
        .expect("ingest validation receipt");
    assert!(ingest.inserted);
    assert_eq!(ingest.receipt_event_id, receipt_event_id);

    let seller_resync = seller_sdk
        .trades()
        .resync()
        .resync(TradeResyncRequest::new(propose_receipt.locator.clone()))
        .await
        .expect("seller resync");
    assert_eq!(seller_resync.status.status, TradeStatusKind::Committed);
    assert_eq!(
        seller_resync.status.rhi_receipt_event_id,
        Some(receipt_event_id.clone())
    );
    assert_eq!(
        seller_resync.status.last_event_id,
        Some(receipt_event_id.clone())
    );

    let buyer_resync = buyer_sdk
        .trades()
        .resync()
        .resync(TradeResyncRequest::new(propose_receipt.locator))
        .await
        .expect("buyer resync");
    assert_eq!(buyer_resync.status.status, TradeStatusKind::Committed);
    assert_eq!(
        buyer_resync.status.rhi_receipt_event_id,
        Some(receipt_event_id)
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_propose_idempotency_replays_same_payload_and_conflicts_different_payload() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let request = TradeProposeRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("trade-product-idempotent"),
        explicit_trade_relays(),
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_idempotency_key("trade-product-idempotent-key")
    .expect("idempotency");

    let first = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(request.clone())
            .await
            .expect("first proposal"),
    );
    let replay = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(request)
            .await
            .expect("replay proposal"),
    );
    assert_eq!(replay.state, SdkMutationState::AlreadyQueued);
    assert_eq!(replay.signed_event_id, first.signed_event_id);
    assert_eq!(replay.outbox_event_id, first.outbox_event_id);
    assert!(replay.workflow.idempotency.replayed_existing_operation);
    assert!(
        replay
            .workflow
            .idempotency
            .safe_to_retry_with_same_idempotency_key
    );

    let conflict = buyer_sdk
        .trades()
        .buyer()
        .propose_trade(
            TradeProposeRequest::new(
                buyer_actor(),
                listing_event_ptr(),
                order_request("trade-product-idempotent-conflict"),
                explicit_trade_relays(),
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_idempotency_key("trade-product-idempotent-key")
            .expect("conflict idempotency"),
        )
        .await
        .expect_err("different payload conflict");

    assert!(matches!(
        conflict,
        RadrootsSdkError::PartialLocalMutation(ref partial)
            if partial.stored
                && !partial.queued
                && partial.operation_kind == TRADE_SUBMIT_OPERATION_KIND
                && partial.failure == RadrootsSdkPartialLocalMutationFailure::OutboxIdempotencyConflict
                && partial.recovery == RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey
    ));
    assert_eq!(conflict.code(), "partial_local_mutation");
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_decline_requires_public_reason_privacy_confirmation() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                TradeProposeRequest::new(
                    buyer_actor(),
                    listing_event_ptr(),
                    order_request("trade-product-privacy-decline"),
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-privacy-decline-propose")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );
    let seller_sdk = directory_sdk_with_signer(storage_root.as_path(), SELLER_SECRET_KEY_HEX).await;

    let missing_confirmation = seller_sdk
        .trades()
        .seller()
        .decline_trade(TradeDeclineRequest::new(
            seller_actor(),
            propose_receipt.locator.clone(),
            "sold elsewhere",
            explicit_trade_relays(),
            PublishMode::EnqueueOnly,
            AckPolicy::NoWait,
        ))
        .await
        .expect_err("missing public note confirmation");

    let RadrootsSdkError::PrivacyPreflight {
        operation,
        status,
        fields,
    } = &missing_confirmation
    else {
        panic!("expected privacy preflight error");
    };
    assert_eq!(operation, "trade.decline");
    assert_eq!(
        *status,
        PrivacyPreflightStatus::ExplicitConfirmationRequired
    );
    assert_eq!(fields, &[ProductSensitivityField::PublicButSensitiveNotes]);
    assert_eq!(missing_confirmation.code(), "privacy_preflight");
    assert_eq!(
        missing_confirmation.detail_json()["detail"]["fields"][0],
        "public_but_sensitive_notes"
    );
    let store =
        RadrootsEventStore::open_file(&seller_sdk.storage_paths().expect("paths").event_store_path)
            .await
            .expect("event store");
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        1
    );

    let decline_receipt = expect_enqueued(
        seller_sdk
            .trades()
            .seller()
            .decline_trade(
                TradeDeclineRequest::new(
                    seller_actor(),
                    propose_receipt.locator.clone(),
                    "sold elsewhere",
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .with_privacy_confirmation(public_note_confirmation())
                .try_with_idempotency_key("trade-product-privacy-decline-confirmed")
                .expect("decline idempotency"),
            )
            .await
            .expect("confirmed decline"),
    );
    let status = seller_sdk
        .trades()
        .status(TradeStatusRequest::new(propose_receipt.locator))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::Declined);
    assert_eq!(
        status.decision_event_id,
        Some(decline_receipt.signed_event_id)
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_cancel_blocks_sensitive_fulfillment_reason_before_mutation() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                TradeProposeRequest::new(
                    buyer_actor(),
                    listing_event_ptr(),
                    order_request("trade-product-privacy-cancel"),
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-privacy-cancel-propose")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );

    let forbidden = buyer_sdk
        .trades()
        .buyer()
        .cancel_trade(
            TradeCancelRequest::new(
                buyer_actor(),
                propose_receipt.locator,
                "pickup address is 123 Farm Lane",
                explicit_trade_relays(),
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .with_privacy_confirmation(public_note_confirmation()),
        )
        .await
        .expect_err("forbidden public fulfillment details");

    let RadrootsSdkError::PrivacyPreflight {
        operation,
        status,
        fields,
    } = &forbidden
    else {
        panic!("expected privacy preflight error");
    };
    assert_eq!(operation, "trade.cancel");
    assert_eq!(*status, PrivacyPreflightStatus::ForbiddenPublicFields);
    assert!(fields.contains(&ProductSensitivityField::SensitiveFulfillmentDetails));
    let store =
        RadrootsEventStore::open_file(&buyer_sdk.storage_paths().expect("paths").event_store_path)
            .await
            .expect("event store");
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        1
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_cancel_enqueues_with_locator_and_updates_status() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                TradeProposeRequest::new(
                    buyer_actor(),
                    listing_event_ptr(),
                    order_request("trade-product-cancel"),
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-cancel-propose")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );

    let cancellation = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .cancel_trade(
                TradeCancelRequest::new(
                    buyer_actor(),
                    propose_receipt.locator.clone(),
                    "changed plan",
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .with_privacy_confirmation(public_note_confirmation())
                .try_with_idempotency_key("trade-product-cancel")
                .expect("cancel idempotency"),
            )
            .await
            .expect("cancel trade"),
    );

    assert_eq!(cancellation.locator, propose_receipt.locator);
    assert_eq!(cancellation.root_event_id, propose_receipt.signed_event_id);
    assert_eq!(
        cancellation.previous_event_id,
        propose_receipt.signed_event_id
    );
    let status = buyer_sdk
        .trades()
        .status(TradeStatusRequest::new(propose_receipt.locator))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::Cancelled);
    assert_eq!(
        status.cancellation_event_id,
        Some(cancellation.signed_event_id)
    );
    assert_eq!(status.next_action, TradeStatusNextActionKind::Terminal);
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_revision_lifecycle_uses_locator_and_updates_status() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let seller_sdk = directory_sdk_with_signer(storage_root.as_path(), SELLER_SECRET_KEY_HEX).await;
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                TradeProposeRequest::new(
                    buyer_actor(),
                    listing_event_ptr(),
                    order_request("trade-product-revision"),
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-revision-propose")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );
    let revision_id: RadrootsOrderRevisionId =
        "revision-product-revision".parse().expect("revision id");
    let proposal = expect_enqueued(
        seller_sdk
            .trades()
            .seller()
            .propose_revision(
                TradeRevisionProposalRequest::new(
                    seller_actor(),
                    propose_receipt.locator.clone(),
                    revision_id.clone(),
                    vec![RadrootsOrderItem {
                        bin_id: "bin-1".parse().expect("bin id"),
                        bin_count: 3,
                    }],
                    revision_economics(),
                    "increase quantity",
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .with_privacy_confirmation(public_note_confirmation())
                .try_with_idempotency_key("trade-product-revision-proposal")
                .expect("revision proposal idempotency"),
            )
            .await
            .expect("propose revision"),
    );
    let pending = buyer_sdk
        .trades()
        .status(TradeStatusRequest::new(propose_receipt.locator.clone()))
        .await
        .expect("pending revision status");
    assert_eq!(pending.status, TradeStatusKind::RevisionProposed);
    assert_eq!(
        pending.pending_revision_event_id,
        Some(proposal.signed_event_id.clone())
    );
    assert!(pending.eligibility.can_decide_revision);

    let decision = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .accept_revision(
                TradeRevisionDecisionRequest::new(
                    buyer_actor(),
                    propose_receipt.locator.clone(),
                    revision_id,
                    RadrootsOrderRevisionOutcome::Accepted,
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    AckPolicy::NoWait,
                )
                .try_with_idempotency_key("trade-product-revision-decision")
                .expect("revision decision idempotency"),
            )
            .await
            .expect("accept revision"),
    );
    assert_eq!(decision.locator, propose_receipt.locator);
    assert_eq!(decision.root_event_id, propose_receipt.signed_event_id);
    assert_eq!(decision.previous_event_id, proposal.signed_event_id);
    let status = buyer_sdk
        .trades()
        .status(TradeStatusRequest::new(propose_receipt.locator))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::AgreedPendingRhi);
    assert_eq!(status.last_event_id, Some(decision.signed_event_id));
    assert_eq!(status.pending_revision_event_id, None);
    assert_eq!(status.economics, Some(revision_economics()));
}

#[cfg(feature = "signer-adapters")]
#[tokio::test]
async fn trade_product_propose_dry_run_returns_plan_without_local_side_effects() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let outcome = sdk
        .trades()
        .buyer()
        .propose_trade(TradeProposeRequest::new(
            buyer_actor(),
            listing_event_ptr(),
            order_request("trade-product-dry-run"),
            explicit_trade_relays(),
            PublishMode::DryRun,
            AckPolicy::NoWait,
        ))
        .await
        .expect("dry-run proposal");
    let plan = match outcome {
        TradeMutationOutcome::DryRun { plan } => plan,
        TradeMutationOutcome::Enqueued { .. } => panic!("expected dry-run outcome"),
        TradeMutationOutcome::Published { .. } => panic!("expected dry-run outcome"),
    };

    assert_eq!(plan.order_id.as_str(), "trade-product-dry-run");
    assert_eq!(plan.frozen_draft.kind, KIND_ORDER_REQUEST);
    assert_eq!(plan.expected_event_id, plan.workflow.expected_event_id);
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").outbox_path)
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

#[cfg(any())]
#[tokio::test]
async fn order_submit_enqueue_returns_sanitized_signer_errors_before_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-wrong-signer"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let error = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
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

#[cfg(any())]
#[tokio::test]
async fn order_submit_enqueue_derives_order_independent_idempotency_key() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let first = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-idempotent"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY_B, RELAY, RELAY], SdkRelayUrlPolicy::Public)
    .expect("first target relays");
    let second = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-idempotent"),
        RelayResolutionPolicy::explicit(
            SdkRelayTargetSet::new([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
                .expect("second target relays"),
        ),
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    );

    let first_receipt = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(first, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("first enqueue");
    let second_receipt = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(second, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
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
    assert!(
        !first_receipt
            .workflow
            .idempotency
            .replayed_existing_operation
    );
    assert!(
        second_receipt
            .workflow
            .idempotency
            .replayed_existing_operation
    );
    assert!(
        second_receipt
            .workflow
            .idempotency
            .safe_to_retry_with_same_idempotency_key
    );
    assert!(
        second_receipt
            .workflow
            .retry
            .safe_to_retry_enqueue_with_same_idempotency_key
    );
    assert!(!second_receipt.workflow.retry.retryable_after_error);
    assert!(second_receipt.workflow.retry.recovery_actions.is_empty());

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

#[cfg(any())]
#[tokio::test]
async fn order_submit_enqueue_pushes_queued_event_with_mock_relay_sync() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let enqueue_request = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-sync"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");
    let enqueue_receipt = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(
            enqueue_request,
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
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

#[cfg(any())]
#[tokio::test]
async fn order_submit_enqueue_reports_partial_local_mutation_after_outbox_conflict() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let first = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-conflict-a"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("first target relays")
    .try_with_idempotency_key("order-submit-conflict-idempotency")
    .expect("first idempotency key");
    sdk.trades()
        .enqueue_submit_with_explicit_signer(first, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect("first enqueue");

    let second = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-conflict-b"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("second target relays")
    .try_with_idempotency_key("order-submit-conflict-idempotency")
    .expect("second idempotency key");
    let error = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(second, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
        .await
        .expect_err("partial");

    assert!(matches!(
        error,
        RadrootsSdkError::PartialLocalMutation(ref partial)
            if partial.stored
                && !partial.queued
                && partial.event_id.is_some()
                && partial.operation_kind == TRADE_SUBMIT_OPERATION_KIND
                && partial.idempotency_digest_prefix.is_some()
                && partial.failure == RadrootsSdkPartialLocalMutationFailure::OutboxIdempotencyConflict
                && partial.recovery == RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey
    ));
    assert!(error.retryable());
    assert_eq!(
        error.recovery_actions(),
        vec![RadrootsSdkRecoveryAction::RetryOperationWithSameIdempotencyKey]
    );
    let detail = error.detail_json();
    assert_eq!(detail["code"], "partial_local_mutation");
    assert_eq!(detail["retryable"], true);
    assert_eq!(
        detail["recovery_actions"],
        serde_json::json!(["retry_operation_with_same_idempotency_key"])
    );
    assert!(
        !error
            .to_string()
            .contains("order-submit-conflict-idempotency")
    );
}

#[cfg(any())]
#[tokio::test]
async fn order_submit_runtime_dtos_serialize_deterministically() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_123);
    let prepare_request = TradeSubmitPrepareRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-serialized"),
    )
    .with_created_at(created_at);
    let prepare_json = serde_json::to_value(&prepare_request).expect("prepare request json");
    assert_struct_serialize_error_paths(&prepare_request, 4);

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

    let enqueue_request = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-serialized-enqueue"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("relay targets")
    .with_idempotency_key(
        SdkIdempotencyKey::new("order-serialized-idempotency").expect("idempotency"),
    )
    .with_created_at(created_at);
    let enqueue_json = serde_json::to_value(&enqueue_request).expect("enqueue request json");
    assert_struct_serialize_error_paths(&enqueue_request, 6);

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

    let try_key_enqueue = TradeSubmitEnqueueRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order_request("order-submit-try-idempotency"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_idempotency_key("order-submit-try-key")
    .expect("try idempotency key");
    assert_eq!(
        serde_json::to_value(&try_key_enqueue).expect("try key request json")["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 20 })
    );

    let receipt = sdk
        .trades()
        .enqueue_submit_with_explicit_signer(
            enqueue_request,
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(
        receipt_json,
        serde_json::json!({
            "workflow": {
                "kind": "submit",
                "operation_kind": TRADE_SUBMIT_OPERATION_KIND,
                "expected_event_id": receipt.workflow.expected_event_id.as_str(),
                "signed_event_id": receipt.workflow.signed_event_id.as_str(),
                "local_event_seq": 1,
                "outbox_operation_id": 1,
                "outbox_event_id": 1,
                "state": "stored_and_queued",
                "idempotency_digest_prefix": receipt.workflow.idempotency_digest_prefix.as_deref(),
                "idempotency": {
                    "digest_prefix": receipt.workflow.idempotency.digest_prefix.as_deref(),
                    "replayed_existing_operation": false,
                    "safe_to_retry_with_same_idempotency_key": true
                },
                "retry": {
                    "retryable_after_error": false,
                    "safe_to_retry_enqueue_with_same_idempotency_key": true,
                    "recovery_actions": []
                }
            },
            "order_id": receipt.order_id.as_str(),
            "locator": {
                "trade_id": receipt.order_id.as_str(),
                "root_event_id": receipt.signed_event_id.as_str(),
                "listing_addr": receipt.listing_addr.as_str(),
                "buyer_pubkey": BUYER_PUBLIC_KEY_HEX,
                "seller_pubkey": SELLER_PUBLIC_KEY_HEX
            },
            "listing_addr": receipt.listing_addr.as_str(),
            "buyer_pubkey": BUYER_PUBLIC_KEY_HEX,
            "seller_pubkey": SELLER_PUBLIC_KEY_HEX,
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

#[cfg(any())]
fn order_revision_proposal(
    raw_order_id: &str,
    root_event_id: &RadrootsEventId,
    previous_event_id: &RadrootsEventId,
) -> RadrootsOrderRevisionProposal {
    RadrootsOrderRevisionProposal {
        revision_id: format!("revision-{raw_order_id}")
            .parse()
            .expect("revision id"),
        order_id: order_id(raw_order_id),
        listing_addr: listing_address(),
        buyer_pubkey: BUYER_PUBLIC_KEY_HEX.parse().expect("buyer pubkey"),
        seller_pubkey: SELLER_PUBLIC_KEY_HEX.parse().expect("seller pubkey"),
        root_event_id: root_event_id.clone(),
        prev_event_id: previous_event_id.clone(),
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 3,
        }],
        economics: revision_economics(),
        reason: "increase quantity".to_owned(),
    }
}

#[cfg(any())]
fn order_revision_decision(
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

#[cfg(any())]
fn order_cancellation(raw_order_id: &str) -> RadrootsOrderCancellation {
    RadrootsOrderCancellation {
        order_id: order_id(raw_order_id),
        listing_addr: listing_address(),
        buyer_pubkey: BUYER_PUBLIC_KEY_HEX.parse().expect("buyer pubkey"),
        seller_pubkey: SELLER_PUBLIC_KEY_HEX.parse().expect("seller pubkey"),
        reason: "buyer changed pickup plan".to_owned(),
    }
}

fn revision_economics() -> RadrootsOrderEconomics {
    RadrootsOrderEconomics {
        quote_id: "revision-quote-1".parse().expect("revision quote id"),
        quote_version: 2,
        pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
        currency: RadrootsCoreCurrency::USD,
        items: vec![RadrootsOrderEconomicItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 3,
            quantity_amount: decimal("1"),
            quantity_unit: RadrootsCoreUnit::Each,
            unit_price_amount: decimal("5"),
            unit_price_currency: RadrootsCoreCurrency::USD,
            line_subtotal: usd("15"),
        }],
        discounts: Vec::<RadrootsOrderEconomicLine>::new(),
        adjustments: Vec::<RadrootsOrderEconomicLine>::new(),
        subtotal: usd("15"),
        discount_total: usd("0"),
        adjustment_total: usd("0"),
        total: usd("15"),
    }
}

fn signed_validation_receipt_event(
    raw_order_id: &str,
    listing_event_id: &RadrootsEventId,
    root_event_id: &RadrootsEventId,
    target_event_id: &RadrootsEventId,
    created_at: u32,
) -> RadrootsNostrEvent {
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

async fn insert_perf_non_trade_events(store: &RadrootsEventStore, base: i64, count: i64) {
    let mut inserted = 0;
    while inserted < count {
        let batch = (count - inserted).min(1_000);
        sqlx::query(
            "WITH RECURSIVE seq(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM seq WHERE n + 1 < ?)
             INSERT INTO nostr_events(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms)
             SELECT lower(printf('%064x', ? + n)), ?, 1700000000 + n, 1, json_array(), '{}', ?, '{}', 'verified', 'unsupported_kind', NULL, NULL, 0, 1700000000000 + n, 1700000000000 + n FROM seq",
        )
        .bind(batch)
        .bind(base + inserted)
        .bind(SELLER_PUBLIC_KEY_HEX)
        .bind(perf_sig())
        .execute(store.pool())
        .await
        .expect("non-trade perf seed");
        inserted += batch;
    }
}

async fn insert_perf_trade_background_events(store: &RadrootsEventStore, base: i64, count: i64) {
    let mut inserted = 0;
    while inserted < count {
        let batch = (count - inserted).min(1_000);
        sqlx::query(
            "WITH RECURSIVE seq(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM seq WHERE n + 1 < ?)
             INSERT INTO nostr_events(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms)
             SELECT lower(printf('%064x', ? + n)), ?, 1700000000 + n, ?, json_array(json_array('d', 'perf-bg-' || printf('%06d', ? + n))), '{}', ?, '{}', 'verified', 'supported', 'radroots.order.request.v1', 'regular', 1, 1700000000000 + n, 1700000000000 + n FROM seq",
        )
        .bind(batch)
        .bind(base + inserted)
        .bind(BUYER_PUBLIC_KEY_HEX)
        .bind(i64::from(KIND_ORDER_REQUEST))
        .bind(base + inserted)
        .bind(perf_sig())
        .execute(store.pool())
        .await
        .expect("trade perf event seed");
        sqlx::query(
            "WITH RECURSIVE seq(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM seq WHERE n + 1 < ?)
             INSERT INTO nostr_event_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed)
             SELECT lower(printf('%064x', ? + n)), 0, 'd', 'perf-bg-' || printf('%06d', ? + n), json_array('d', 'perf-bg-' || printf('%06d', ? + n)), NULL, NULL, 0 FROM seq",
        )
        .bind(batch)
        .bind(base + inserted)
        .bind(base + inserted)
        .bind(base + inserted)
        .execute(store.pool())
        .await
        .expect("trade perf tag seed");
        inserted += batch;
    }
}

fn perf_sig() -> String {
    "0".repeat(128)
}

fn hash32(ch: char) -> String {
    format!("0x{}", ch.to_string().repeat(64))
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
    let draft = radroots_events_codec::order::order_request_event_build(
        &listing_event_ptr(),
        &order_request(raw_order_id),
    )
    .expect("request draft");
    signed_event(BUYER_SECRET_KEY_HEX, created_at, draft)
}

#[cfg(any())]
fn request_event_ptr(event: &RadrootsNostrEvent) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: event.id.clone(),
        relays: Some(RELAY.to_owned()),
    }
}

#[cfg(any())]
fn order_event_ptr(event_id: &RadrootsEventId) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: event_id.as_str().to_owned(),
        relays: Some(RELAY.to_owned()),
    }
}

#[cfg(any())]
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

fn signed_order_decision_event(
    raw_order_id: &str,
    root_event_id: &RadrootsEventId,
    created_at: u32,
) -> RadrootsNostrEvent {
    let draft = radroots_events_codec::order::order_decision_event_build(
        root_event_id,
        root_event_id,
        &order_decision(raw_order_id),
    )
    .expect("decision draft");
    signed_event(SELLER_SECRET_KEY_HEX, created_at, draft)
}

fn signed_non_order_event(created_at: u32) -> RadrootsNostrEvent {
    signed_event(
        SELLER_SECRET_KEY_HEX,
        created_at,
        WireEventParts {
            kind: KIND_LISTING,
            content: "{}".to_owned(),
            tags: vec![vec!["d".to_owned(), "not-an-order".to_owned()]],
        },
    )
}

#[cfg(any())]
#[tokio::test]
async fn order_request_evidence_ingest_stores_request_and_enables_decision_enqueue() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-decision-ingested", 39);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    let ingest_request = TradeRequestEvidenceIngestRequest::new(request_event.clone())
        .with_observed_at(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_039));

    let ingest_receipt = sdk
        .trades()
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

    let actor = seller_actor();
    let plan = sdk
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
            actor.clone(),
            request_event_ptr(&request_event),
            order_decision("order-decision-ingested"),
        ))
        .expect("prepare decision");
    let receipt = sdk
        .trades()
        .enqueue_prepared_decision_with_explicit_signer(
            &actor,
            plan,
            RelayResolutionPolicy::try_explicit([RELAY], SdkRelayUrlPolicy::Public)
                .expect("target relays"),
            PublishMode::EnqueueOnly,
            AckPolicy::NoWait,
            None,
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue decision");

    assert_eq!(receipt.local_event_seq, 2);
    let duplicate_receipt = sdk
        .trades()
        .ingest_request_evidence(TradeRequestEvidenceIngestRequest::new(
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
async fn order_evidence_ingest_stores_lifecycle_evidence_for_projection() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-evidence-ingest", 39);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    let decision_event =
        signed_order_decision_event("order-evidence-ingest", &request_event_id, 40);

    let request_receipt = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(request_event.clone()))
        .await
        .expect("request evidence");
    assert_eq!(request_receipt.order_id.as_str(), "order-evidence-ingest");
    assert_eq!(request_receipt.event_kind, KIND_ORDER_REQUEST);
    assert_eq!(request_receipt.local_event_seq, 1);
    assert!(request_receipt.inserted);

    let decision_receipt = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(decision_event.clone()))
        .await
        .expect("decision evidence");
    assert_eq!(decision_receipt.order_id.as_str(), "order-evidence-ingest");
    assert_eq!(decision_receipt.event_kind, KIND_ORDER_DECISION);
    assert_eq!(decision_receipt.local_event_seq, 2);
    assert!(decision_receipt.inserted);

    let duplicate_receipt = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(decision_event))
        .await
        .expect("duplicate decision evidence");
    assert_eq!(duplicate_receipt.local_event_seq, 2);
    assert!(!duplicate_receipt.inserted);
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        2
    );

    let status = sdk
        .trades()
        .status(status_request("order-evidence-ingest"))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::AgreedPendingRhi);
    assert_eq!(status.event_count, 2);
    assert_eq!(
        status
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(decision_receipt.event_id.as_str())
    );
}

#[tokio::test]
async fn order_evidence_ingest_rejects_non_order_events() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let error = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(signed_non_order_event(41)))
        .await
        .expect_err("non order event");

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
async fn order_request_evidence_ingest_rejects_non_request_events() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let root_event_id = deterministic_event_id("non-request-root");
    let decision_event = signed_order_decision_event("non-request-root", &root_event_id, 40);

    let error = sdk
        .trades()
        .ingest_request_evidence(TradeRequestEvidenceIngestRequest::new(decision_event))
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

#[cfg(any())]
#[tokio::test]
async fn order_decision_prepare_accept_and_decline_are_side_effect_free() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event_id = deterministic_event_id("order-decision-prepare-request");
    let request_event = RadrootsNostrEventPtr {
        id: request_event_id.as_str().to_owned(),
        relays: Some(RELAY.to_owned()),
    };
    let accepted_request = TradeDecisionPrepareRequest::new(
        seller_actor(),
        request_event.clone(),
        order_decision("order-decision-prepare-accept"),
    );

    let accepted = sdk
        .trades()
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
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
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

#[cfg(any())]
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
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
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
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
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
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
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
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
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
        .trades()
        .prepare_decision(TradeDecisionPrepareRequest::new(
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

#[cfg(any())]
#[tokio::test]
async fn order_decision_runtime_dtos_serialize_deterministically() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_321);
    let prepare_event_id = deterministic_event_id("order-decision-serialized-request");
    let prepare_request = TradeDecisionPrepareRequest::new(
        seller_actor(),
        RadrootsNostrEventPtr {
            id: prepare_event_id.as_str().to_owned(),
            relays: Some(RELAY.to_owned()),
        },
        order_decision("order-decision-serialized"),
    )
    .with_created_at(created_at);
    let prepare_json = serde_json::to_value(&prepare_request).expect("prepare request json");
    assert_struct_serialize_error_paths(&prepare_request, 4);

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
    let enqueue_request = TradeDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        order_decision("order-decision-serialized-enqueue"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("target relays")
    .with_idempotency_key(
        SdkIdempotencyKey::new("order-decision-serialized-idempotency").expect("idempotency"),
    )
    .with_created_at(created_at);
    let enqueue_json = serde_json::to_value(&enqueue_request).expect("enqueue request json");
    assert_struct_serialize_error_paths(&enqueue_request, 6);

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

    let try_key_enqueue = TradeDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        order_decision("order-decision-try-idempotency"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_idempotency_key("order-decision-try-key")
    .expect("try idempotency key");
    assert_eq!(
        serde_json::to_value(&try_key_enqueue).expect("try key request json")["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 22 })
    );

    let receipt = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(
            enqueue_request,
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue");
    assert_eq!(receipt.workflow.kind, TradeWorkflowKind::Decision);
    assert_eq!(
        receipt.workflow.operation_kind,
        TRADE_DECISION_OPERATION_KIND
    );
    assert_eq!(
        receipt.workflow.expected_event_id,
        receipt.expected_event_id
    );
    assert_eq!(receipt.workflow.signed_event_id, receipt.signed_event_id);
    assert_eq!(receipt.workflow.local_event_seq, receipt.local_event_seq);
    assert_eq!(
        receipt.workflow.outbox_operation_id,
        receipt.outbox_operation_id
    );
    assert_eq!(receipt.workflow.outbox_event_id, receipt.outbox_event_id);
    assert_eq!(receipt.workflow.state, receipt.state);
    assert_eq!(
        receipt.workflow.idempotency_digest_prefix,
        receipt.idempotency_digest_prefix
    );
    assert!(
        receipt
            .workflow
            .idempotency
            .safe_to_retry_with_same_idempotency_key
    );
    assert!(!receipt.workflow.retry.retryable_after_error);
    assert!(receipt.workflow.retry.recovery_actions.is_empty());
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(
        receipt_json,
        serde_json::json!({
            "workflow": {
                "kind": "decision",
                "operation_kind": TRADE_DECISION_OPERATION_KIND,
                "expected_event_id": receipt.workflow.expected_event_id.as_str(),
                "signed_event_id": receipt.workflow.signed_event_id.as_str(),
                "local_event_seq": 2,
                "outbox_operation_id": 1,
                "outbox_event_id": 1,
                "state": "stored_and_queued",
                "idempotency_digest_prefix": receipt.workflow.idempotency_digest_prefix.as_deref(),
                "idempotency": {
                    "digest_prefix": receipt.workflow.idempotency.digest_prefix.as_deref(),
                    "replayed_existing_operation": false,
                    "safe_to_retry_with_same_idempotency_key": true
                },
                "retry": {
                    "retryable_after_error": false,
                    "safe_to_retry_enqueue_with_same_idempotency_key": true,
                    "recovery_actions": []
                }
            },
            "order_id": receipt.order_id.as_str(),
            "locator": {
                "trade_id": receipt.order_id.as_str(),
                "root_event_id": request_event.id.as_str(),
                "listing_addr": receipt.listing_addr.as_str(),
                "buyer_pubkey": BUYER_PUBLIC_KEY_HEX,
                "seller_pubkey": SELLER_PUBLIC_KEY_HEX
            },
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

#[cfg(any())]
#[tokio::test]
async fn order_revision_and_cancellation_dtos_serialize_deterministically() {
    let created_at = RadrootsSdkTimestamp::from_unix_seconds(1_700_000_654);
    let root_event_id = deterministic_event_id("order-dto-root");
    let previous_event_id = deterministic_event_id("order-dto-previous");
    let root_event = order_event_ptr(&root_event_id);
    let previous_event = order_event_ptr(&previous_event_id);
    let proposal =
        order_revision_proposal("order-revision-dto", &root_event_id, &previous_event_id);
    let revision_decision = order_revision_decision(
        &proposal,
        &previous_event_id,
        RadrootsOrderRevisionOutcome::Declined {
            reason: "not available".to_owned(),
        },
    );
    let cancellation = order_cancellation("order-revision-dto");

    let proposal_prepare = TradeRevisionProposalPrepareRequest::new(
        seller_actor(),
        root_event.clone(),
        previous_event.clone(),
        proposal.clone(),
    )
    .with_created_at(created_at);
    let proposal_prepare_json =
        serde_json::to_value(&proposal_prepare).expect("proposal prepare json");
    assert_struct_serialize_error_paths(&proposal_prepare, 5);
    assert_eq!(
        proposal_prepare_json["actor"]["pubkey"],
        SELLER_PUBLIC_KEY_HEX
    );
    assert_eq!(
        proposal_prepare_json["root_event"]["id"],
        root_event_id.as_str()
    );
    assert_eq!(
        proposal_prepare_json["previous_event"]["id"],
        previous_event_id.as_str()
    );
    assert_eq!(
        proposal_prepare_json["proposal"]["order_id"],
        "order-revision-dto"
    );
    assert_eq!(
        proposal_prepare_json["proposal"]["reason"],
        "increase quantity"
    );
    assert_eq!(proposal_prepare_json["created_at"], 1_700_000_654);

    let proposal_enqueue = TradeRevisionProposalEnqueueRequest::new(
        seller_actor(),
        root_event.clone(),
        previous_event.clone(),
        proposal.clone(),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("proposal relays")
    .with_idempotency_key(SdkIdempotencyKey::new("order-revision-proposal-dto").expect("key"))
    .with_created_at(created_at);
    let proposal_enqueue_json =
        serde_json::to_value(&proposal_enqueue).expect("proposal enqueue json");
    assert_struct_serialize_error_paths(&proposal_enqueue, 7);
    assert_eq!(
        proposal_enqueue_json["target_relays"],
        serde_json::json!({
            "kind": "explicit",
            "relays": [RELAY, RELAY_B],
            "canonical_relays": [RELAY_B, RELAY]
        })
    );
    assert_eq!(
        proposal_enqueue_json["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 27 })
    );
    assert!(!proposal_enqueue_json.to_string().contains("proposal-dto"));

    let proposal_try_key = TradeRevisionProposalEnqueueRequest::new(
        seller_actor(),
        root_event.clone(),
        previous_event.clone(),
        proposal.clone(),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_idempotency_key("order-revision-proposal-try")
    .expect("proposal try key");
    assert_eq!(
        serde_json::to_value(&proposal_try_key).expect("proposal try json")["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 27 })
    );

    let decision_prepare = TradeRevisionDecisionPrepareRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        revision_decision.clone(),
    )
    .with_created_at(created_at);
    let decision_prepare_json =
        serde_json::to_value(&decision_prepare).expect("decision prepare json");
    assert_struct_serialize_error_paths(&decision_prepare, 5);
    assert_eq!(
        decision_prepare_json["actor"]["pubkey"],
        BUYER_PUBLIC_KEY_HEX
    );
    assert_eq!(
        decision_prepare_json["decision"]["revision_id"],
        proposal.revision_id.as_str()
    );
    assert_eq!(
        decision_prepare_json["decision"]["decision"],
        serde_json::json!({
            "decision": "declined",
            "reason": "not available"
        })
    );
    assert_eq!(decision_prepare_json["created_at"], 1_700_000_654);

    let decision_enqueue = TradeRevisionDecisionEnqueueRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        revision_decision,
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("decision relays")
    .with_idempotency_key(
        SdkIdempotencyKey::new("order-revision-decision-dto").expect("decision idempotency"),
    )
    .with_created_at(created_at);
    let decision_enqueue_json =
        serde_json::to_value(&decision_enqueue).expect("decision enqueue json");
    assert_struct_serialize_error_paths(&decision_enqueue, 7);
    assert_eq!(
        decision_enqueue_json["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 27 })
    );
    assert_eq!(decision_enqueue_json["created_at"], 1_700_000_654);
    assert!(!decision_enqueue_json.to_string().contains("decision-dto"));

    let decision_try_key = TradeRevisionDecisionEnqueueRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        order_revision_decision(
            &proposal,
            &previous_event_id,
            RadrootsOrderRevisionOutcome::Accepted,
        ),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_idempotency_key("order-revision-decision-try")
    .expect("decision try key");
    assert_eq!(
        serde_json::to_value(&decision_try_key).expect("decision try json")["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 27 })
    );

    let cancellation_prepare = TradeCancellationPrepareRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        cancellation.clone(),
    )
    .with_created_at(created_at);
    let cancellation_prepare_json =
        serde_json::to_value(&cancellation_prepare).expect("cancellation prepare json");
    assert_struct_serialize_error_paths(&cancellation_prepare, 5);
    assert_eq!(
        cancellation_prepare_json["cancellation"]["reason"],
        "buyer changed pickup plan"
    );
    assert_eq!(cancellation_prepare_json["created_at"], 1_700_000_654);

    let cancellation_enqueue = TradeCancellationEnqueueRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        cancellation,
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY, RELAY_B], SdkRelayUrlPolicy::Public)
    .expect("cancellation relays")
    .with_idempotency_key(
        SdkIdempotencyKey::new("order-cancellation-dto").expect("cancellation idempotency"),
    )
    .with_created_at(created_at);
    let cancellation_enqueue_json =
        serde_json::to_value(&cancellation_enqueue).expect("cancellation enqueue json");
    assert_struct_serialize_error_paths(&cancellation_enqueue, 7);
    assert_eq!(
        cancellation_enqueue_json["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 22 })
    );
    assert_eq!(cancellation_enqueue_json["created_at"], 1_700_000_654);
    assert!(
        !cancellation_enqueue_json
            .to_string()
            .contains("cancellation-dto")
    );

    let cancellation_try_key = TradeCancellationEnqueueRequest::new(
        buyer_actor(),
        root_event.clone(),
        previous_event.clone(),
        order_cancellation("order-revision-dto"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_idempotency_key("order-cancellation-try")
    .expect("cancellation try key");
    assert_eq!(
        serde_json::to_value(&cancellation_try_key).expect("cancellation try json")["idempotency_key"],
        serde_json::json!({ "value": "<redacted>", "len": 22 })
    );

    let event = signed_order_request_event("order-evidence-dto", 77);
    let request_evidence =
        TradeRequestEvidenceIngestRequest::new(event.clone()).with_observed_at(created_at);
    let request_evidence_json =
        serde_json::to_value(&request_evidence).expect("request evidence json");
    assert_struct_serialize_error_paths(&request_evidence, 2);
    assert_eq!(request_evidence_json["event"]["id"], event.id.as_str());
    assert_eq!(request_evidence_json["observed_at"], 1_700_000_654);

    let order_evidence =
        TradeEvidenceIngestRequest::new(event.clone()).with_observed_at(created_at);
    let order_evidence_json = serde_json::to_value(&order_evidence).expect("order evidence json");
    assert_struct_serialize_error_paths(&order_evidence, 2);
    assert_eq!(order_evidence_json["event"]["id"], event.id.as_str());
    assert_eq!(order_evidence_json["observed_at"], 1_700_000_654);
}

#[cfg(any())]
#[tokio::test]
async fn order_decision_enqueue_accept_stores_event_queues_outbox_and_updates_status() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-decision-accept", 40);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 4_000))
        .await
        .expect("ingest request");
    let request = TradeDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        order_decision("order-decision-accept"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays")
    .try_with_idempotency_key("order-decision-accept-idempotency")
    .expect("idempotency");

    let receipt = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
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
    assert_eq!(operation.operation_kind, TRADE_DECISION_OPERATION_KIND);
    let outbox_event = outbox
        .get_event(receipt.outbox_event_id)
        .await
        .expect("outbox event")
        .expect("outbox event");
    assert_eq!(outbox_event.state, RadrootsOutboxEventState::Signed);
    assert_eq!(outbox_event.draft.kind, KIND_ORDER_DECISION);
    assert!(outbox_event.signed_event.is_some());

    let status = sdk
        .trades()
        .status(status_request("order-decision-accept"))
        .await
        .expect("status");
    assert!(status.found);
    assert_eq!(status.status, TradeStatusKind::AgreedPendingRhi);
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
    assert_eq!(
        status
            .agreement_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(receipt.signed_event_id.as_str())
    );
    assert_eq!(status.pending_revision_event_id, None);
    assert!(status.issues.is_empty());
}

#[cfg(any())]
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
    let request = TradeDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        decision,
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let receipt = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
        .await
        .expect("enqueue");

    assert_eq!(receipt.state, SdkMutationState::StoredAndQueued);
    let status = sdk
        .trades()
        .status(status_request("order-decision-decline"))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::Declined);
    assert_eq!(
        status
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(receipt.signed_event_id.as_str())
    );
    assert!(status.issues.is_empty());
}

#[cfg(any())]
#[tokio::test]
async fn order_decision_enqueue_rejects_missing_request_evidence_before_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let missing_request = RadrootsNostrEventPtr {
        id: deterministic_event_id("missing-order-request")
            .as_str()
            .to_owned(),
        relays: Some(RELAY.to_owned()),
    };
    let request = TradeDecisionEnqueueRequest::new(
        seller_actor(),
        missing_request,
        order_decision("order-decision-missing-request"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let error = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
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

#[cfg(any())]
#[tokio::test]
async fn order_decision_enqueue_returns_sanitized_signer_errors_before_decision_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-decision-wrong-signer", 42);
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 4_200))
        .await
        .expect("ingest request");
    let request = TradeDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        order_decision("order-decision-wrong-signer"),
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let error = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(request, &FixtureSigner::new(BUYER_SECRET_KEY_HEX))
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

#[cfg(any())]
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
    let request = TradeDecisionEnqueueRequest::new(
        seller_actor(),
        request_event_ptr(&request_event),
        decline,
        RelayResolutionPolicy::ConfiguredRelays,
        PublishMode::EnqueueOnly,
        AckPolicy::NoWait,
    )
    .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
    .expect("target relays");

    let error = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(request, &FixtureSigner::new(SELLER_SECRET_KEY_HEX))
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
        .trades()
        .status(status_request("order-decision-conflict"))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::AgreedPendingRhi);
    assert_eq!(
        status
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(decision_event.id.as_str())
    );
}

#[cfg(any())]
#[tokio::test]
async fn order_revision_lifecycle_accepts_proposal_and_waits_for_rhi() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-lifecycle-agreement", 50);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 5_000))
        .await
        .expect("ingest request");

    let proposal = order_revision_proposal(
        "order-lifecycle-agreement",
        &request_event_id,
        &request_event_id,
    );
    let proposal_actor = seller_actor();
    let proposal_plan = sdk
        .trades()
        .prepare_revision_proposal(TradeRevisionProposalPrepareRequest::new(
            proposal_actor.clone(),
            request_event_ptr(&request_event),
            request_event_ptr(&request_event),
            proposal.clone(),
        ))
        .expect("prepare revision proposal");
    let proposal_receipt = sdk
        .trades()
        .enqueue_prepared_revision_proposal_with_explicit_signer(
            &proposal_actor,
            proposal_plan,
            RelayResolutionPolicy::try_explicit([RELAY], SdkRelayUrlPolicy::Public)
                .expect("proposal target relays"),
            PublishMode::EnqueueOnly,
            AckPolicy::NoWait,
            Some(
                SdkIdempotencyKey::new("order-lifecycle-revision-proposal")
                    .expect("proposal idempotency"),
            ),
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue revision proposal");
    assert_eq!(
        proposal_receipt.signed_event_id,
        proposal_receipt.expected_event_id
    );
    assert_eq!(
        outbox_operation_kind(&sdk, proposal_receipt.outbox_operation_id).await,
        TRADE_REVISION_PROPOSAL_OPERATION_KIND
    );
    let stored_proposal = store
        .get_event(proposal_receipt.signed_event_id.as_str())
        .await
        .expect("proposal event lookup")
        .expect("proposal event");
    assert_eq!(stored_proposal.kind, KIND_ORDER_REVISION_PROPOSAL);
    assert_eq!(
        stored_proposal.contract_id.as_deref(),
        Some("radroots.order.revision_proposal.v1")
    );

    let revision_decision = order_revision_decision(
        &proposal,
        &proposal_receipt.signed_event_id,
        RadrootsOrderRevisionOutcome::Accepted,
    );
    let revision_decision_actor = buyer_actor();
    let revision_decision_plan = sdk
        .trades()
        .prepare_revision_decision(TradeRevisionDecisionPrepareRequest::new(
            revision_decision_actor.clone(),
            request_event_ptr(&request_event),
            order_event_ptr(&proposal_receipt.signed_event_id),
            revision_decision,
        ))
        .expect("prepare revision decision");
    let revision_decision_receipt = sdk
        .trades()
        .enqueue_prepared_revision_decision_with_explicit_signer(
            &revision_decision_actor,
            revision_decision_plan,
            RelayResolutionPolicy::try_explicit([RELAY], SdkRelayUrlPolicy::Public)
                .expect("revision decision target relays"),
            PublishMode::EnqueueOnly,
            AckPolicy::NoWait,
            None,
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue revision decision");
    assert_eq!(
        outbox_operation_kind(&sdk, revision_decision_receipt.outbox_operation_id).await,
        TRADE_REVISION_DECISION_OPERATION_KIND
    );
    assert_eq!(
        store
            .get_event(revision_decision_receipt.signed_event_id.as_str())
            .await
            .expect("revision decision lookup")
            .expect("revision decision")
            .kind,
        KIND_ORDER_REVISION_DECISION
    );

    let status = sdk
        .trades()
        .status(status_request("order-lifecycle-agreement"))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::AgreedPendingRhi);
    assert_eq!(status.event_count, 3);
    assert_eq!(
        status
            .agreement_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(revision_decision_receipt.signed_event_id.as_str())
    );
    assert!(status.decision_event_id.is_none());
    assert!(status.cancellation_event_id.is_none());
    assert_eq!(status.pending_revision_event_id, None);
    assert_eq!(status.listing_addr, Some(listing_address()));
    assert_eq!(
        status.buyer_pubkey.as_ref().map(ToString::to_string),
        Some(BUYER_PUBLIC_KEY_HEX.to_owned())
    );
    assert_eq!(
        status.seller_pubkey.as_ref().map(ToString::to_string),
        Some(SELLER_PUBLIC_KEY_HEX.to_owned())
    );
    assert_eq!(status.economics, Some(revision_economics()));
    assert!(!status.lifecycle_terminal);
    assert_eq!(
        status.next_action,
        TradeStatusNextActionKind::AwaitRhiValidation
    );
    assert!(status.evidence.has_request);
    assert!(!status.evidence.has_decision);
    assert!(status.evidence.has_agreement);
    assert!(!status.evidence.has_pending_revision);
    assert!(!status.evidence.has_cancellation);
    assert!(!status.evidence.has_issues);
    assert!(!status.eligibility.can_decide);
    assert!(!status.eligibility.can_propose_revision);
    assert!(!status.eligibility.can_decide_revision);
    assert!(!status.eligibility.can_cancel);
    assert!(status.issues.is_empty());
}

#[cfg(any())]
#[tokio::test]
async fn order_revision_proposal_status_exposes_pending_and_blocks_follow_on_lifecycle() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-lifecycle-pending-revision", 55);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 5_500))
        .await
        .expect("ingest request");
    let proposal = order_revision_proposal(
        "order-lifecycle-pending-revision",
        &request_event_id,
        &request_event_id,
    );
    let proposal_receipt = sdk
        .trades()
        .enqueue_revision_proposal_with_explicit_signer(
            TradeRevisionProposalEnqueueRequest::new(
                seller_actor(),
                request_event_ptr(&request_event),
                request_event_ptr(&request_event),
                proposal,
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("proposal target relays"),
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue revision proposal");

    let status = sdk
        .trades()
        .status(status_request("order-lifecycle-pending-revision"))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::RevisionProposed);
    assert_eq!(status.event_count, 2);
    assert!(status.agreement_event_id.is_none());
    assert_eq!(
        status
            .pending_revision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(proposal_receipt.signed_event_id.as_str())
    );
    assert_eq!(
        status.last_event_id.as_ref().map(RadrootsEventId::as_str),
        Some(proposal_receipt.signed_event_id.as_str())
    );
    assert!(status.issues.is_empty());
    assert!(!status.eligibility.can_decide);
    assert!(!status.eligibility.can_propose_revision);
    assert!(status.eligibility.can_decide_revision);
    assert!(!status.eligibility.can_cancel);

    let decision_error = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(
            TradeDecisionEnqueueRequest::new(
                seller_actor(),
                request_event_ptr(&request_event),
                order_decision("order-lifecycle-pending-revision"),
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("decision target relays"),
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("pending revision blocks direct decision");
    assert!(matches!(
        decision_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let blocked_proposal = order_revision_proposal(
        "order-lifecycle-pending-revision",
        &request_event_id,
        &proposal_receipt.signed_event_id,
    );
    let proposal_error = sdk
        .trades()
        .enqueue_revision_proposal_with_explicit_signer(
            TradeRevisionProposalEnqueueRequest::new(
                seller_actor(),
                request_event_ptr(&request_event),
                order_event_ptr(&proposal_receipt.signed_event_id),
                blocked_proposal,
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("blocked proposal target relays"),
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("pending revision blocks new proposal");
    assert!(matches!(
        proposal_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        2
    );
}

#[cfg(any())]
#[tokio::test]
async fn order_declined_revision_finalizes_declined_negotiation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-lifecycle-declined-revision", 56);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 5_600))
        .await
        .expect("ingest request");
    let proposal = order_revision_proposal(
        "order-lifecycle-declined-revision",
        &request_event_id,
        &request_event_id,
    );
    let proposal_receipt = sdk
        .trades()
        .enqueue_revision_proposal_with_explicit_signer(
            TradeRevisionProposalEnqueueRequest::new(
                seller_actor(),
                request_event_ptr(&request_event),
                request_event_ptr(&request_event),
                proposal.clone(),
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("proposal target relays"),
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue revision proposal");
    let declined_revision = order_revision_decision(
        &proposal,
        &proposal_receipt.signed_event_id,
        RadrootsOrderRevisionOutcome::Declined {
            reason: "keep original order".to_owned(),
        },
    );
    let declined_revision_receipt = sdk
        .trades()
        .enqueue_revision_decision_with_explicit_signer(
            TradeRevisionDecisionEnqueueRequest::new(
                buyer_actor(),
                request_event_ptr(&request_event),
                order_event_ptr(&proposal_receipt.signed_event_id),
                declined_revision,
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("declined revision target relays"),
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue declined revision");

    let status = sdk
        .trades()
        .status(status_request("order-lifecycle-declined-revision"))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::Declined);
    assert_eq!(status.event_count, 3);
    assert!(status.agreement_event_id.is_none());
    assert_eq!(
        status
            .pending_revision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(proposal_receipt.signed_event_id.as_str())
    );
    assert_eq!(
        status.last_event_id.as_ref().map(RadrootsEventId::as_str),
        Some(declined_revision_receipt.signed_event_id.as_str())
    );
    assert!(status.lifecycle_terminal);
    assert_eq!(status.next_action, TradeStatusNextActionKind::Terminal);
    assert!(!status.eligibility.can_decide);
    assert!(!status.eligibility.can_propose_revision);
    assert!(!status.eligibility.can_decide_revision);
    assert!(!status.eligibility.can_cancel);

    let second_decision = order_revision_decision(
        &proposal,
        &proposal_receipt.signed_event_id,
        RadrootsOrderRevisionOutcome::Accepted,
    );
    let second_decision_error = sdk
        .trades()
        .enqueue_revision_decision_with_explicit_signer(
            TradeRevisionDecisionEnqueueRequest::new(
                buyer_actor(),
                request_event_ptr(&request_event),
                order_event_ptr(&proposal_receipt.signed_event_id),
                second_decision,
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("second decision target relays"),
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("second revision decision after decline");
    assert!(matches!(
        second_decision_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        3
    );
}

#[cfg(any())]
#[tokio::test]
async fn order_cancel_lifecycle_enqueue_updates_status() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-lifecycle-cancel", 60);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 6_000))
        .await
        .expect("ingest request");
    let cancellation_actor = buyer_actor();
    let cancellation_plan = sdk
        .trades()
        .prepare_cancellation(TradeCancellationPrepareRequest::new(
            cancellation_actor.clone(),
            request_event_ptr(&request_event),
            request_event_ptr(&request_event),
            order_cancellation("order-lifecycle-cancel"),
        ))
        .expect("prepare cancellation");
    let cancellation = sdk
        .trades()
        .enqueue_prepared_cancellation_with_explicit_signer(
            &cancellation_actor,
            cancellation_plan,
            RelayResolutionPolicy::try_explicit([RELAY], SdkRelayUrlPolicy::Public)
                .expect("cancellation target relays"),
            PublishMode::EnqueueOnly,
            AckPolicy::NoWait,
            Some(
                SdkIdempotencyKey::new("order-lifecycle-cancel").expect("cancellation idempotency"),
            ),
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue cancellation");

    assert_eq!(cancellation.root_event_id, request_event_id);
    assert_eq!(cancellation.previous_event_id, request_event_id);
    assert_eq!(
        outbox_operation_kind(&sdk, cancellation.outbox_operation_id).await,
        TRADE_CANCELLATION_OPERATION_KIND
    );
    assert_eq!(
        store
            .get_event(cancellation.signed_event_id.as_str())
            .await
            .expect("cancellation lookup")
            .expect("cancellation")
            .kind,
        KIND_ORDER_CANCELLATION
    );
    let replay = sdk
        .trades()
        .enqueue_cancellation_with_explicit_signer(
            TradeCancellationEnqueueRequest::new(
                buyer_actor(),
                request_event_ptr(&request_event),
                request_event_ptr(&request_event),
                order_cancellation("order-lifecycle-cancel"),
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("replay target relays")
            .try_with_idempotency_key("order-lifecycle-cancel")
            .expect("replay idempotency"),
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect("replay cancellation");
    assert_eq!(replay.state, SdkMutationState::AlreadyQueued);
    assert_eq!(replay.signed_event_id, cancellation.signed_event_id);
    assert_eq!(replay.outbox_event_id, cancellation.outbox_event_id);
    let status = sdk
        .trades()
        .status(status_request("order-lifecycle-cancel"))
        .await
        .expect("status");
    assert_eq!(status.status, TradeStatusKind::Cancelled);
    assert_eq!(
        status
            .cancellation_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(cancellation.signed_event_id.as_str())
    );
    assert!(status.lifecycle_terminal);
    assert_eq!(status.next_action, TradeStatusNextActionKind::Terminal);
    assert!(status.evidence.has_request);
    assert!(!status.evidence.has_decision);
    assert!(status.evidence.has_cancellation);
    assert!(!status.evidence.has_issues);
    assert!(!status.eligibility.can_cancel);
    assert!(status.issues.is_empty());
}

#[cfg(any())]
#[tokio::test]
async fn order_lifecycle_enqueue_rejects_invalid_state_before_mutation() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-lifecycle-invalid", 70);
    let request_event_id = RadrootsEventId::parse(request_event.id.as_str()).expect("request id");
    let missing = sdk
        .trades()
        .enqueue_revision_proposal_with_explicit_signer(
            TradeRevisionProposalEnqueueRequest::new(
                seller_actor(),
                request_event_ptr(&request_event),
                request_event_ptr(&request_event),
                order_revision_proposal(
                    "order-lifecycle-invalid",
                    &request_event_id,
                    &request_event_id,
                ),
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("missing target relays"),
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("missing local evidence");
    assert!(matches!(missing, RadrootsSdkError::InvalidRequest { .. }));
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );

    store
        .ingest_event(RadrootsEventIngest::new(request_event.clone(), 7_000))
        .await
        .expect("ingest request");
    let decision_receipt = sdk
        .trades()
        .enqueue_decision_with_explicit_signer(
            TradeDecisionEnqueueRequest::new(
                seller_actor(),
                request_event_ptr(&request_event),
                order_decision("order-lifecycle-invalid"),
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("decision target relays"),
            &FixtureSigner::new(SELLER_SECRET_KEY_HEX),
        )
        .await
        .expect("enqueue decision");
    let revision_without_proposal = order_revision_decision(
        &order_revision_proposal(
            "order-lifecycle-invalid",
            &request_event_id,
            &decision_receipt.signed_event_id,
        ),
        &decision_receipt.signed_event_id,
        RadrootsOrderRevisionOutcome::Accepted,
    );
    let revision_error = sdk
        .trades()
        .enqueue_revision_decision_with_explicit_signer(
            TradeRevisionDecisionEnqueueRequest::new(
                buyer_actor(),
                request_event_ptr(&request_event),
                order_event_ptr(&decision_receipt.signed_event_id),
                revision_without_proposal,
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("revision decision target relays"),
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("revision decision without proposal");
    assert!(matches!(
        revision_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));

    let cancellation_error = sdk
        .trades()
        .enqueue_cancellation_with_explicit_signer(
            TradeCancellationEnqueueRequest::new(
                buyer_actor(),
                request_event_ptr(&request_event),
                order_event_ptr(&decision_receipt.signed_event_id),
                order_cancellation("order-lifecycle-invalid"),
                RelayResolutionPolicy::ConfiguredRelays,
                PublishMode::EnqueueOnly,
                AckPolicy::NoWait,
            )
            .try_with_target_relays([RELAY], SdkRelayUrlPolicy::Public)
            .expect("cancellation target relays"),
            &FixtureSigner::new(BUYER_SECRET_KEY_HEX),
        )
        .await
        .expect_err("cancellation after accepted agreement");
    assert!(matches!(
        cancellation_error,
        RadrootsSdkError::InvalidRequest { .. }
    ));
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
async fn order_status_returns_not_found_for_missing_local_order() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request = status_request("order-1");

    assert_eq!(request.limit, TRADE_STATUS_DEFAULT_LIMIT);

    let receipt = sdk.trades().status(request).await.expect("status");

    assert!(!receipt.found);
    assert_eq!(receipt.order_id.as_str(), "order-1");
    assert_eq!(receipt.source, SdkTradeStatusSource::LocalEventStore);
    assert_eq!(receipt.event_count, 0);
    assert_eq!(receipt.limit_applied, TRADE_STATUS_DEFAULT_LIMIT);
    assert!(receipt.event_ids.is_empty());
    assert_eq!(receipt.status, TradeStatusKind::Missing);
    assert!(receipt.listing_addr.is_none());
    assert!(receipt.buyer_pubkey.is_none());
    assert!(receipt.seller_pubkey.is_none());
    assert!(receipt.economics.is_none());
    assert_eq!(receipt.next_action, TradeStatusNextActionKind::NoLocalOrder);
    assert_eq!(receipt.evidence.event_count, 0);
    assert_eq!(receipt.evidence.limit_applied, TRADE_STATUS_DEFAULT_LIMIT);
    assert!(!receipt.evidence.has_request);
    assert!(!receipt.evidence.has_issues);
    assert!(!receipt.eligibility.can_decide);
    assert!(!receipt.eligibility.can_cancel);
    assert!(!receipt.eligibility.can_propose_revision);
    assert!(!receipt.eligibility.can_decide_revision);
    assert!(receipt.issues.is_empty());
}

#[tokio::test]
async fn order_status_rejects_invalid_limits_before_querying() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;

    let zero = sdk
        .trades()
        .status(status_request("order-1").with_limit(0))
        .await
        .expect_err("zero limit");
    let too_large = sdk
        .trades()
        .status(status_request("order-1").with_limit(TRADE_STATUS_MAX_LIMIT + 1))
        .await
        .expect_err("too large");

    assert!(matches!(
        zero,
        RadrootsSdkError::TradeStatusLimitInvalid {
            limit: 0,
            min: 1,
            max: TRADE_STATUS_MAX_LIMIT
        }
    ));
    assert!(matches!(
        too_large,
        RadrootsSdkError::TradeStatusLimitInvalid {
            limit,
            min: 1,
            max: TRADE_STATUS_MAX_LIMIT
        } if limit == TRADE_STATUS_MAX_LIMIT + 1
    ));
}

#[test]
fn order_status_parse_rejects_invalid_order_ids() {
    let error = TradeStatusRequest::parse("bad order id").expect_err("invalid order id");

    assert!(matches!(error, RadrootsSdkError::InvalidTradeId { .. }));
}

#[tokio::test]
async fn order_status_contract_dtos_serialize_deterministically() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request = status_request("order-1").with_limit(25);
    let request_json = serde_json::to_value(&request).expect("request json");
    assert_struct_serialize_error_paths(&request, 2);

    assert_eq!(
        request_json,
        serde_json::json!({
            "locator": {
                "trade_id": "order-1",
                "root_event_id": null,
                "listing_addr": null,
                "buyer_pubkey": null,
                "seller_pubkey": null
            },
            "limit": 25
        })
    );

    let receipt = sdk.trades().status(request).await.expect("status");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(receipt_json["source"], "local_event_store");
    assert_eq!(receipt_json["status"], "missing");
    assert_eq!(receipt_json["locator"], request_json["locator"]);
    assert_eq!(receipt_json["order_id"], "order-1");
    assert_eq!(receipt_json["root_event_id"], serde_json::Value::Null);
    assert_eq!(receipt_json["ambiguity_candidates"], serde_json::json!([]));
    assert_eq!(receipt_json["listing_addr"], serde_json::Value::Null);
    assert_eq!(receipt_json["buyer_pubkey"], serde_json::Value::Null);
    assert_eq!(receipt_json["seller_pubkey"], serde_json::Value::Null);
    assert_eq!(
        receipt_json["rhi_receipt_event_id"],
        serde_json::Value::Null
    );
    assert_eq!(receipt_json["economics"], serde_json::Value::Null);
    assert_eq!(receipt_json["next_action"], "no_local_order");
    assert_eq!(receipt_json["evidence"]["event_count"], 0);
    assert_eq!(receipt_json["evidence"]["limit_applied"], 25);
    assert_eq!(receipt_json["evidence"]["has_request"], false);
    assert_eq!(receipt_json["eligibility"]["can_decide"], false);
    assert_eq!(receipt_json["eligibility"]["can_cancel"], false);
    assert_eq!(receipt_json["eligibility"]["can_propose_revision"], false);
    assert_eq!(receipt_json["eligibility"]["can_decide_revision"], false);

    let issue = SdkTradeStatusIssue {
        kind: SdkTradeStatusIssueKind::DecisionPayloadInvalid,
        event_ids: vec![deterministic_event_id("issue-event")],
    };
    assert_eq!(issue.code(), "decision_payload_invalid");
    assert_struct_serialize_error_paths(&issue, 3);
    assert_eq!(
        serde_json::to_value(issue).expect("issue json"),
        serde_json::json!({
            "code": "decision_payload_invalid",
            "kind": "decision_payload_invalid",
            "event_ids": [deterministic_event_id("issue-event")]
        })
    );
}

#[test]
fn order_status_issue_mapping_preserves_kind_codes_and_event_ids() {
    macro_rules! single_issue {
        ($variant:ident, $kind:ident, $code:literal) => {{
            let event_id = deterministic_event_id($code);
            (
                RadrootsOrderIssue::$variant {
                    event_id: event_id.clone(),
                },
                SdkTradeStatusIssueKind::$kind,
                $code,
                vec![event_id],
            )
        }};
    }

    macro_rules! multi_issue {
        ($variant:ident, $kind:ident, $code:literal) => {{
            let event_ids = vec![
                deterministic_event_id(concat!($code, "-a")),
                deterministic_event_id(concat!($code, "-b")),
            ];
            (
                RadrootsOrderIssue::$variant {
                    event_ids: event_ids.clone(),
                },
                SdkTradeStatusIssueKind::$kind,
                $code,
                event_ids,
            )
        }};
    }

    let cases = vec![
        (
            RadrootsOrderIssue::MissingRequest,
            SdkTradeStatusIssueKind::MissingRequest,
            "missing_request",
            Vec::new(),
        ),
        multi_issue!(MultipleRequests, MultipleRequests, "multiple_requests"),
        single_issue!(
            RequestPayloadInvalid,
            RequestPayloadInvalid,
            "request_payload_invalid"
        ),
        single_issue!(
            RequestOrderIdMismatch,
            RequestOrderIdMismatch,
            "request_order_id_mismatch"
        ),
        single_issue!(
            RequestAuthorMismatch,
            RequestAuthorMismatch,
            "request_author_mismatch"
        ),
        single_issue!(
            RequestListingAddressInvalid,
            RequestListingAddressInvalid,
            "request_listing_address_invalid"
        ),
        single_issue!(
            RequestSellerListingMismatch,
            RequestSellerListingMismatch,
            "request_seller_listing_mismatch"
        ),
        single_issue!(
            DecisionPayloadInvalid,
            DecisionPayloadInvalid,
            "decision_payload_invalid"
        ),
        single_issue!(
            DecisionOrderIdMismatch,
            DecisionOrderIdMismatch,
            "decision_order_id_mismatch"
        ),
        single_issue!(
            DecisionAuthorMismatch,
            DecisionAuthorMismatch,
            "decision_author_mismatch"
        ),
        single_issue!(
            DecisionCounterpartyMismatch,
            DecisionCounterpartyMismatch,
            "decision_counterparty_mismatch"
        ),
        single_issue!(
            DecisionBuyerMismatch,
            DecisionBuyerMismatch,
            "decision_buyer_mismatch"
        ),
        single_issue!(
            DecisionSellerMismatch,
            DecisionSellerMismatch,
            "decision_seller_mismatch"
        ),
        single_issue!(
            DecisionListingAddressInvalid,
            DecisionListingAddressInvalid,
            "decision_listing_address_invalid"
        ),
        single_issue!(
            DecisionListingMismatch,
            DecisionListingMismatch,
            "decision_listing_mismatch"
        ),
        single_issue!(
            DecisionRootMismatch,
            DecisionRootMismatch,
            "decision_root_mismatch"
        ),
        single_issue!(
            DecisionPreviousMismatch,
            DecisionPreviousMismatch,
            "decision_previous_mismatch"
        ),
        single_issue!(
            DecisionMissingInventoryCommitments,
            DecisionMissingInventoryCommitments,
            "decision_missing_inventory_commitments"
        ),
        single_issue!(
            DecisionInventoryCommitmentMismatch,
            DecisionInventoryCommitmentMismatch,
            "decision_inventory_commitment_mismatch"
        ),
        single_issue!(
            DecisionMissingReason,
            DecisionMissingReason,
            "decision_missing_reason"
        ),
        multi_issue!(
            ConflictingDecisions,
            ConflictingDecisions,
            "conflicting_decisions"
        ),
        single_issue!(
            RevisionProposalPayloadInvalid,
            RevisionProposalPayloadInvalid,
            "revision_proposal_payload_invalid"
        ),
        single_issue!(
            RevisionProposalOrderIdMismatch,
            RevisionProposalOrderIdMismatch,
            "revision_proposal_order_id_mismatch"
        ),
        single_issue!(
            RevisionProposalAuthorMismatch,
            RevisionProposalAuthorMismatch,
            "revision_proposal_author_mismatch"
        ),
        single_issue!(
            RevisionProposalCounterpartyMismatch,
            RevisionProposalCounterpartyMismatch,
            "revision_proposal_counterparty_mismatch"
        ),
        single_issue!(
            RevisionProposalBuyerMismatch,
            RevisionProposalBuyerMismatch,
            "revision_proposal_buyer_mismatch"
        ),
        single_issue!(
            RevisionProposalSellerMismatch,
            RevisionProposalSellerMismatch,
            "revision_proposal_seller_mismatch"
        ),
        single_issue!(
            RevisionProposalListingAddressInvalid,
            RevisionProposalListingAddressInvalid,
            "revision_proposal_listing_address_invalid"
        ),
        single_issue!(
            RevisionProposalListingMismatch,
            RevisionProposalListingMismatch,
            "revision_proposal_listing_mismatch"
        ),
        single_issue!(
            RevisionProposalRootMismatch,
            RevisionProposalRootMismatch,
            "revision_proposal_root_mismatch"
        ),
        single_issue!(
            RevisionProposalPreviousMismatch,
            RevisionProposalPreviousMismatch,
            "revision_proposal_previous_mismatch"
        ),
        single_issue!(
            RevisionDecisionWithoutProposal,
            RevisionDecisionWithoutProposal,
            "revision_decision_without_proposal"
        ),
        single_issue!(
            RevisionDecisionPayloadInvalid,
            RevisionDecisionPayloadInvalid,
            "revision_decision_payload_invalid"
        ),
        single_issue!(
            RevisionDecisionOrderIdMismatch,
            RevisionDecisionOrderIdMismatch,
            "revision_decision_order_id_mismatch"
        ),
        single_issue!(
            RevisionDecisionAuthorMismatch,
            RevisionDecisionAuthorMismatch,
            "revision_decision_author_mismatch"
        ),
        single_issue!(
            RevisionDecisionCounterpartyMismatch,
            RevisionDecisionCounterpartyMismatch,
            "revision_decision_counterparty_mismatch"
        ),
        single_issue!(
            RevisionDecisionBuyerMismatch,
            RevisionDecisionBuyerMismatch,
            "revision_decision_buyer_mismatch"
        ),
        single_issue!(
            RevisionDecisionSellerMismatch,
            RevisionDecisionSellerMismatch,
            "revision_decision_seller_mismatch"
        ),
        single_issue!(
            RevisionDecisionListingAddressInvalid,
            RevisionDecisionListingAddressInvalid,
            "revision_decision_listing_address_invalid"
        ),
        single_issue!(
            RevisionDecisionListingMismatch,
            RevisionDecisionListingMismatch,
            "revision_decision_listing_mismatch"
        ),
        single_issue!(
            RevisionDecisionRootMismatch,
            RevisionDecisionRootMismatch,
            "revision_decision_root_mismatch"
        ),
        single_issue!(
            RevisionDecisionPreviousMismatch,
            RevisionDecisionPreviousMismatch,
            "revision_decision_previous_mismatch"
        ),
        single_issue!(
            RevisionDecisionRevisionIdMismatch,
            RevisionDecisionRevisionIdMismatch,
            "revision_decision_revision_id_mismatch"
        ),
        single_issue!(
            CancellationWithoutCancellableOrder,
            CancellationWithoutCancellableOrder,
            "cancellation_without_cancellable_order"
        ),
        single_issue!(
            CancellationPayloadInvalid,
            CancellationPayloadInvalid,
            "cancellation_payload_invalid"
        ),
        single_issue!(
            CancellationOrderIdMismatch,
            CancellationOrderIdMismatch,
            "cancellation_order_id_mismatch"
        ),
        single_issue!(
            CancellationAuthorMismatch,
            CancellationAuthorMismatch,
            "cancellation_author_mismatch"
        ),
        single_issue!(
            CancellationCounterpartyMismatch,
            CancellationCounterpartyMismatch,
            "cancellation_counterparty_mismatch"
        ),
        single_issue!(
            CancellationBuyerMismatch,
            CancellationBuyerMismatch,
            "cancellation_buyer_mismatch"
        ),
        single_issue!(
            CancellationSellerMismatch,
            CancellationSellerMismatch,
            "cancellation_seller_mismatch"
        ),
        single_issue!(
            CancellationListingAddressInvalid,
            CancellationListingAddressInvalid,
            "cancellation_listing_address_invalid"
        ),
        single_issue!(
            CancellationListingMismatch,
            CancellationListingMismatch,
            "cancellation_listing_mismatch"
        ),
        single_issue!(
            CancellationRootMismatch,
            CancellationRootMismatch,
            "cancellation_root_mismatch"
        ),
        single_issue!(
            CancellationPreviousMismatch,
            CancellationPreviousMismatch,
            "cancellation_previous_mismatch"
        ),
        multi_issue!(ForkedLifecycle, ForkedLifecycle, "forked_lifecycle"),
        single_issue!(
            ValidationReceiptWithoutPendingAgreement,
            ValidationReceiptWithoutPendingAgreement,
            "validation_receipt_without_pending_agreement"
        ),
        single_issue!(
            ValidationReceiptOrderIdMismatch,
            ValidationReceiptOrderIdMismatch,
            "validation_receipt_order_id_mismatch"
        ),
        single_issue!(
            ValidationReceiptTypeMismatch,
            ValidationReceiptTypeMismatch,
            "validation_receipt_type_mismatch"
        ),
        single_issue!(
            ValidationReceiptRootMismatch,
            ValidationReceiptRootMismatch,
            "validation_receipt_root_mismatch"
        ),
        single_issue!(
            ValidationReceiptTargetMismatch,
            ValidationReceiptTargetMismatch,
            "validation_receipt_target_mismatch"
        ),
        single_issue!(
            ValidationReceiptListingMismatch,
            ValidationReceiptListingMismatch,
            "validation_receipt_listing_mismatch"
        ),
        multi_issue!(
            ConflictingValidationReceipts,
            ConflictingValidationReceipts,
            "conflicting_validation_receipts"
        ),
        {
            let event_id = deterministic_event_id("deterministic_validation_failure");
            (
                RadrootsOrderIssue::DeterministicValidationFailure {
                    event_id: event_id.clone(),
                    reason: "fixture validation failed".to_owned(),
                },
                SdkTradeStatusIssueKind::DeterministicValidationFailure,
                "deterministic_validation_failure",
                vec![event_id],
            )
        },
        {
            let expected_event_id = deterministic_event_id("stale_listing_event_expected");
            let current_event_id = deterministic_event_id("stale_listing_event_current");
            (
                RadrootsOrderIssue::StaleListingEvent {
                    expected_event_id: expected_event_id.clone(),
                    current_event_id: current_event_id.clone(),
                },
                SdkTradeStatusIssueKind::StaleListingEvent,
                "stale_listing_event",
                vec![expected_event_id, current_event_id],
            )
        },
    ];

    for (issue, expected_kind, expected_code, expected_event_ids) in cases {
        let sdk_issue = SdkTradeStatusIssue::from(issue);

        assert_eq!(sdk_issue.kind, expected_kind);
        assert_eq!(sdk_issue.code(), expected_code);
        assert_eq!(sdk_issue.event_ids, expected_event_ids);
    }
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
        .trades()
        .status(status_request("order-1").with_limit(1_000))
        .await
        .expect("status");

    assert!(receipt.found);
    assert_eq!(receipt.order_id.as_str(), "order-1");
    assert_eq!(receipt.source, SdkTradeStatusSource::LocalEventStore);
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
    assert_eq!(receipt.status, TradeStatusKind::AgreedPendingRhi);
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
    assert_eq!(receipt.listing_addr, Some(listing_address()));
    assert_eq!(
        receipt.buyer_pubkey.as_ref().map(ToString::to_string),
        Some(BUYER_PUBLIC_KEY_HEX.to_owned())
    );
    assert_eq!(
        receipt.seller_pubkey.as_ref().map(ToString::to_string),
        Some(SELLER_PUBLIC_KEY_HEX.to_owned())
    );
    assert_eq!(receipt.economics, Some(economics()));
    assert!(receipt.issues.is_empty());
    assert!(!receipt.lifecycle_terminal);
    assert_eq!(
        receipt.next_action,
        TradeStatusNextActionKind::AwaitRhiValidation
    );
    assert_eq!(receipt.evidence.event_count, 2);
    assert!(receipt.evidence.has_request);
    assert!(receipt.evidence.has_decision);
    assert!(receipt.evidence.has_agreement);
    assert!(!receipt.evidence.has_pending_revision);
    assert!(!receipt.evidence.has_issues);
    assert!(!receipt.eligibility.can_decide);
    assert!(!receipt.eligibility.can_propose_revision);
    assert!(!receipt.eligibility.can_decide_revision);
    assert!(!receipt.eligibility.can_cancel);
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
        .trades()
        .status(status_request("order-1").with_limit(1))
        .await
        .expect("status");

    assert!(receipt.found);
    assert_eq!(receipt.status, TradeStatusKind::Requested);
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
async fn order_status_reports_root_ambiguity_for_reused_trade_ids() {
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
        .trades()
        .status(status_request("order-1"))
        .await
        .expect("status");

    assert!(!receipt.found);
    assert_eq!(receipt.status, TradeStatusKind::Ambiguous);
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
    assert!(receipt.issues.is_empty());
    let candidate_roots = receipt
        .ambiguity_candidates
        .iter()
        .map(|candidate| {
            candidate
                .locator
                .root_event_id
                .as_ref()
                .map(RadrootsEventId::as_str)
                .expect("root event id")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        candidate_roots,
        vec![
            first_request_event.id.as_str(),
            second_request_event.id.as_str()
        ]
    );
}

#[cfg(feature = "signer-adapters")]
#[tokio::test]
async fn trade_product_mutation_returns_structured_ambiguity() {
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

    let error = sdk
        .trades()
        .seller()
        .accept_trade(TradeAcceptRequest::new(
            seller_actor(),
            status_request("order-1").locator,
            vec![RadrootsOrderInventoryCommitment {
                bin_id: "bin-1".parse().expect("bin id"),
                bin_count: 1,
            }],
            explicit_trade_relays(),
            PublishMode::EnqueueOnly,
            AckPolicy::NoWait,
        ))
        .await
        .expect_err("ambiguous product mutation");

    let RadrootsSdkError::TradeAmbiguous {
        operation,
        locator,
        candidates,
    } = &error
    else {
        panic!("expected structured trade ambiguity error");
    };
    assert_eq!(operation, "trade.accept");
    assert_eq!(locator.order_id().as_str(), "order-1");
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| {
                candidate
                    .root_event_id
                    .as_ref()
                    .map(RadrootsEventId::as_str)
                    .expect("root event id")
            })
            .collect::<Vec<_>>(),
        vec![
            first_request_event.id.as_str(),
            second_request_event.id.as_str()
        ]
    );
    assert_eq!(
        error.recovery_actions(),
        vec![RadrootsSdkRecoveryAction::SelectTradeRoot]
    );
    let detail = error.detail_json();
    assert_eq!(detail["code"], "trade_ambiguous");
    assert_eq!(detail["detail"]["operation"], "trade.accept");
    assert_eq!(
        detail["detail"]["candidates"]
            .as_array()
            .expect("candidates")
            .len(),
        2
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
    sqlx::query("UPDATE nostr_events SET tags_json = '[' WHERE event_id = ?")
        .bind(request_event.id.as_str())
        .execute(store.pool())
        .await
        .expect("corrupt tags");

    let error = sdk
        .trades()
        .status(status_request("order-1"))
        .await
        .expect_err("projection error");
    let message = error.to_string();

    assert!(matches!(error, RadrootsSdkError::Projection { .. }));
    assert!(message.contains("contains invalid tags_json"));
    assert!(!message.contains(raw_event_json.as_str()));
    assert!(!message.contains(request_event.sig.as_str()));
    assert!(!message.contains("\"tags\""));
    assert!(!message.contains("\"content\""));
}

#[tokio::test]
#[ignore = "measures the 100k local-event MVP status target"]
async fn local_status_meets_mvp_scale_target() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let background_non_trade_events = PERF_TOTAL_LOCAL_EVENTS - PERF_TRADE_RELEVANT_EVENTS;
    let background_trade_events = PERF_TRADE_RELEVANT_EVENTS - PERF_ACTIVE_TRADES as i64;
    insert_perf_non_trade_events(&store, 10_000_000, background_non_trade_events).await;
    insert_perf_trade_background_events(&store, 20_000_000, background_trade_events).await;

    let mut active_order_ids = Vec::with_capacity(PERF_ACTIVE_TRADES);
    for index in 0..PERF_ACTIVE_TRADES {
        let order_id = format!("perf-active-{index:04}");
        let event = signed_order_request_event(&order_id, 20_000 + index as u32);
        store
            .ingest_event(RadrootsEventIngest::new(
                event,
                1_700_100_000_000 + index as i64,
            ))
            .await
            .expect("active trade ingest");
        active_order_ids.push(order_id);
    }

    let summary = store.status_summary().await.expect("status summary");
    assert_eq!(summary.total_events, PERF_TOTAL_LOCAL_EVENTS);

    let mut durations = Vec::with_capacity(active_order_ids.len());
    for order_id in &active_order_ids {
        let started = Instant::now();
        let status = sdk
            .trades()
            .status(status_request(order_id))
            .await
            .expect("status");
        durations.push(started.elapsed());
        assert_eq!(status.status, TradeStatusKind::Requested);
        assert_eq!(status.event_count, 1);
    }

    durations.sort_unstable();
    let p95 = durations[(durations.len() * 95 / 100).saturating_sub(1)];
    println!(
        "local status p95 {}us for {PERF_TOTAL_LOCAL_EVENTS} local events, {PERF_TRADE_RELEVANT_EVENTS} trade-relevant events, and {PERF_ACTIVE_TRADES} active trades",
        p95.as_micros()
    );
    assert!(
        p95 <= PERF_STATUS_P95_TARGET,
        "local status p95 {}us exceeded target {}us for {PERF_TOTAL_LOCAL_EVENTS} local events, {PERF_TRADE_RELEVANT_EVENTS} trade-relevant events, and {PERF_ACTIVE_TRADES} active trades",
        p95.as_micros(),
        PERF_STATUS_P95_TARGET.as_micros()
    );
}
