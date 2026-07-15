#![cfg(feature = "runtime")]

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
use std::path::Path;
#[cfg(feature = "transport-nostr-runtime")]
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(feature = "transport-nostr-runtime")]
use futures::future::BoxFuture;
#[cfg(feature = "transport-nostr-runtime")]
use nostr::JsonUtil;
#[cfg(feature = "signer-adapters")]
use radroots_authority::RadrootsActorContext;
#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
use radroots_authority::RadrootsLocalEventSigner;
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
};
#[cfg(feature = "signer-adapters")]
use radroots_event::contract::RadrootsActorRole;
use radroots_event::ids::RadrootsPublicKey;
use radroots_event::wire::RadrootsNip01EventWireParts;
use radroots_event::{
    RadrootsEventEnvelope, RadrootsEventPtr,
    draft::RadrootsSignedEvent,
    ids::{RadrootsEventId, RadrootsListingAddress, RadrootsOrderId},
    kinds::{KIND_LISTING, KIND_ORDER_DECISION, KIND_ORDER_REQUEST, KIND_POST},
    order::{
        RadrootsOrderDecision, RadrootsOrderDecisionOutcome, RadrootsOrderEconomicItem,
        RadrootsOrderEconomicLine, RadrootsOrderEconomics, RadrootsOrderInventoryCommitment,
        RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
    },
};
use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_event,
};
#[cfg(feature = "signer-adapters")]
use radroots_outbox::RadrootsOutbox;
#[cfg(any(feature = "signer-adapters", feature = "transport-nostr-runtime"))]
use radroots_sdk::{NostrProfile, NostrRelayUrlPolicy, TransportProfile};
#[cfg(feature = "signer-adapters")]
use radroots_sdk::{
    PrivacyPreflightConfirmation, PrivacyPreflightStatus, ProductSensitivityField, PublishMode,
    RadrootsSdkRecoveryAction, SatisfactionPolicy, SdkMutationState, TargetPolicy, TargetSet,
};
use radroots_sdk::{
    RadrootsClient, RadrootsSdkError, RadrootsSdkTimestamp, RadrootsTradeValidationTrustPolicy,
    RadrootsTradeValidationTrustState, SdkTradeStatusIssue, SdkTradeStatusIssueKind,
    SdkTradeStatusSource, TRADE_STATUS_DEFAULT_LIMIT, TRADE_STATUS_MAX_LIMIT,
    TRADE_STATUS_WATCH_MAX_CAPACITY, TradeEvidenceIngestRequest, TradeRequestEvidenceIngestRequest,
    TradeStatusKind, TradeStatusNextActionKind, TradeStatusRequest, TradeStatusWatchCancelState,
    TradeStatusWatchRequest,
};
#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
use radroots_sdk::{RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider};
#[cfg(feature = "signer-adapters")]
use radroots_sdk::{
    TRADE_SUBMIT_OPERATION_KIND, TradeAcceptRequest, TradeCancelRequest, TradeDeclineRequest,
    TradeEvidenceMode, TradeMutationOutcome, TradeProposeRequest,
};
#[cfg(feature = "transport-nostr-runtime")]
use radroots_sdk::{
    TradeEvidenceQueryBranchKind, TradeResyncNostrRelayOutcomeKind,
    TradeResyncNostrRelayTransportOutcomeKind, TradeResyncRequest, TradeSellerInboxRequest,
    TradeValidationReceiptInspectRequest, TradeValidationReceiptListRequest,
    TradeValidationReceiptVerifyRequest,
};
#[cfg(feature = "transport-nostr-runtime")]
use radroots_trade::identity::RadrootsTradeLocator;
use radroots_trade::order::RadrootsOrderIssue;
use radroots_trade::validation_receipt::{
    RadrootsTradeValidationReceipt, RadrootsValidationReceiptProof,
    RadrootsValidationReceiptProofSystem, RadrootsValidationReceiptResult,
    RadrootsValidationReceiptStatement, RadrootsValidationReceiptType, RadrootsValidatorSetV1,
    validation_receipt_event_build, validation_receipt_public_values_hash_hex,
    validator_set_address_from_str,
};
#[cfg(feature = "transport-nostr-runtime")]
use radroots_transport_nostr::{
    RadrootsMockRelayFetchAdapter, RadrootsRelayFetchAdapter, RadrootsRelayFetchItem,
    RadrootsRelayFetchRequest, RadrootsRelayTransportError,
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

fn signed_event_from_envelope(event: RadrootsEventEnvelope) -> RadrootsSignedEvent {
    let wire = event.to_nip01_wire();
    let raw_json = serde_json::to_string(&wire).expect("raw event json");
    RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
}
#[cfg(feature = "transport-nostr-runtime")]
const RELAY_B: &str = "wss://relay-b.radroots.test";
const PERF_TOTAL_LOCAL_EVENTS: i64 = 100_000;
const PERF_TRADE_RELEVANT_EVENTS: i64 = 25_000;
const PERF_ACTIVE_TRADES: usize = 1_000;
const PERF_STATUS_P95_TARGET: Duration = Duration::from_millis(50);
const STATUS_NOISE_NON_TRADE_EVENTS: i64 = 128;
const STATUS_NOISE_TRADE_BACKGROUND_EVENTS: i64 = 64;

#[derive(Clone, Copy)]
enum FailingSerializeFailure {
    Start,
    Field(usize),
    End,
}

struct FailingStructSerializer {
    failure: FailingSerializeFailure,
}

#[cfg(feature = "transport-nostr-runtime")]
#[derive(Clone, Default)]
struct CapturingRelayFetchAdapter {
    filters_json: Arc<Mutex<Vec<String>>>,
}

#[cfg(feature = "transport-nostr-runtime")]
impl CapturingRelayFetchAdapter {
    fn filters_json(&self) -> Vec<String> {
        self.filters_json
            .lock()
            .expect("captured filters lock")
            .clone()
    }
}

#[cfg(feature = "transport-nostr-runtime")]
impl RadrootsRelayFetchAdapter for CapturingRelayFetchAdapter {
    fn fetch<'a>(
        &'a self,
        request: RadrootsRelayFetchRequest,
    ) -> BoxFuture<'a, Result<Vec<RadrootsRelayFetchItem>, RadrootsRelayTransportError>> {
        Box::pin(async move {
            let filters = request
                .filters()
                .iter()
                .map(JsonUtil::as_json)
                .collect::<Vec<_>>();
            *self.filters_json.lock().expect("captured filters lock") = filters;
            Ok(vec![relay_eose(RELAY)])
        })
    }
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

async fn directory_sdk_and_store() -> (tempfile::TempDir, RadrootsClient, RadrootsEventStore) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let sdk = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .build()
        .await
        .expect("sdk");
    let store = RadrootsEventStore::open_file(&sdk.storage_paths().expect("paths").runtime_path)
        .await
        .expect("event store");
    (tempdir, sdk, store)
}

#[cfg(feature = "transport-nostr-runtime")]
async fn directory_sdk_and_store_with_relays(
    relays: &[&str],
) -> (tempfile::TempDir, RadrootsClient, RadrootsEventStore) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let mut builder = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000));
    if !relays.is_empty() {
        builder = builder.transport_profile(TransportProfile::nostr(
            NostrProfile::new(relays.iter().copied(), NostrRelayUrlPolicy::Public)
                .expect("Nostr profile"),
        ));
    }
    let sdk = builder.build().await.expect("sdk");
    let store = RadrootsEventStore::open_file(&sdk.storage_paths().expect("paths").runtime_path)
        .await
        .expect("event store");
    (tempdir, sdk, store)
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
async fn directory_sdk_with_signer(storage_root: &Path, secret_key_hex: &str) -> RadrootsClient {
    directory_sdk_with_signer_and_relays(storage_root, secret_key_hex, &[]).await
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
async fn directory_sdk_with_signer_and_relays(
    storage_root: &Path,
    secret_key_hex: &str,
    relays: &[&str],
) -> RadrootsClient {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    let signer_keys = RadrootsNostrKeys::new(secret_key);
    let mut builder = RadrootsClient::builder()
        .directory_storage(storage_root)
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::from_event_signer(
                RadrootsLocalEventSigner::new(signer_keys).expect("local event signer"),
            )
            .expect("local signer"),
        ));
    if !relays.is_empty() {
        builder = builder.transport_profile(TransportProfile::nostr(
            NostrProfile::new(relays.iter().copied(), NostrRelayUrlPolicy::Public)
                .expect("Nostr profile"),
        ));
    }
    builder.build().await.expect("sdk")
}

fn order_id(raw: &str) -> RadrootsOrderId {
    RadrootsOrderId::parse(raw).expect("order id")
}

fn status_request(raw: &str) -> TradeStatusRequest {
    TradeStatusRequest::parse(raw).expect("order status request")
}

#[cfg(feature = "signer-adapters")]
fn buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(BUYER_PUBLIC_KEY_HEX, [RadrootsActorRole::Buyer]).expect("actor")
}

#[cfg(feature = "signer-adapters")]
fn seller_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(SELLER_PUBLIC_KEY_HEX, [RadrootsActorRole::Seller]).expect("actor")
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

#[cfg(feature = "signer-adapters")]
fn explicit_trade_relays() -> TargetPolicy {
    TargetPolicy::explicit(
        TargetSet::nostr_relays([RELAY], NostrRelayUrlPolicy::Public).expect("transport targets"),
    )
}

#[cfg(feature = "signer-adapters")]
fn public_note_confirmation() -> PrivacyPreflightConfirmation {
    PrivacyPreflightConfirmation::new().confirm(ProductSensitivityField::PublicButSensitiveNotes)
}

#[cfg(feature = "signer-adapters")]
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

fn validator_set_id() -> &'static str {
    "018f3d99-7d35-7c0c-8a0f-7f3b645abcde"
}

fn validator_set_event_id() -> RadrootsEventId {
    deterministic_event_id("validator-set-event")
}

fn validator_set_addr_raw(author_pubkey: &str) -> String {
    format!("30381:{author_pubkey}:{}", validator_set_id())
}

