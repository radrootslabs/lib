#![cfg(all(
    feature = "runtime",
    feature = "signer-adapters",
    feature = "local-signer",
    feature = "radrootsd-proxy"
))]

use radroots_authority::RadrootsActorContext;
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreUnit,
};
use radroots_events::{
    RadrootsEventPtr,
    contract::RadrootsActorRole,
    ids::RadrootsListingAddress,
    kinds::KIND_LISTING,
    order::{
        RadrootsOrderEconomicItem, RadrootsOrderEconomicLine, RadrootsOrderEconomics,
        RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
    },
};
use radroots_nostr::prelude::{RadrootsNostrKeys, RadrootsNostrSecretKey};
use radroots_sdk::{
    NostrRelayUrlPolicy, ProxyProfile, PublishMode, PushOutboxTargetOutcomeKind, RadrootsClient,
    RadrootsSdkLocalKeySigner, RadrootsSdkSignerProvider, RadrootsSdkTimestamp, SatisfactionPolicy,
    TargetPolicy, TargetSet, TradeMutationOutcome, TradeProposeRequest, TransportProfile,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread::JoinHandle,
};

const BUYER_SECRET_KEY_HEX: &str =
    "10c5304d6c9ae3a1a16f7860f1cc8f5e3a76225a2663b3a989a0d775919b7df5";
const BUYER_PUBLIC_KEY_HEX: &str =
    "585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df";
const SELLER_PUBLIC_KEY_HEX: &str =
    "e0266e3cfb0d2886f91c73f5f868f3b98273713e5fcd97c081663f5518a4b3af";
const RELAY: &str = "wss://relay.radroots.test";

struct RecordedTransportPublishRequest {
    body: String,
}

fn spawn_trade_transport_publish_server() -> (String, JoinHandle<RecordedTransportPublishRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind transport publish server");
    let endpoint = format!("http://{}/rpc", listener.local_addr().expect("addr"));
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let body = read_transport_publish_request_body(&mut stream);
        write_transport_publish_accept_response(&mut stream, body.as_str());
        RecordedTransportPublishRequest { body }
    });
    (endpoint, handle)
}

fn read_transport_publish_request_body(stream: &mut TcpStream) -> String {
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

fn write_transport_publish_accept_response(stream: &mut TcpStream, body: &str) {
    let body_json: serde_json::Value = serde_json::from_str(body).expect("body json");
    let event = &body_json["params"]["event"];
    let response_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": body_json["id"],
        "result": {
            "deduplicated": false,
            "job": {
                "job_id": "trade-product-publish-job",
                "status": "delivery_satisfied",
                "terminal": true,
                "delivery_satisfied": true,
                "event_id": event["id"],
                "pubkey": event["pubkey"],
                "event_kind": event["kind"],
                "target_policy": body_json["params"]["target_policy"],
                "delivery_policy": body_json["params"]["delivery_policy"],
                "target_count": 1,
                "acknowledged_count": 1,
                "retryable_count": 0,
                "terminal_count": 0,
                "requested_at_ms": 1700000000000i64,
                "completed_at_ms": 1700000000100i64,
                "targets": [{
                    "transport_kind": "nostr",
                    "endpoint_uri": RELAY,
                    "source": "request",
                    "attempted": true,
                    "outcome_kind": "accepted",
                    "message": "accepted"
                }]
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

fn buyer_actor() -> RadrootsActorContext {
    RadrootsActorContext::test(BUYER_PUBLIC_KEY_HEX, [RadrootsActorRole::Buyer]).expect("actor")
}

fn listing_address() -> RadrootsListingAddress {
    RadrootsListingAddress::parse(format!(
        "{KIND_LISTING}:{SELLER_PUBLIC_KEY_HEX}:AAAAAAAAAAAAAAAAAAAAAg"
    ))
    .expect("listing address")
}

fn listing_event_ptr() -> RadrootsEventPtr {
    RadrootsEventPtr {
        id: "6ccf12d1e56c21065d239bc3d46c0000cd000095d20000d9000073cd00009600".to_owned(),
        relays: Some(RELAY.to_owned()),
    }
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
        order_id: raw_order_id.parse().expect("order id"),
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

fn trade_propose_request(
    raw_order_id: &str,
    publish_mode: PublishMode,
    satisfaction_policy: SatisfactionPolicy,
) -> TradeProposeRequest {
    let order = order_request(raw_order_id);
    TradeProposeRequest::new(
        buyer_actor(),
        listing_event_ptr(),
        order.order_id,
        order.listing_addr,
        order.seller_pubkey,
        order.items,
        order.economics,
        explicit_trade_relays(),
        publish_mode,
        satisfaction_policy,
    )
}

fn explicit_trade_relays() -> TargetPolicy {
    TargetPolicy::explicit(
        TargetSet::nostr_relays([RELAY], NostrRelayUrlPolicy::Public).expect("target relays"),
    )
}

#[tokio::test]
async fn trade_product_propose_enqueue_and_publish_uses_ack_policy() {
    let (endpoint, handle) = spawn_trade_transport_publish_server();
    let tempdir = tempfile::tempdir().expect("tempdir");
    let secret_key = RadrootsNostrSecretKey::from_hex(BUYER_SECRET_KEY_HEX).expect("secret key");
    let signer_keys = RadrootsNostrKeys::new(secret_key);
    let sdk = RadrootsClient::builder()
        .directory_storage(tempdir.path().join("sdk"))
        .fixed_clock(RadrootsSdkTimestamp::from_unix_seconds(1_700_000_000))
        .signer_provider(RadrootsSdkSignerProvider::LocalKey(
            RadrootsSdkLocalKeySigner::new(signer_keys).expect("local signer"),
        ))
        .transport_profile(TransportProfile::proxy(ProxyProfile::new(endpoint)))
        .build()
        .await
        .expect("sdk");

    let outcome = sdk
        .trades()
        .buyer()
        .propose_trade(
            trade_propose_request(
                "trade-product-publish",
                PublishMode::EnqueueAndPublish,
                SatisfactionPolicy::AnyAccepted,
            )
            .try_with_idempotency_key("trade-product-publish")
            .expect("idempotency"),
        )
        .await
        .expect("publish proposal");
    let (receipt, publish) = match outcome {
        TradeMutationOutcome::Published { receipt, publish } => (receipt, publish),
        TradeMutationOutcome::DryRun { .. } => panic!("expected published outcome"),
        TradeMutationOutcome::Enqueued { .. } => panic!("expected published outcome"),
    };

    assert_eq!(receipt.order_id.as_str(), "trade-product-publish");
    assert_eq!(publish.attempted_events, 1);
    assert_eq!(publish.published_events, 1);
    assert_eq!(publish.events.len(), 1);
    assert_eq!(publish.events[0].outbox_event_id, receipt.outbox_event_id);
    assert_eq!(publish.events[0].quorum, 1);
    assert!(publish.events[0].quorum_met);
    assert_eq!(
        publish.events[0].targets[0].outcome_kind,
        PushOutboxTargetOutcomeKind::Accepted
    );

    let recorded = handle.join().expect("transport publish request");
    let body: serde_json::Value = serde_json::from_str(recorded.body.as_str()).expect("body");
    assert_eq!(body["method"], "transport.publish.event");
    assert_eq!(body["params"]["delivery_policy"]["mode"], "any");
    assert_eq!(body["params"]["target_policy"]["kind"], "explicit_targets");
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["endpoint_uri"],
        RELAY
    );
}
