use radroots_local_events::{
    BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT, BUYER_ORDER_REQUEST_DOCUMENT_KIND,
    BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND, buyer_order_request_local_work_record_id,
    validate_buyer_order_request_local_work_payload,
};
use serde_json::json;

#[test]
fn buyer_order_request_record_id_is_deterministic_for_app_orders() {
    assert_eq!(
        buyer_order_request_local_work_record_id(" order-1 ").expect("record id"),
        "app:local_work:order_request:order-1"
    );
}

#[test]
fn buyer_order_request_payload_requires_current_no_payment_order_document() {
    let payload = json!({
        "record_kind": BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND,
        "scope": "app",
        "currentness": {
            "current": true
        },
        "no_payment": {
            "payment_required": false,
            "settlement_deferred": true
        },
        "document": {
            "version": 1,
            "kind": BUYER_ORDER_REQUEST_DOCUMENT_KIND,
            "order": {
                "order_id": "ord_1",
                "listing_addr": "30402:seller:listing",
                "buyer_pubkey": "buyer",
                "seller_pubkey": "seller",
                "items": [
                    {
                        "bin_id": "bin-1",
                        "bin_count": 1
                    }
                ]
            },
            "buyer_actor": {
                "account_id": "buyer-account",
                "pubkey": "buyer",
                "source": BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT
            }
        }
    });

    validate_buyer_order_request_local_work_payload(&payload).expect("valid payload");
}

#[test]
fn buyer_order_request_payload_rejects_payment_required_documents() {
    let payload = json!({
        "record_kind": BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND,
        "currentness": {
            "current": true
        },
        "no_payment": {
            "payment_required": true,
            "settlement_deferred": true
        },
        "document": {
            "kind": BUYER_ORDER_REQUEST_DOCUMENT_KIND,
            "order": {
                "order_id": "ord_1"
            }
        }
    });

    let error =
        validate_buyer_order_request_local_work_payload(&payload).expect_err("invalid payload");

    assert!(error.to_string().contains("payment_required"));
}
