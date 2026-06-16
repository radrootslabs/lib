#![cfg(feature = "runtime")]

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
};
use radroots_event_store::{RadrootsEventIngest, RadrootsEventStore};
use radroots_events::kinds::KIND_LISTING;
use radroots_events::{
    RadrootsNostrEvent,
    ids::{RadrootsEventId, RadrootsOrderId},
};
use radroots_nostr::prelude::{
    RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp, radroots_event_from_nostr,
    radroots_nostr_build_event,
};
use radroots_sdk::protocol::RadrootsNostrEventPtr;
use radroots_sdk::protocol::WireEventParts;
use radroots_sdk::protocol::order::{
    RadrootsListingAddress, RadrootsOrderDecision, RadrootsOrderDecisionOutcome,
    RadrootsOrderEconomicItem, RadrootsOrderEconomicLine, RadrootsOrderEconomics,
    RadrootsOrderInventoryCommitment, RadrootsOrderItem, RadrootsOrderPricingBasis,
    RadrootsOrderRequest,
};
use radroots_sdk::{
    ORDER_STATUS_DEFAULT_LIMIT, ORDER_STATUS_MAX_LIMIT, OrderPaymentStateKind,
    OrderSettlementStateKind, OrderStatusKind, OrderStatusRequest, RadrootsSdk, RadrootsSdkError,
    RadrootsSdkTimestamp, SdkOrderStatusIssueKind, SdkOrderStatusSource,
};

const BUYER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const BUYER_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const SELLER_SECRET_KEY_HEX: &str =
    "59392e9068f66431b12f70218fb61281cb6b433d7f27c55d61f1a63fe1a96ff8";
const SELLER_PUBLIC_KEY_HEX: &str =
    "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";

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

fn listing_address() -> RadrootsListingAddress {
    RadrootsListingAddress::parse(format!(
        "{KIND_LISTING}:{SELLER_PUBLIC_KEY_HEX}:AAAAAAAAAAAAAAAAAAAAAg"
    ))
    .expect("listing address")
}

fn listing_event_ptr() -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: deterministic_event_id("listing-event").into_string(),
        relays: Some("wss://relay.radroots.test".to_owned()),
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

    assert!(matches!(zero, RadrootsSdkError::InvalidRequest { .. }));
    assert!(matches!(too_large, RadrootsSdkError::InvalidRequest { .. }));
}

#[test]
fn order_status_parse_rejects_invalid_order_ids() {
    let error = OrderStatusRequest::parse("bad order id").expect_err("invalid order id");

    assert!(matches!(error, RadrootsSdkError::InvalidRequest { .. }));
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