fn validator_set_policy_for_validator_pubkey(
    validator_pubkey: &str,
) -> RadrootsTradeValidationTrustPolicy {
    let validator_set = RadrootsValidatorSetV1 {
        set_id: validator_set_id().to_owned(),
        validator_pubkey: RadrootsPublicKey::parse(validator_pubkey).expect("validator pubkey"),
        threshold: 1,
        valid_from: 1_700_000_000,
        valid_until: 1_800_000_000,
        protocol_contract_hash: hash32('7'),
        operator_name: "Radroots validation operator".to_owned(),
        operator_contact: None,
    };
    RadrootsTradeValidationTrustPolicy::production().with_validator_set(
        validator_set,
        validator_set_address_from_str(validator_set_addr_raw(validator_pubkey))
            .expect("validator set address"),
        validator_set_event_id().into_string(),
    )
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

#[cfg(feature = "signer-adapters")]
fn trade_propose_request(
    raw_order_id: &str,
    publish_mode: PublishMode,
    satisfaction_policy: SatisfactionPolicy,
) -> TradeProposeRequest {
    let order = order_request(raw_order_id);
    TradeProposeRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order,
        explicit_trade_relays(),
        publish_mode,
        satisfaction_policy,
    )
}

#[cfg(all(
    feature = "signer-adapters",
    feature = "local-signer",
    feature = "transport-nostr-runtime"
))]
#[tokio::test]
async fn trade_product_clients_propose_inbox_accept_status_and_resync() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer_and_relays(
        storage_root.as_path(),
        BUYER_SECRET_KEY_HEX,
        &[RELAY],
    )
    .await;
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                trade_propose_request(
                    "trade-product-facade-flow",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000204")
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

    let seller_sdk = directory_sdk_with_signer_and_relays(
        storage_root.as_path(),
        SELLER_SECRET_KEY_HEX,
        &[RELAY],
    )
    .await;
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
                    SatisfactionPolicy::NoWait,
                    TradeEvidenceMode::LocalOnly,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000205")
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
        .status(TradeStatusRequest::new(propose_receipt.locator.clone()))
        .await
        .expect("facade status");
    assert_eq!(status.status, TradeStatusKind::AgreedPendingValidation);
    assert_eq!(
        status
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(accept_receipt.signed_event_id.as_str())
    );

    let resync_adapter = RadrootsMockRelayFetchAdapter::new(vec![relay_eose(RELAY)]);
    let resync = seller_sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(propose_receipt.locator),
            &resync_adapter,
        )
        .await
        .expect("facade resync");
    assert_eq!(resync.nostr_relay_urls, vec![RELAY.to_owned()]);
    assert_eq!(resync.evidence.eose_count, 1);
    assert_eq!(
        resync.status.status,
        TradeStatusKind::AgreedPendingValidation
    );
    assert_eq!(
        resync.status.last_event_id,
        Some(accept_receipt.signed_event_id)
    );
}

#[cfg(all(
    feature = "signer-adapters",
    feature = "local-signer",
    feature = "transport-nostr-runtime"
))]
#[tokio::test]
async fn trade_product_clients_resync_committed_after_rhi_validation_receipt() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let buyer_storage_root = tempdir.path().join("buyer-sdk");
    let seller_storage_root = tempdir.path().join("seller-sdk");
    let buyer_sdk = directory_sdk_with_signer_and_relays(
        buyer_storage_root.as_path(),
        BUYER_SECRET_KEY_HEX,
        &[RELAY],
    )
    .await;
    let seller_sdk = directory_sdk_with_signer_and_relays(
        seller_storage_root.as_path(),
        SELLER_SECRET_KEY_HEX,
        &[RELAY],
    )
    .await;
    let buyer_store =
        RadrootsEventStore::open_file(&buyer_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("buyer event store");
    let seller_store =
        RadrootsEventStore::open_file(&seller_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("seller event store");
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                trade_propose_request(
                    "trade-product-committed-resync",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000206")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );
    assert_eq!(
        seller_store
            .status_summary()
            .await
            .expect("seller isolated before proposal import")
            .total_events,
        0
    );
    let seller_proposal_resync_adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_event_item_from_store(&buyer_store, &propose_receipt.signed_event_id, RELAY, 4_000)
            .await,
        relay_eose(RELAY),
    ]);
    let seller_proposal_resync = seller_sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(propose_receipt.locator.clone()),
            &seller_proposal_resync_adapter,
        )
        .await
        .expect("seller proposal resync");
    assert_eq!(
        seller_proposal_resync.status.status,
        TradeStatusKind::Requested
    );
    assert_eq!(seller_proposal_resync.evidence.inserted_count, 1);
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
                    SatisfactionPolicy::NoWait,
                    TradeEvidenceMode::LocalOnly,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000207")
                .expect("accept idempotency"),
            )
            .await
            .expect("accept trade"),
    );
    let receipt_raw_event = signed_raw_validation_receipt_event(
        "trade-product-committed-resync",
        &propose_receipt.listing_event_id,
        &propose_receipt.signed_event_id,
        &accept_receipt.signed_event_id,
        33,
    );
    let receipt_event = radroots_event_from_nostr(&receipt_raw_event);
    let receipt_event_id =
        RadrootsEventId::parse(receipt_raw_event.id.to_hex().as_str()).expect("receipt id");
    let service_pubkey = public_key_hex_for_secret(SERVICE_SECRET_KEY_HEX);

    seller_store
        .ingest_event(RadrootsEventIngest::new(
            signed_event_from_envelope(receipt_event),
            4_040,
        ))
        .await
        .expect("ingest validation receipt");

    let seller_resync = seller_sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(propose_receipt.locator.clone()),
            &RadrootsMockRelayFetchAdapter::new(vec![relay_eose(RELAY)]),
        )
        .await
        .expect("seller resync");
    assert_eq!(
        seller_resync.status.status,
        TradeStatusKind::AgreedPendingValidation
    );
    assert_eq!(
        seller_resync.status.rhi_receipt_event_id,
        Some(receipt_event_id.clone())
    );
    assert_eq!(
        seller_resync.status.last_event_id,
        Some(accept_receipt.signed_event_id.clone())
    );
    assert_eq!(
        seller_resync
            .status
            .validation_trust
            .as_ref()
            .map(|trust| trust.state),
        Some(RadrootsTradeValidationTrustState::Untrusted)
    );

    let trusted_local_policy = validator_set_policy_for_validator_pubkey(service_pubkey.as_str());
    let seller_trusted_local = seller_sdk
        .trades()
        .status(
            TradeStatusRequest::new(propose_receipt.locator.clone())
                .with_validation_trust_policy(trusted_local_policy),
        )
        .await
        .expect("seller trusted local status");
    assert_eq!(seller_trusted_local.status, TradeStatusKind::Committed);
    let seller_trust = seller_trusted_local
        .validation_trust
        .as_ref()
        .expect("seller validation trust");
    assert_eq!(
        seller_trust.state,
        RadrootsTradeValidationTrustState::ValidatorSetCommitted
    );
    assert!(seller_trust.production_committed);
    assert_eq!(
        seller_trust
            .receipt_author
            .as_ref()
            .map(|pubkey| pubkey.as_str()),
        Some(service_pubkey.as_str())
    );

    let buyer_committed_resync_adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_event_item_from_store(&seller_store, &accept_receipt.signed_event_id, RELAY, 4_100)
            .await,
        relay_raw_event_item(&receipt_raw_event, RELAY, 4_200),
        relay_eose(RELAY),
    ]);
    let buyer_resync = buyer_sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(propose_receipt.locator),
            &buyer_committed_resync_adapter,
        )
        .await
        .expect("buyer resync");
    assert_eq!(
        buyer_resync.status.status,
        TradeStatusKind::AgreedPendingValidation
    );
    assert_eq!(
        buyer_resync.status.rhi_receipt_event_id,
        Some(receipt_event_id)
    );
    assert_eq!(buyer_resync.evidence.inserted_count, 2);
}

#[tokio::test]
async fn trade_status_trust_policy_requires_trusted_cryptographic_receipt_for_committed_confidence()
{
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let order_id = "trade-status-trusted-crypto";
    let request_event = signed_order_request_event(order_id, 70);
    let request_event_id = RadrootsEventId::parse(request_event.id_str()).expect("request id");
    let decision_event = signed_order_decision_event(order_id, &request_event_id, 71);
    let decision_event_id = RadrootsEventId::parse(decision_event.id_str()).expect("decision id");
    let listing_event_id = deterministic_event_id("listing-event");
    let receipt_raw_event = signed_raw_sp1_validation_receipt_event(
        order_id,
        &listing_event_id,
        &request_event_id,
        &decision_event_id,
        72,
    );
    let receipt_event = radroots_event_from_nostr(&receipt_raw_event);
    let receipt_event_id =
        RadrootsEventId::parse(receipt_raw_event.id.to_hex()).expect("receipt id");
    for (event, observed_at_ms) in [
        (request_event, 7_000),
        (decision_event, 7_100),
        (receipt_event, 7_200),
    ] {
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event_from_envelope(event),
                observed_at_ms,
            ))
            .await
            .expect("ingest trade status trust event");
    }

    let default_status = sdk
        .trades()
        .status(status_request(order_id))
        .await
        .expect("default status");
    assert_eq!(
        default_status.status,
        TradeStatusKind::AgreedPendingValidation
    );
    assert_eq!(
        default_status
            .validation_trust
            .as_ref()
            .map(|trust| trust.state),
        Some(RadrootsTradeValidationTrustState::Untrusted)
    );

    let service_pubkey = public_key_hex_for_secret(SERVICE_SECRET_KEY_HEX);
    let trusted_policy = validator_set_policy_for_validator_pubkey(service_pubkey.as_str());
    let trusted_status = sdk
        .trades()
        .status(status_request(order_id).with_validation_trust_policy(trusted_policy))
        .await
        .expect("trusted status");

    assert_eq!(trusted_status.status, TradeStatusKind::Committed);
    assert_eq!(
        trusted_status.rhi_receipt_event_id,
        Some(receipt_event_id.clone())
    );
    assert!(trusted_status.lifecycle_terminal);
    assert_eq!(
        trusted_status.next_action,
        TradeStatusNextActionKind::Terminal
    );
    let trust = trusted_status.validation_trust.expect("trusted decision");
    assert_eq!(
        trust.state,
        RadrootsTradeValidationTrustState::CryptographicCommitted
    );
    assert!(trust.production_committed);
    assert!(trust.cryptographic_proof_required);
    assert!(trust.cryptographic_proof_verified);
    assert_eq!(trust.proof_system.as_deref(), Some("sp1_core"));
    assert_eq!(
        trust.receipt_author.as_ref().map(|pubkey| pubkey.as_str()),
        Some(service_pubkey.as_str())
    );
}

#[cfg(all(
    feature = "signer-adapters",
    feature = "local-signer",
    feature = "transport-nostr-runtime"
))]
#[tokio::test]
async fn trade_product_accept_resync_before_mutation_imports_relay_visible_request() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let buyer_storage_root = tempdir.path().join("buyer-sdk");
    let seller_storage_root = tempdir.path().join("seller-sdk");
    let buyer_sdk = directory_sdk_with_signer_and_relays(
        buyer_storage_root.as_path(),
        BUYER_SECRET_KEY_HEX,
        &[RELAY],
    )
    .await;
    let seller_sdk = directory_sdk_with_signer_and_relays(
        seller_storage_root.as_path(),
        SELLER_SECRET_KEY_HEX,
        &[RELAY],
    )
    .await;
    let buyer_store =
        RadrootsEventStore::open_file(&buyer_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("buyer event store");
    let seller_store =
        RadrootsEventStore::open_file(&seller_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("seller event store");
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                trade_propose_request(
                    "01890f0e-6c00-7000-8000-000000000209",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000208")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );
    assert_eq!(
        seller_store
            .status_summary()
            .await
            .expect("seller isolated before mutation")
            .total_events,
        0
    );
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_event_item_from_store(&buyer_store, &propose_receipt.signed_event_id, RELAY, 4_300)
            .await,
        relay_eose(RELAY),
    ]);
    let accept_receipt = expect_enqueued(
        seller_sdk
            .trades()
            .seller()
            .accept_trade_with_fetch_adapter(
                TradeAcceptRequest::new(
                    seller_actor(),
                    propose_receipt.locator.clone(),
                    vec![RadrootsOrderInventoryCommitment {
                        bin_id: "bin-1".parse().expect("bin id"),
                        bin_count: 2,
                    }],
                    explicit_trade_relays(),
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                    TradeEvidenceMode::ResyncBeforeMutation,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000209")
                .expect("accept idempotency"),
                &adapter,
            )
            .await
            .expect("accept trade"),
    );
    assert_eq!(
        accept_receipt.request_event_id,
        propose_receipt.signed_event_id
    );
    let status = seller_sdk
        .trades()
        .status(TradeStatusRequest::new(propose_receipt.locator))
        .await
        .expect("seller status after accept");
    assert_eq!(status.status, TradeStatusKind::AgreedPendingValidation);
    assert_eq!(
        status.decision_event_id,
        Some(accept_receipt.signed_event_id)
    );
    assert_eq!(
        seller_store
            .status_summary()
            .await
            .expect("seller imported request and decision")
            .total_events,
        2
    );
}

#[cfg(all(
    feature = "signer-adapters",
    feature = "local-signer",
    feature = "transport-nostr-runtime"
))]
#[tokio::test]
async fn trade_product_accept_local_only_does_not_fetch_nostr_evidence() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let buyer_storage_root = tempdir.path().join("buyer-sdk");
    let seller_storage_root = tempdir.path().join("seller-sdk");
    let buyer_sdk = directory_sdk_with_signer_and_relays(
        buyer_storage_root.as_path(),
        BUYER_SECRET_KEY_HEX,
        &[RELAY],
    )
    .await;
    let seller_sdk = directory_sdk_with_signer_and_relays(
        seller_storage_root.as_path(),
        SELLER_SECRET_KEY_HEX,
        &[RELAY],
    )
    .await;
    let seller_store =
        RadrootsEventStore::open_file(&seller_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("seller event store");
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                trade_propose_request(
                    "trade-product-local-only-no-fetch",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-00000000020a")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![relay_closed(RELAY, "must not fetch")]);
    let error = seller_sdk
        .trades()
        .seller()
        .accept_trade_with_fetch_adapter(
            TradeAcceptRequest::new(
                seller_actor(),
                propose_receipt.locator,
                vec![RadrootsOrderInventoryCommitment {
                    bin_id: "bin-1".parse().expect("bin id"),
                    bin_count: 2,
                }],
                explicit_trade_relays(),
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
                TradeEvidenceMode::LocalOnly,
            ),
            &adapter,
        )
        .await
        .expect_err("local-only accept without local evidence");
    let RadrootsSdkError::InvalidRequest { message } = error else {
        panic!("expected invalid request");
    };
    assert!(message.contains("trade.accept requires a locally projected trade"));
    assert_eq!(
        seller_store
            .status_summary()
            .await
            .expect("seller remains empty")
            .total_events,
        0
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_accept_require_explicit_evidence_ingests_supplied_request() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let buyer_storage_root = tempdir.path().join("buyer-sdk");
    let seller_storage_root = tempdir.path().join("seller-sdk");
    let buyer_sdk =
        directory_sdk_with_signer(buyer_storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let seller_sdk =
        directory_sdk_with_signer(seller_storage_root.as_path(), SELLER_SECRET_KEY_HEX).await;
    let buyer_store =
        RadrootsEventStore::open_file(&buyer_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("buyer event store");
    let seller_store =
        RadrootsEventStore::open_file(&seller_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("seller event store");
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                trade_propose_request(
                    "01890f0e-6c00-7000-8000-00000000020c",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-00000000020b")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );
    let request_event = event_from_store(&buyer_store, &propose_receipt.signed_event_id).await;
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
                    SatisfactionPolicy::NoWait,
                    TradeEvidenceMode::require_explicit_evidence([
                        TradeEvidenceIngestRequest::new(signed_event_from_envelope(request_event)),
                    ]),
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-00000000020c")
                .expect("accept idempotency"),
            )
            .await
            .expect("accept trade"),
    );
    assert_eq!(
        accept_receipt.request_event_id,
        propose_receipt.signed_event_id
    );
    let status = seller_sdk
        .trades()
        .status(TradeStatusRequest::new(propose_receipt.locator))
        .await
        .expect("seller explicit status after accept");
    assert_eq!(status.status, TradeStatusKind::AgreedPendingValidation);
    assert_eq!(
        seller_store
            .status_summary()
            .await
            .expect("seller imported request and decision")
            .total_events,
        2
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_accept_require_explicit_evidence_rejects_empty_evidence() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let buyer_storage_root = tempdir.path().join("buyer-sdk");
    let seller_storage_root = tempdir.path().join("seller-sdk");
    let buyer_sdk =
        directory_sdk_with_signer(buyer_storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let seller_sdk =
        directory_sdk_with_signer(seller_storage_root.as_path(), SELLER_SECRET_KEY_HEX).await;
    let seller_store =
        RadrootsEventStore::open_file(&seller_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("seller event store");
    let propose_receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                trade_propose_request(
                    "trade-product-empty-explicit-accept",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-00000000020d")
                .expect("propose idempotency"),
            )
            .await
            .expect("propose trade"),
    );
    let error = seller_sdk
        .trades()
        .seller()
        .accept_trade(TradeAcceptRequest::new(
            seller_actor(),
            propose_receipt.locator,
            vec![RadrootsOrderInventoryCommitment {
                bin_id: "bin-1".parse().expect("bin id"),
                bin_count: 2,
            }],
            explicit_trade_relays(),
            PublishMode::EnqueueOnly,
            SatisfactionPolicy::NoWait,
            TradeEvidenceMode::require_explicit_evidence(Vec::<TradeEvidenceIngestRequest>::new()),
        ))
        .await
        .expect_err("empty explicit evidence");
    let RadrootsSdkError::InvalidRequest { message } = error else {
        panic!("expected invalid request");
    };
    assert_eq!(message, "trade.accept requires explicit trade evidence");
    assert_eq!(
        seller_store
            .status_summary()
            .await
            .expect("seller remains empty")
            .total_events,
        0
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_validation_receipts_fetch_from_relays_and_inspect_receipt() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let order_id = "trade-validation-receipts-sdk";
    let listing_event_id = deterministic_event_id("validation-receipt-listing");
    let root_event_id = deterministic_event_id("validation-receipt-request");
    let target_event_id = deterministic_event_id("validation-receipt-decision");
    let receipt_raw_event = signed_raw_validation_receipt_event(
        order_id,
        &listing_event_id,
        &root_event_id,
        &target_event_id,
        33,
    );
    let receipt_event_id =
        RadrootsEventId::parse(receipt_raw_event.id.to_hex()).expect("receipt event id");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_raw_event_item(&receipt_raw_event, RELAY, 4_000),
        relay_eose(RELAY),
    ]);

    let list = sdk
        .trades()
        .validation_receipts()
        .list_with_fetch_adapter(
            TradeValidationReceiptListRequest::parse(order_id).expect("list request"),
            &adapter,
        )
        .await
        .expect("validation receipt list");

    assert_eq!(list.nostr_relay_urls, vec![RELAY.to_owned()]);
    assert_eq!(list.receipts.len(), 1);
    assert!(list.invalid_receipts.is_empty());
    assert_eq!(list.nostr_evidence.out_of_filter_count, 0);
    assert_eq!(list.receipts[0].event.id_str(), receipt_event_id.as_str());
    assert_eq!(
        list.receipts[0].tags.root_event_id.as_str(),
        root_event_id.as_str()
    );

    let inspect = sdk
        .trades()
        .validation_receipts()
        .inspect_with_fetch_adapter(
            TradeValidationReceiptInspectRequest::new(receipt_event_id.clone()),
            &adapter,
        )
        .await
        .expect("validation receipt inspect");

    assert_eq!(inspect.receipt_event_id, receipt_event_id);
    assert!(inspect.invalid_receipt.is_none());
    assert_eq!(
        inspect
            .receipt
            .as_ref()
            .map(|receipt| receipt.tags.target_event_id.as_str()),
        Some(target_event_id.as_str())
    );

    let verify = sdk
        .trades()
        .validation_receipts()
        .verify_with_fetch_adapter(
            TradeValidationReceiptVerifyRequest::new(receipt_event_id.clone()),
            &adapter,
        )
        .await
        .expect("validation receipt verify");
    assert_eq!(
        verify
            .receipt
            .as_ref()
            .map(|receipt| receipt.tags.listing_event_id.as_str()),
        Some(listing_event_id.as_str())
    );
    assert!(
        store
            .get_event(receipt_event_id.as_str())
            .await
            .expect("stored receipt lookup")
            .is_some()
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_validation_receipt_list_rejects_out_of_filter_order_receipts() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let requested_order_id = "validation-receipt-list-requested";
    let unrelated_order_id = "validation-receipt-list-unrelated";
    let unrelated_receipt = signed_raw_validation_receipt_event(
        unrelated_order_id,
        &deterministic_event_id("validation-receipt-list-unrelated-listing"),
        &deterministic_event_id("validation-receipt-list-unrelated-request"),
        &deterministic_event_id("validation-receipt-list-unrelated-decision"),
        36,
    );
    let unrelated_receipt_id =
        RadrootsEventId::parse(unrelated_receipt.id.to_hex()).expect("unrelated receipt id");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_raw_event_item(&unrelated_receipt, RELAY, 4_010),
        relay_eose(RELAY),
    ]);

    let list = sdk
        .trades()
        .validation_receipts()
        .list_with_fetch_adapter(
            TradeValidationReceiptListRequest::parse(requested_order_id).expect("list request"),
            &adapter,
        )
        .await
        .expect("validation receipt list");

    assert!(list.receipts.is_empty());
    assert!(list.invalid_receipts.is_empty());
    assert_eq!(list.nostr_evidence.inserted_count, 0);
    assert_eq!(list.nostr_evidence.out_of_filter_count, 1);
    assert!(list.nostr_evidence.events[0].out_of_filter);
    assert!(
        store
            .get_event(unrelated_receipt_id.as_str())
            .await
            .expect("unrelated receipt lookup")
            .is_none()
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_validation_receipt_inspect_rejects_unrequested_relay_receipts() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let requested_receipt = signed_raw_validation_receipt_event(
        "validation-receipt-inspect-requested",
        &deterministic_event_id("validation-receipt-inspect-requested-listing"),
        &deterministic_event_id("validation-receipt-inspect-requested-request"),
        &deterministic_event_id("validation-receipt-inspect-requested-decision"),
        37,
    );
    let requested_receipt_id =
        RadrootsEventId::parse(requested_receipt.id.to_hex()).expect("requested receipt id");
    let unrelated_receipt = signed_raw_validation_receipt_event(
        "validation-receipt-inspect-unrelated",
        &deterministic_event_id("validation-receipt-inspect-unrelated-listing"),
        &deterministic_event_id("validation-receipt-inspect-unrelated-request"),
        &deterministic_event_id("validation-receipt-inspect-unrelated-decision"),
        38,
    );
    let unrelated_receipt_id =
        RadrootsEventId::parse(unrelated_receipt.id.to_hex()).expect("unrelated receipt id");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_raw_event_item(&unrelated_receipt, RELAY, 4_020),
        relay_eose(RELAY),
    ]);

    let inspect = sdk
        .trades()
        .validation_receipts()
        .inspect_with_fetch_adapter(
            TradeValidationReceiptInspectRequest::new(requested_receipt_id.clone()),
            &adapter,
        )
        .await
        .expect("validation receipt inspect");

    assert_eq!(inspect.receipt_event_id, requested_receipt_id);
    assert!(inspect.receipt.is_none());
    assert!(inspect.invalid_receipt.is_none());
    assert_eq!(inspect.nostr_evidence.inserted_count, 0);
    assert_eq!(inspect.nostr_evidence.out_of_filter_count, 1);
    assert!(inspect.nostr_evidence.events[0].out_of_filter);
    assert!(
        store
            .get_event(unrelated_receipt_id.as_str())
            .await
            .expect("unrelated receipt lookup")
            .is_none()
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_validation_receipt_inspect_skips_noise_before_exact_receipt_match() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let requested_receipt = signed_raw_validation_receipt_event(
        "validation-receipt-inspect-noise-requested",
        &deterministic_event_id("validation-receipt-inspect-noise-requested-listing"),
        &deterministic_event_id("validation-receipt-inspect-noise-requested-request"),
        &deterministic_event_id("validation-receipt-inspect-noise-requested-decision"),
        39,
    );
    let requested_receipt_id =
        RadrootsEventId::parse(requested_receipt.id.to_hex()).expect("requested receipt id");
    let unrelated_receipt = signed_raw_validation_receipt_event(
        "validation-receipt-inspect-noise-unrelated",
        &deterministic_event_id("validation-receipt-inspect-noise-unrelated-listing"),
        &deterministic_event_id("validation-receipt-inspect-noise-unrelated-request"),
        &deterministic_event_id("validation-receipt-inspect-noise-unrelated-decision"),
        40,
    );
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_malformed(RELAY),
        relay_raw_event_item(&unrelated_receipt, RELAY, 4_021),
        relay_raw_event_item(&requested_receipt, RELAY, 4_022),
        relay_eose(RELAY),
    ]);

    let inspect = sdk
        .trades()
        .validation_receipts()
        .inspect_with_fetch_adapter(
            TradeValidationReceiptInspectRequest::new(requested_receipt_id.clone()),
            &adapter,
        )
        .await
        .expect("validation receipt inspect");

    assert_eq!(inspect.receipt_event_id, requested_receipt_id);
    assert!(inspect.receipt.is_some());
    assert!(inspect.invalid_receipt.is_none());
    assert_eq!(inspect.nostr_evidence.inserted_count, 1);
    assert_eq!(inspect.nostr_evidence.malformed_count, 1);
    assert_eq!(inspect.nostr_evidence.out_of_filter_count, 1);
    assert_eq!(inspect.nostr_evidence.skipped_over_limit_count, 0);
    assert!(
        inspect
            .nostr_evidence
            .events
            .iter()
            .any(|event| event.malformed)
    );
    assert!(
        inspect
            .nostr_evidence
            .events
            .iter()
            .any(|event| event.out_of_filter)
    );
    assert!(
        store
            .get_event(requested_receipt_id.as_str())
            .await
            .expect("requested receipt lookup")
            .is_some()
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_resync_imports_nostr_evidence_into_empty_local_store() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let request_event = signed_raw_order_request_event("resync-empty-local-import", 41);
    let request_event_id =
        RadrootsEventId::parse(request_event.id.to_hex().as_str()).expect("event id");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_raw_event_item(&request_event, RELAY, 5_000),
        relay_eose(RELAY),
    ]);

    let resync = sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(RadrootsTradeLocator::from_order_id(order_id(
                "resync-empty-local-import",
            ))),
            &adapter,
        )
        .await
        .expect("resync");

    assert_eq!(resync.status.status, TradeStatusKind::Requested);
    assert_eq!(resync.evidence.inserted_count, 1);
    assert_eq!(resync.evidence.duplicate_count, 0);
    assert_eq!(resync.evidence.query_plan.branches.len(), 4);
    assert!(
        resync
            .evidence
            .query_plan
            .branches
            .iter()
            .any(|branch| branch.kind == TradeEvidenceQueryBranchKind::RequestRoots)
    );
    let request_branch = resync
        .evidence
        .branches
        .iter()
        .find(|branch| branch.branch == TradeEvidenceQueryBranchKind::RequestRoots)
        .expect("request root branch");
    assert_eq!(request_branch.accepted_count, 1);
    assert_eq!(request_branch.inserted_count, 1);
    assert!(!request_branch.empty_result);
    let lifecycle_branch = resync
        .evidence
        .branches
        .iter()
        .find(|branch| branch.branch == TradeEvidenceQueryBranchKind::LifecycleChain)
        .expect("lifecycle branch");
    assert!(lifecycle_branch.empty_result);
    assert_eq!(
        resync.evidence.events[0].event_id.as_deref(),
        Some(request_event_id.as_str())
    );
    assert!(
        store
            .get_event(request_event_id.as_str())
            .await
            .expect("stored event")
            .is_some()
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_status_local_only_ignores_failing_fetch_adapter() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![relay_closed(RELAY, "must not fetch")]);

    let status = sdk
        .trades()
        .status_with_fetch_adapter(
            status_request("status-local-only-no-fetch")
                .with_source(SdkTradeStatusSource::LocalOnly),
            &adapter,
        )
        .await
        .expect("local-only status");

    assert_eq!(status.source, SdkTradeStatusSource::LocalOnly);
    assert_eq!(status.status, TradeStatusKind::Missing);
    assert!(status.online_evidence.is_none());
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_status_resync_then_local_fetches_evidence_before_status() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let request_event = signed_raw_order_request_event("status-resync-then-local", 46);
    let request_event_id =
        RadrootsEventId::parse(request_event.id.to_hex().as_str()).expect("request id");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_raw_event_item(&request_event, RELAY, 5_250),
        relay_eose(RELAY),
    ]);

    let status = sdk
        .trades()
        .status_with_fetch_adapter(
            status_request("status-resync-then-local")
                .with_source(SdkTradeStatusSource::ResyncThenLocal),
            &adapter,
        )
        .await
        .expect("resync-then-local status");

    assert_eq!(status.source, SdkTradeStatusSource::ResyncThenLocal);
    assert_eq!(status.status, TradeStatusKind::Requested);
    assert_eq!(status.request_event_id, Some(request_event_id.clone()));
    let evidence = status.online_evidence.as_ref().expect("online evidence");
    assert_eq!(evidence.inserted_count, 1);
    let request_branch = evidence
        .branches
        .iter()
        .find(|branch| branch.branch == TradeEvidenceQueryBranchKind::RequestRoots)
        .expect("request branch");
    assert_eq!(request_branch.accepted_count, 1);
    assert_eq!(
        request_branch.events[0].event_id.as_deref(),
        Some(request_event_id.as_str())
    );
    assert!(
        store
            .get_event(request_event_id.as_str())
            .await
            .expect("stored event")
            .is_some()
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_resync_skips_noise_before_matching_trade_event() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let requested_order_id = order_id("resync-noise-requested");
    let request_event = signed_raw_order_request_event(requested_order_id.as_str(), 44);
    let request_event_id =
        RadrootsEventId::parse(request_event.id.to_hex()).expect("request event id");
    let unrelated_event = signed_raw_order_request_event("resync-noise-unrelated", 45);
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_malformed(RELAY),
        relay_raw_event_item(&unrelated_event, RELAY, 5_020),
        relay_raw_event_item(&request_event, RELAY, 5_021),
        relay_eose(RELAY),
    ]);

    let resync = sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(RadrootsTradeLocator::from_order_id(requested_order_id)),
            &adapter,
        )
        .await
        .expect("resync");

    assert_eq!(resync.status.status, TradeStatusKind::Requested);
    assert_eq!(resync.evidence.inserted_count, 1);
    assert_eq!(resync.evidence.malformed_count, 1);
    assert_eq!(resync.evidence.out_of_filter_count, 1);
    assert_eq!(resync.evidence.skipped_over_limit_count, 0);
    assert!(
        store
            .get_event(request_event_id.as_str())
            .await
            .expect("stored event")
            .is_some()
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_resync_duplicate_replay_is_idempotent() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let request_event = signed_raw_order_request_event("resync-duplicate-replay", 42);
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_raw_event_item(&request_event, RELAY, 5_100),
        relay_eose(RELAY),
    ]);
    let locator = RadrootsTradeLocator::from_order_id(order_id("resync-duplicate-replay"));

    let first = sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(TradeResyncRequest::new(locator.clone()), &adapter)
        .await
        .expect("first resync");
    let second = sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(TradeResyncRequest::new(locator), &adapter)
        .await
        .expect("second resync");

    assert_eq!(first.evidence.inserted_count, 1);
    assert_eq!(second.evidence.inserted_count, 0);
    assert_eq!(second.evidence.duplicate_count, 1);
    assert_eq!(second.status.status, TradeStatusKind::Requested);
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_resync_splits_lifecycle_branch_into_single_kind_filters() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let adapter = CapturingRelayFetchAdapter::default();
    sdk.trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(RadrootsTradeLocator::from_order_id(order_id(
                "resync-lifecycle-filter-shape",
            ))),
            &adapter,
        )
        .await
        .expect("resync");

    let filters = adapter.filters_json();
    assert_eq!(filters.len(), 4);
    for kind in [3422, 3423, 3432, 3440] {
        assert!(
            filters
                .iter()
                .any(|filter| filter.contains(format!("\"kinds\":[{kind}]").as_str())),
            "missing single-kind filter for {kind}: {filters:?}"
        );
    }
    assert!(
        !filters
            .iter()
            .any(|filter| filter.contains("\"kinds\":[3423,3424,3425,3432]"))
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_resync_reports_malformed_evidence_without_poisoning_store() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let adapter =
        RadrootsMockRelayFetchAdapter::new(vec![relay_malformed(RELAY), relay_eose(RELAY)]);

    let resync = sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(RadrootsTradeLocator::from_order_id(order_id(
                "resync-malformed-evidence",
            ))),
            &adapter,
        )
        .await
        .expect("resync");

    assert_eq!(resync.status.status, TradeStatusKind::Missing);
    assert_eq!(resync.evidence.malformed_count, 1);
    assert_eq!(resync.evidence.inserted_count, 0);
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("store summary")
            .total_events,
        0
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_resync_rejects_out_of_filter_evidence_without_poisoning_store() {
    let (_tempdir, sdk, store) = directory_sdk_and_store_with_relays(&[RELAY]).await;
    let unrelated_event = signed_raw_order_request_event("resync-out-of-filter-unrelated", 43);
    let unrelated_event_id =
        RadrootsEventId::parse(unrelated_event.id.to_hex()).expect("unrelated event id");
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_raw_event_item(&unrelated_event, RELAY, 5_150),
        relay_eose(RELAY),
    ]);

    let resync = sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(RadrootsTradeLocator::from_order_id(order_id(
                "resync-out-of-filter-requested",
            ))),
            &adapter,
        )
        .await
        .expect("resync");

    assert_eq!(resync.status.status, TradeStatusKind::Missing);
    assert_eq!(resync.evidence.inserted_count, 0);
    assert_eq!(resync.evidence.out_of_filter_count, 1);
    assert!(resync.evidence.events[0].out_of_filter);
    assert!(
        store
            .get_event(unrelated_event_id.as_str())
            .await
            .expect("unrelated event lookup")
            .is_none()
    );
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_resync_errors_on_total_relay_failure() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store_with_relays(&[RELAY, RELAY_B]).await;
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_closed(RELAY, "timeout: relay offline"),
        relay_closed(RELAY_B, "error: relay unavailable"),
    ]);

    let error = sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(RadrootsTradeLocator::from_order_id(order_id(
                "resync-total-relay-failure",
            ))),
            &adapter,
        )
        .await
        .expect_err("total relay failure");

    assert_eq!(error.code(), "product_sync_transport_setup_failure");
}

#[cfg(feature = "transport-nostr-runtime")]
#[tokio::test]
async fn trade_resync_reports_partial_relay_failure() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store_with_relays(&[RELAY, RELAY_B]).await;
    let request_event = signed_raw_order_request_event("resync-partial-relay-failure", 43);
    let adapter = RadrootsMockRelayFetchAdapter::new(vec![
        relay_closed(RELAY_B, "timeout: relay unavailable"),
        relay_raw_event_item(&request_event, RELAY, 5_200),
        relay_eose(RELAY),
    ]);

    let resync = sdk
        .trades()
        .resync()
        .resync_with_fetch_adapter(
            TradeResyncRequest::new(RadrootsTradeLocator::from_order_id(order_id(
                "resync-partial-relay-failure",
            ))),
            &adapter,
        )
        .await
        .expect("partial failure resync");

    assert_eq!(resync.status.status, TradeStatusKind::Requested);
    assert_eq!(resync.evidence.closed_count, 1);
    assert_eq!(resync.evidence.eose_count, 1);
    assert_eq!(
        resync.evidence.nostr_relay_outcomes[0].outcome_kind,
        TradeResyncNostrRelayOutcomeKind::Closed
    );
    assert_eq!(
        resync.evidence.nostr_relay_outcomes[0].transport_outcome_kind,
        Some(TradeResyncNostrRelayTransportOutcomeKind::Timeout)
    );
}

#[cfg(feature = "transport-nostr-runtime")]
async fn relay_event_item_from_store(
    source: &RadrootsEventStore,
    event_id: &RadrootsEventId,
    relay_url: &str,
    observed_at_ms: i64,
) -> RadrootsRelayFetchItem {
    let stored = source
        .get_event(event_id.as_str())
        .await
        .expect("source event lookup")
        .expect("source event");
    RadrootsRelayFetchItem::Event {
        relay_url: relay_url.to_owned(),
        raw_json: stored.raw_json,
        observed_at_ms,
    }
}

#[cfg(feature = "signer-adapters")]
async fn event_from_store(
    source: &RadrootsEventStore,
    event_id: &RadrootsEventId,
) -> RadrootsEventEnvelope {
    let stored = source
        .get_event(event_id.as_str())
        .await
        .expect("source event lookup")
        .expect("source event");
    let event =
        serde_json::from_str::<nostr::Event>(stored.raw_json.as_str()).expect("stored raw event");
    radroots_event_from_nostr(&event)
}

#[cfg(feature = "transport-nostr-runtime")]
fn relay_raw_event_item(
    event: &nostr::Event,
    relay_url: &str,
    observed_at_ms: i64,
) -> RadrootsRelayFetchItem {
    RadrootsRelayFetchItem::Event {
        relay_url: relay_url.to_owned(),
        raw_json: event.as_json(),
        observed_at_ms,
    }
}

#[cfg(feature = "transport-nostr-runtime")]
fn relay_eose(relay_url: &str) -> RadrootsRelayFetchItem {
    RadrootsRelayFetchItem::Eose {
        relay_url: relay_url.to_owned(),
    }
}

#[cfg(feature = "transport-nostr-runtime")]
fn relay_closed(relay_url: &str, message: &str) -> RadrootsRelayFetchItem {
    RadrootsRelayFetchItem::Closed {
        relay_url: relay_url.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(feature = "transport-nostr-runtime")]
fn relay_malformed(relay_url: &str) -> RadrootsRelayFetchItem {
    RadrootsRelayFetchItem::Event {
        relay_url: relay_url.to_owned(),
        raw_json: "{".to_owned(),
        observed_at_ms: 4_999,
    }
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_propose_idempotency_replays_same_payload_and_conflicts_different_payload() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;
    let storage_paths = buyer_sdk.storage_paths().expect("storage paths");
    let store = RadrootsEventStore::open_file(&storage_paths.runtime_path)
        .await
        .expect("event store");
    let outbox = RadrootsOutbox::open_file(&storage_paths.runtime_path)
        .await
        .expect("outbox");
    let request = trade_propose_request(
        "trade-product-idempotent",
        PublishMode::EnqueueOnly,
        SatisfactionPolicy::NoWait,
    )
    .try_with_idempotency_key("01890f0e-6c00-7000-8000-00000000020e")
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
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store summary")
            .total_events,
        1
    );
    assert_eq!(
        outbox
            .status_summary(i64::MAX)
            .await
            .expect("outbox summary")
            .total_events,
        1
    );

    let conflict = buyer_sdk
        .trades()
        .buyer()
        .propose_trade(
            trade_propose_request(
                "trade-product-idempotent-conflict",
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            )
            .try_with_idempotency_key("01890f0e-6c00-7000-8000-00000000020e")
            .expect("conflict idempotency"),
        )
        .await
        .expect_err("different payload conflict");

    assert!(matches!(
        conflict,
        RadrootsSdkError::IdempotencyConflict {
            ref operation_kind,
            ..
        } if operation_kind == TRADE_SUBMIT_OPERATION_KIND
    ));
    assert_eq!(conflict.code(), "idempotency_conflict");
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store summary")
            .total_events,
        1
    );
    assert_eq!(
        outbox
            .status_summary(i64::MAX)
            .await
            .expect("outbox summary")
            .total_events,
        1
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_propose_requires_public_note_privacy_confirmation() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;

    let missing_confirmation = buyer_sdk
        .trades()
        .buyer()
        .propose_trade(
            trade_propose_request(
                "trade-product-propose-public-note",
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            )
            .with_public_note("please leave at the community table"),
        )
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
    assert_eq!(operation, "trade.propose");
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
        RadrootsEventStore::open_file(&buyer_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("event store");
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
}

#[cfg(all(feature = "signer-adapters", feature = "local-signer"))]
#[tokio::test]
async fn trade_product_propose_publishes_public_note_after_confirmation() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;

    let receipt = expect_enqueued(
        buyer_sdk
            .trades()
            .buyer()
            .propose_trade(
                trade_propose_request(
                    "01890f0e-6c00-7000-8000-00000000020f",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .with_public_note("please leave at the community table")
                .with_privacy_confirmation(public_note_confirmation())
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-00000000020f")
                .expect("proposal idempotency"),
            )
            .await
            .expect("confirmed proposal"),
    );

    assert_eq!(
        receipt.order_id.as_str(),
        "01890f0e-6c00-7000-8000-00000000020f"
    );
    let store =
        RadrootsEventStore::open_file(&buyer_sdk.storage_paths().expect("paths").runtime_path)
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
async fn trade_product_propose_blocks_sensitive_fulfillment_note_even_when_confirmed() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let storage_root = tempdir.path().join("sdk");
    let buyer_sdk = directory_sdk_with_signer(storage_root.as_path(), BUYER_SECRET_KEY_HEX).await;

    let forbidden = buyer_sdk
        .trades()
        .buyer()
        .propose_trade(
            trade_propose_request(
                "trade-product-propose-sensitive-note",
                PublishMode::EnqueueOnly,
                SatisfactionPolicy::NoWait,
            )
            .with_public_note("pickup address is 123 Farm Lane")
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
    assert_eq!(operation, "trade.propose");
    assert_eq!(*status, PrivacyPreflightStatus::ForbiddenPublicFields);
    assert!(fields.contains(&ProductSensitivityField::SensitiveFulfillmentDetails));
    let store =
        RadrootsEventStore::open_file(&buyer_sdk.storage_paths().expect("paths").runtime_path)
            .await
            .expect("event store");
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
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
                trade_propose_request(
                    "trade-product-privacy-decline",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000210")
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
            SatisfactionPolicy::NoWait,
            TradeEvidenceMode::LocalOnly,
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
        RadrootsEventStore::open_file(&seller_sdk.storage_paths().expect("paths").runtime_path)
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
                    SatisfactionPolicy::NoWait,
                    TradeEvidenceMode::LocalOnly,
                )
                .with_privacy_confirmation(public_note_confirmation())
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000211")
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
                trade_propose_request(
                    "trade-product-privacy-cancel",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000212")
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
                SatisfactionPolicy::NoWait,
                TradeEvidenceMode::LocalOnly,
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
        RadrootsEventStore::open_file(&buyer_sdk.storage_paths().expect("paths").runtime_path)
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
                trade_propose_request(
                    "01890f0e-6c00-7000-8000-000000000214",
                    PublishMode::EnqueueOnly,
                    SatisfactionPolicy::NoWait,
                )
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000213")
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
                    SatisfactionPolicy::NoWait,
                    TradeEvidenceMode::LocalOnly,
                )
                .with_privacy_confirmation(public_note_confirmation())
                .try_with_idempotency_key("01890f0e-6c00-7000-8000-000000000214")
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

#[cfg(feature = "signer-adapters")]
#[tokio::test]
async fn trade_product_propose_dry_run_returns_plan_without_local_side_effects() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let outcome = sdk
        .trades()
        .buyer()
        .propose_trade(trade_propose_request(
            "trade-product-dry-run",
            PublishMode::DryRun,
            SatisfactionPolicy::NoWait,
        ))
        .await
        .expect("dry-run proposal");
    let plan = match outcome {
        TradeMutationOutcome::DryRun { plan } => plan,
        TradeMutationOutcome::Enqueued { .. } => panic!("expected dry-run outcome"),
        TradeMutationOutcome::Published { .. } => panic!("expected dry-run outcome"),
    };

    assert_eq!(plan.order_id.as_str(), "trade-product-dry-run");
    assert_eq!(plan.frozen_draft.kind_u32(), KIND_ORDER_REQUEST);
    assert_eq!(plan.expected_event_id, plan.workflow.expected_event_id);
    assert_eq!(
        store
            .status_summary()
            .await
            .expect("event store status")
            .total_events,
        0
    );
    let outbox = RadrootsOutbox::open_file(&sdk.storage_paths().expect("paths").runtime_path)
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

#[cfg(feature = "transport-nostr-runtime")]
fn signed_raw_validation_receipt_event(
    raw_order_id: &str,
    listing_event_id: &RadrootsEventId,
    root_event_id: &RadrootsEventId,
    target_event_id: &RadrootsEventId,
    created_at: u32,
) -> nostr::Event {
    signed_raw_event(
        SERVICE_SECRET_KEY_HEX,
        created_at,
        validation_receipt_wire_parts(
            raw_order_id,
            listing_event_id,
            root_event_id,
            target_event_id,
        ),
    )
}

fn signed_raw_sp1_validation_receipt_event(
    raw_order_id: &str,
    listing_event_id: &RadrootsEventId,
    root_event_id: &RadrootsEventId,
    target_event_id: &RadrootsEventId,
    created_at: u32,
) -> nostr::Event {
    signed_raw_event(
        SERVICE_SECRET_KEY_HEX,
        created_at,
        validation_receipt_wire_parts_with_proof(
            raw_order_id,
            listing_event_id,
            root_event_id,
            target_event_id,
            RadrootsValidationReceiptProofSystem::Sp1Core,
        ),
    )
}

#[cfg(feature = "transport-nostr-runtime")]
fn validation_receipt_wire_parts(
    raw_order_id: &str,
    listing_event_id: &RadrootsEventId,
    root_event_id: &RadrootsEventId,
    target_event_id: &RadrootsEventId,
) -> RadrootsNip01EventWireParts {
    validation_receipt_wire_parts_with_proof(
        raw_order_id,
        listing_event_id,
        root_event_id,
        target_event_id,
        RadrootsValidationReceiptProofSystem::None,
    )
}

fn validation_receipt_wire_parts_with_proof(
    raw_order_id: &str,
    listing_event_id: &RadrootsEventId,
    root_event_id: &RadrootsEventId,
    target_event_id: &RadrootsEventId,
    proof_system: RadrootsValidationReceiptProofSystem,
) -> RadrootsNip01EventWireParts {
    let proof = match proof_system {
        RadrootsValidationReceiptProofSystem::None => RadrootsValidationReceiptProof {
            inline_proof_base64: None,
            mode: None,
            program_hash: None,
            proof_reference: None,
            system: RadrootsValidationReceiptProofSystem::None,
            verifying_key_hash: None,
        },
        RadrootsValidationReceiptProofSystem::Sp1Core => RadrootsValidationReceiptProof {
            inline_proof_base64: Some("AQID".to_owned()),
            mode: Some("core".to_owned()),
            program_hash: Some(hash32('a')),
            proof_reference: None,
            system: RadrootsValidationReceiptProofSystem::Sp1Core,
            verifying_key_hash: Some(hash32('b')),
        },
        RadrootsValidationReceiptProofSystem::Sp1Compressed => RadrootsValidationReceiptProof {
            inline_proof_base64: Some("AQID".to_owned()),
            mode: Some("compressed".to_owned()),
            program_hash: Some(hash32('a')),
            proof_reference: None,
            system: RadrootsValidationReceiptProofSystem::Sp1Compressed,
            verifying_key_hash: Some(hash32('b')),
        },
        RadrootsValidationReceiptProofSystem::Sp1Groth16 => RadrootsValidationReceiptProof {
            inline_proof_base64: Some("AQID".to_owned()),
            mode: Some("groth16".to_owned()),
            program_hash: Some(hash32('a')),
            proof_reference: None,
            system: RadrootsValidationReceiptProofSystem::Sp1Groth16,
            verifying_key_hash: Some(hash32('b')),
        },
        RadrootsValidationReceiptProofSystem::Sp1Plonk => RadrootsValidationReceiptProof {
            inline_proof_base64: Some("AQID".to_owned()),
            mode: Some("plonk".to_owned()),
            program_hash: Some(hash32('a')),
            proof_reference: None,
            system: RadrootsValidationReceiptProofSystem::Sp1Plonk,
            verifying_key_hash: Some(hash32('b')),
        },
    };
    let receipt = RadrootsTradeValidationReceipt {
        changed_records_root: hash32('6'),
        domain: "radroots.receipt".to_owned(),
        error_bitmap: "0x00000000000000000000000000000000".to_owned(),
        event_set_root: hash32('c'),
        new_state_root: hash32('4'),
        previous_state_root: hash32('3'),
        proof,
        public_values_hash: validation_receipt_public_values_hash_hex(br#"{"schema_version":1}"#),
        receipt_type: RadrootsValidationReceiptType::TradeTransition,
        result: RadrootsValidationReceiptResult::Valid,
        statement: RadrootsValidationReceiptStatement {
            listing_event_id: listing_event_id.as_str().to_owned(),
            root_event_id: root_event_id.as_str().to_owned(),
            target_event_id: target_event_id.as_str().to_owned(),
            validator_set_addr: validator_set_address_from_str(validator_set_addr_raw(
                public_key_hex_for_secret(SERVICE_SECRET_KEY_HEX).as_str(),
            ))
            .expect("validator set address"),
            validator_set_event_id: validator_set_event_id().into_string(),
            statement_type: RadrootsValidationReceiptType::TradeTransition,
        },
        version: 1,
    };
    validation_receipt_event_build(raw_order_id, &receipt).expect("receipt event")
}

fn public_key_hex_for_secret(secret_key_hex: &str) -> String {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    RadrootsNostrKeys::new(secret_key).public_key().to_hex()
}

async fn insert_perf_non_trade_events(store: &RadrootsEventStore, base: i64, count: i64) {
    let mut inserted = 0;
    while inserted < count {
        let batch = (count - inserted).min(1_000);
        sqlx::query(
            "WITH RECURSIVE seq(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM seq WHERE n + 1 < ?)
             INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms)
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
             INSERT INTO event_envelopes(event_id, pubkey, created_at, kind, tags_json, content, sig, raw_json, verification_status, contract_status, contract_id, event_class, projection_eligible, inserted_at_ms, updated_at_ms)
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
             INSERT INTO event_envelope_tags(event_id, tag_index, tag_name, tag_value, tag_json, contract_semantic, contract_value_type, relay_indexed)
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

async fn ingest_status_noise_events(
    store: &RadrootsEventStore,
    non_trade_count: i64,
    trade_count: i64,
) {
    for index in 0..non_trade_count {
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event_from_envelope(signed_status_noise_post_event(
                    index,
                    32_000 + index as u32,
                )),
                1_700_200_000_000 + index,
            ))
            .await
            .expect("non-trade status noise ingest");
    }

    for index in 0..trade_count {
        let order_id = format!("status-noise-background-{index:03}");
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event_from_envelope(signed_order_request_event(
                    &order_id,
                    33_000 + index as u32,
                )),
                1_700_200_100_000 + index,
            ))
            .await
            .expect("trade status noise ingest");
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
    parts: RadrootsNip01EventWireParts,
) -> RadrootsEventEnvelope {
    let event = signed_raw_event(secret_key_hex, created_at, parts);
    radroots_event_from_nostr(&event)
}

fn signed_raw_event(
    secret_key_hex: &str,
    created_at: u32,
    parts: RadrootsNip01EventWireParts,
) -> nostr::Event {
    let secret_key = RadrootsNostrSecretKey::from_hex(secret_key_hex).expect("secret key");
    let keys = RadrootsNostrKeys::new(secret_key);
    radroots_nostr_build_event(parts.kind, parts.content, parts.tags)
        .expect("event builder")
        .custom_created_at(RadrootsNostrTimestamp::from_secs(u64::from(created_at)))
        .sign_with_keys(&keys)
        .expect("signed event")
}

fn signed_order_request_event(raw_order_id: &str, created_at: u32) -> RadrootsEventEnvelope {
    let draft = radroots_event_codec::order::order_request_event_build(
        &listing_event_ptr(),
        &order_request(raw_order_id),
    )
    .expect("request draft");
    signed_event(BUYER_SECRET_KEY_HEX, created_at, draft)
}

#[cfg(feature = "transport-nostr-runtime")]
fn signed_raw_order_request_event(raw_order_id: &str, created_at: u32) -> nostr::Event {
    let draft = radroots_event_codec::order::order_request_event_build(
        &listing_event_ptr(),
        &order_request(raw_order_id),
    )
    .expect("request draft");
    signed_raw_event(BUYER_SECRET_KEY_HEX, created_at, draft)
}

fn signed_order_decision_event(
    raw_order_id: &str,
    root_event_id: &RadrootsEventId,
    created_at: u32,
) -> RadrootsEventEnvelope {
    let draft = radroots_event_codec::order::order_decision_event_build(
        root_event_id,
        root_event_id,
        &order_decision(raw_order_id),
    )
    .expect("decision draft");
    signed_event(SELLER_SECRET_KEY_HEX, created_at, draft)
}

fn signed_status_noise_post_event(index: i64, created_at: u32) -> RadrootsEventEnvelope {
    signed_event(
        SELLER_SECRET_KEY_HEX,
        created_at,
        RadrootsNip01EventWireParts {
            kind: KIND_POST,
            content: format!("local status noise {index}"),
            tags: Vec::new(),
        },
    )
}

fn signed_non_order_event(created_at: u32) -> RadrootsEventEnvelope {
    signed_event(
        SELLER_SECRET_KEY_HEX,
        created_at,
        RadrootsNip01EventWireParts {
            kind: KIND_LISTING,
            content: "{}".to_owned(),
            tags: vec![vec!["d".to_owned(), "not-an-order".to_owned()]],
        },
    )
}

#[tokio::test]
async fn order_evidence_ingest_stores_lifecycle_evidence_for_projection() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-evidence-ingest", 39);
    let request_event_id = RadrootsEventId::parse(request_event.id_str()).expect("request id");
    let decision_event =
        signed_order_decision_event("order-evidence-ingest", &request_event_id, 40);

    let request_receipt = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(signed_event_from_envelope(
            request_event.clone(),
        )))
        .await
        .expect("request evidence");
    assert_eq!(request_receipt.order_id.as_str(), "order-evidence-ingest");
    assert_eq!(request_receipt.event_kind, KIND_ORDER_REQUEST);
    assert_eq!(request_receipt.local_event_seq, 1);
    assert!(request_receipt.inserted);

    let decision_receipt = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(signed_event_from_envelope(
            decision_event.clone(),
        )))
        .await
        .expect("decision evidence");
    assert_eq!(decision_receipt.order_id.as_str(), "order-evidence-ingest");
    assert_eq!(decision_receipt.event_kind, KIND_ORDER_DECISION);
    assert_eq!(decision_receipt.local_event_seq, 2);
    assert!(decision_receipt.inserted);

    let duplicate_receipt = sdk
        .trades()
        .ingest_evidence(TradeEvidenceIngestRequest::new(signed_event_from_envelope(
            decision_event,
        )))
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
    assert_eq!(status.status, TradeStatusKind::AgreedPendingValidation);
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
        .ingest_evidence(TradeEvidenceIngestRequest::new(signed_event_from_envelope(
            signed_non_order_event(41),
        )))
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
        .ingest_request_evidence(TradeRequestEvidenceIngestRequest::new(
            signed_event_from_envelope(decision_event),
        ))
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
async fn order_status_returns_not_found_for_missing_local_order() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request = status_request("order-1");

    assert_eq!(request.limit, TRADE_STATUS_DEFAULT_LIMIT);

    let receipt = sdk.trades().status(request).await.expect("status");

    assert!(!receipt.found);
    assert_eq!(receipt.order_id.as_str(), "order-1");
    assert_eq!(receipt.source, SdkTradeStatusSource::LocalOnly);
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
    assert!(receipt.issues.is_empty());
}

#[tokio::test]
async fn order_status_query_uses_indexed_order_id_under_background_event_noise() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    ingest_status_noise_events(
        &store,
        STATUS_NOISE_NON_TRADE_EVENTS,
        STATUS_NOISE_TRADE_BACKGROUND_EVENTS,
    )
    .await;

    let request_event = signed_order_request_event("order-status-noise-active", 31_000);
    let request_event_id = request_event.id().clone();
    store
        .ingest_event(RadrootsEventIngest::new(
            signed_event_from_envelope(request_event),
            1_700_200_000_000,
        ))
        .await
        .expect("active order ingest");

    let status = sdk
        .trades()
        .status(status_request("order-status-noise-active"))
        .await
        .expect("status");
    let summary = store.status_summary().await.expect("status summary");

    assert_eq!(
        summary.total_events,
        STATUS_NOISE_NON_TRADE_EVENTS + STATUS_NOISE_TRADE_BACKGROUND_EVENTS + 1
    );
    assert_eq!(status.status, TradeStatusKind::Requested);
    assert_eq!(status.event_count, 1);
    assert_eq!(status.limit_applied, TRADE_STATUS_DEFAULT_LIMIT);
    assert_eq!(status.event_ids, vec![request_event_id]);
    assert_eq!(
        status.next_action,
        TradeStatusNextActionKind::AwaitSellerDecision
    );
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

#[test]
fn order_status_parse_accepts_root_specific_selectors() {
    let root_event_id = deterministic_event_id("order-status-root-selector");
    let request = TradeStatusRequest::parse(&format!("order-1@{}", root_event_id.as_str()))
        .expect("root-specific status request");

    assert_eq!(request.locator.order_id().as_str(), "order-1");
    assert_eq!(
        request
            .locator
            .root_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(root_event_id.as_str())
    );
    assert_eq!(
        TradeStatusRequest::locator_selector(&request.locator),
        format!("order-1@{}", root_event_id.as_str())
    );
}

#[test]
fn order_status_parse_rejects_malformed_root_selectors() {
    for selector in ["order-1@", "@aaaaaaaa", "order-1@aaaaaaaa@bbbbbbbb"] {
        let error = TradeStatusRequest::parse(selector).expect_err("malformed root selector");
        assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    }
}

#[tokio::test]
async fn order_status_contract_dtos_serialize_deterministically() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let request = status_request("order-1").with_limit(25);
    let request_json = serde_json::to_value(&request).expect("request json");
    assert_struct_serialize_error_paths(&request, 4);

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
            "limit": 25,
            "source": "local_only",
            "validation_trust_policy": {
                "validator_set": null,
                "validator_set_addr": null,
                "validator_set_event_id": null,
                "require_cryptographic_proof": false
            }
        })
    );

    let receipt = sdk.trades().status(request).await.expect("status");
    let receipt_json = serde_json::to_value(&receipt).expect("receipt json");

    assert_eq!(receipt_json["source"], "local_only");
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
    assert_eq!(receipt_json["validation_trust"], serde_json::Value::Null);
    assert_eq!(receipt_json["online_evidence"], serde_json::Value::Null);
    assert_eq!(receipt_json["next_action"], "no_local_order");
    assert_eq!(receipt_json["evidence"]["event_count"], 0);
    assert_eq!(receipt_json["evidence"]["limit_applied"], 25);
    assert_eq!(receipt_json["evidence"]["has_request"], false);
    assert_eq!(receipt_json["evidence"]["has_validation_receipt"], false);
    assert_eq!(receipt_json["eligibility"]["can_decide"], false);
    assert_eq!(receipt_json["eligibility"]["can_cancel"], false);

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
    let request_event_id = RadrootsEventId::parse(request_event.id_str()).expect("request id");
    let decision_event = signed_order_decision_event("order-1", &request_event_id, 21);

    for (event, observed_at_ms) in [
        (request_event.clone(), 2_000),
        (decision_event.clone(), 2_100),
    ] {
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event_from_envelope(event),
                observed_at_ms,
            ))
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
    assert_eq!(receipt.source, SdkTradeStatusSource::LocalOnly);
    assert_eq!(receipt.event_count, 2);
    assert_eq!(receipt.limit_applied, 1_000);
    assert_eq!(
        receipt
            .event_ids
            .iter()
            .map(RadrootsEventId::as_str)
            .collect::<Vec<_>>(),
        vec![request_event.id_str(), decision_event.id_str()]
    );
    assert_eq!(receipt.status, TradeStatusKind::AgreedPendingValidation);
    assert_eq!(
        receipt
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(request_event.id_str())
    );
    assert_eq!(
        receipt
            .decision_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(decision_event.id_str())
    );
    assert_eq!(
        receipt.last_event_id.as_ref().map(RadrootsEventId::as_str),
        Some(decision_event.id_str())
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
        TradeStatusNextActionKind::AwaitValidation
    );
    assert_eq!(receipt.evidence.event_count, 2);
    assert!(receipt.evidence.has_request);
    assert!(receipt.evidence.has_decision);
    assert!(receipt.evidence.has_agreement);
    assert!(!receipt.evidence.has_issues);
    assert!(!receipt.eligibility.can_decide);
    assert!(!receipt.eligibility.can_cancel);
}

#[tokio::test]
async fn order_status_reports_limited_local_results() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let request_event = signed_order_request_event("order-1", 25);
    let request_event_id = RadrootsEventId::parse(request_event.id_str()).expect("request id");
    let decision_event = signed_order_decision_event("order-1", &request_event_id, 26);

    for (event, observed_at_ms) in [(request_event.clone(), 2_500), (decision_event, 2_600)] {
        store
            .ingest_event(RadrootsEventIngest::new(
                signed_event_from_envelope(event),
                observed_at_ms,
            ))
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
        vec![request_event.id_str()]
    );
    assert_eq!(
        receipt
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(request_event.id_str())
    );
    assert!(receipt.decision_event_id.is_none());
    assert_eq!(
        receipt.last_event_id.as_ref().map(RadrootsEventId::as_str),
        Some(request_event.id_str())
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
            .ingest_event(RadrootsEventIngest::new(
                signed_event_from_envelope(event),
                observed_at_ms,
            ))
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
        vec![first_request_event.id_str(), second_request_event.id_str()]
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
        vec![first_request_event.id_str(), second_request_event.id_str()]
    );
    assert_eq!(
        receipt
            .ambiguity_candidates
            .iter()
            .map(|candidate| TradeStatusRequest::locator_selector(&candidate.locator))
            .collect::<Vec<_>>(),
        vec![
            format!("order-1@{}", first_request_event.id_str()),
            format!("order-1@{}", second_request_event.id_str())
        ]
    );

    let root_specific = sdk
        .trades()
        .status(
            TradeStatusRequest::parse(&format!("order-1@{}", second_request_event.id_str()))
                .expect("root status request"),
        )
        .await
        .expect("root-specific status");

    assert!(root_specific.found);
    assert_eq!(root_specific.status, TradeStatusKind::Requested);
    assert_eq!(
        root_specific
            .request_event_id
            .as_ref()
            .map(RadrootsEventId::as_str),
        Some(second_request_event.id_str())
    );
    assert!(root_specific.ambiguity_candidates.is_empty());
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
            .ingest_event(RadrootsEventIngest::new(
                signed_event_from_envelope(event),
                observed_at_ms,
            ))
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
            SatisfactionPolicy::NoWait,
            TradeEvidenceMode::LocalOnly,
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
        vec![first_request_event.id_str(), second_request_event.id_str()]
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
        .ingest_event(RadrootsEventIngest::new(
            signed_event_from_envelope(request_event.clone()),
            3_000,
        ))
        .await
        .expect("ingest");
    sqlx::query("UPDATE event_envelopes SET tags_json = '[' WHERE event_id = ?")
        .bind(request_event.id_str())
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
    assert!(!message.contains(request_event.sig_str()));
    assert!(!message.contains("\"tags\""));
    assert!(!message.contains("\"content\""));
}

#[tokio::test]
async fn trade_status_watch_emits_finite_refresh_window() {
    let (_tempdir, sdk, store) = directory_sdk_and_store().await;
    let order_id = "watch-finite-refresh-window";
    let request_event = signed_order_request_event(order_id, 820);
    store
        .ingest_event(RadrootsEventIngest::new(
            signed_event_from_envelope(request_event),
            1_700_400_000_000,
        ))
        .await
        .expect("request ingest");

    let mut watch = sdk
        .trades()
        .watch(
            TradeStatusWatchRequest::new(status_request(order_id))
                .with_capacity(2)
                .with_refresh_interval_ms(1)
                .with_refresh_limit(2),
        )
        .await
        .expect("watch");

    let first = watch.next().await.expect("first").expect("first update");
    let second = watch.next().await.expect("second").expect("second update");
    let closed = watch.next().await.expect("closed");
    let cancel = watch.cancel().await;

    assert_eq!(watch.capacity(), 2);
    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);
    assert_eq!(first.status.status, TradeStatusKind::Requested);
    assert_eq!(second.status.status, TradeStatusKind::Requested);
    assert!(closed.is_none());
    assert_eq!(cancel.state, TradeStatusWatchCancelState::AlreadyFinished);
}

#[tokio::test]
async fn trade_status_watch_backpressures_slow_consumer_with_bounded_buffer() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let mut watch = sdk
        .trades()
        .watch(
            TradeStatusWatchRequest::parse("watch-slow-consumer")
                .expect("watch request")
                .with_capacity(1)
                .with_refresh_interval_ms(1)
                .with_refresh_limit(50),
        )
        .await
        .expect("watch");

    tokio::time::sleep(Duration::from_millis(25)).await;
    let buffered_len = watch.buffered_len();
    let cancel = watch.cancel().await;
    let post_cancel = watch.next().await.expect("post cancel");

    assert_eq!(buffered_len, 1);
    assert_eq!(cancel.state, TradeStatusWatchCancelState::Cancelled);
    assert_eq!(cancel.buffered_updates_dropped, 1);
    assert!(post_cancel.is_none());
}

#[tokio::test]
async fn trade_status_watch_cancel_drains_buffer_and_closes_stream() {
    let (_tempdir, sdk, _store) = directory_sdk_and_store().await;
    let mut watch = sdk
        .trades()
        .watch(
            TradeStatusWatchRequest::parse("watch-cancel-close")
                .expect("watch request")
                .with_capacity(4)
                .with_refresh_interval_ms(1),
        )
        .await
        .expect("watch");
    let first = watch.next().await.expect("first").expect("first update");

    let cancel = watch.cancel().await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    let post_cancel = watch.next().await.expect("post cancel");

    assert_eq!(first.sequence, 1);
    assert_eq!(first.status.status, TradeStatusKind::Missing);
    assert_eq!(cancel.state, TradeStatusWatchCancelState::Cancelled);
    assert!(post_cancel.is_none());
}

#[tokio::test]
async fn trade_status_watch_closes_after_producer_error() {
    let sdk = RadrootsClient::builder().build().await.expect("sdk");
    let mut watch = sdk
        .trades()
        .watch(
            TradeStatusWatchRequest::new(
                TradeStatusRequest::parse("watch-producer-error")
                    .expect("status request")
                    .with_source(SdkTradeStatusSource::ResyncThenLocal),
            )
            .with_refresh_interval_ms(1)
            .with_refresh_limit(2)
            .with_capacity(2),
        )
        .await
        .expect("watch");

    let error = watch.next().await.expect_err("producer error");
    let closed = watch.next().await.expect("closed");

    assert!(!error.to_string().is_empty());
    assert!(closed.is_none());
}

#[tokio::test]
async fn trade_status_watch_rejects_unbounded_capacity() {
    let sdk = RadrootsClient::builder().build().await.expect("sdk");
    let result = sdk
        .trades()
        .watch(
            TradeStatusWatchRequest::parse("watch-invalid-capacity")
                .expect("watch request")
                .with_capacity(TRADE_STATUS_WATCH_MAX_CAPACITY + 1),
        )
        .await;
    let Err(error) = result else {
        panic!("expected capacity error");
    };

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
    assert!(error.to_string().contains("capacity"));
}

#[tokio::test]
#[ignore = "manual expensive release-gate lane for the 100k local-event status target"]
async fn manual_local_status_perf_gate_measures_100k_events() {
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
                signed_event_from_envelope(event),
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
    let cargo_target_dir =
        std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "<unset>".to_owned());
    println!(
        "manual local status performance gate p95_us={} target_us={} total_local_events={PERF_TOTAL_LOCAL_EVENTS} trade_relevant_events={PERF_TRADE_RELEVANT_EVENTS} active_trades={PERF_ACTIVE_TRADES} os={} arch={} cargo_target_dir={cargo_target_dir}",
        p95.as_micros(),
        PERF_STATUS_P95_TARGET.as_micros(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    assert!(
        p95 <= PERF_STATUS_P95_TARGET,
        "local status p95 {}us exceeded target {}us for {PERF_TOTAL_LOCAL_EVENTS} local events, {PERF_TRADE_RELEVANT_EVENTS} trade-relevant events, and {PERF_ACTIVE_TRADES} active trades",
        p95.as_micros(),
        PERF_STATUS_P95_TARGET.as_micros()
    );
}
