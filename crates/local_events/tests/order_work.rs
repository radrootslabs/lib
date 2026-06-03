use radroots_local_events::{
    BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT,
    BUYER_ORDER_REQUEST_ACTOR_SOURCE_UNRESOLVED_APP, BUYER_ORDER_REQUEST_DOCUMENT_KIND,
    BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND, BuyerOrderRequestSupportState,
    buyer_order_request_local_work_record_id, validate_buyer_order_request_local_work_payload,
    validate_supported_buyer_order_request_local_work_payload,
    validate_unsupported_buyer_order_request_local_work_payload,
};
use serde_json::{Value, json};

#[test]
fn buyer_order_request_record_id_is_deterministic_for_app_orders() {
    assert_eq!(
        buyer_order_request_local_work_record_id(" order-1 ").expect("record id"),
        "app:local_work:order_request:order-1"
    );
}

#[test]
fn buyer_order_request_payload_accepts_supported_exportable_work() {
    let payload = supported_payload();

    let validation =
        validate_buyer_order_request_local_work_payload(&payload).expect("valid payload");
    let supported = validate_supported_buyer_order_request_local_work_payload(&payload)
        .expect("supported payload");

    assert_eq!(validation.order_id, "ord_1");
    assert_eq!(
        validation.support_state,
        BuyerOrderRequestSupportState::Supported
    );
    assert!(validation.support_issues.is_empty());
    assert_eq!(supported, validation);
}

#[test]
fn buyer_order_request_payload_accepts_explicit_unsupported_work() {
    let mut payload = supported_payload();
    payload["exportability"] = json!({
        "state": "identity_unresolved",
        "reason": "canonical_hex_pubkey_required"
    });
    payload["support_status"] = json!({
        "state": "unsupported",
        "issues": ["buyer_pubkey_required"]
    });
    payload["document"]["order"]["buyer_pubkey"] = json!("");
    payload["document"]["buyer_actor"]["pubkey"] = json!("");
    payload["document"]["buyer_actor"]["source"] =
        json!(BUYER_ORDER_REQUEST_ACTOR_SOURCE_UNRESOLVED_APP);

    let validation =
        validate_buyer_order_request_local_work_payload(&payload).expect("valid payload");
    let unsupported = validate_unsupported_buyer_order_request_local_work_payload(&payload)
        .expect("unsupported payload");
    let supported_error = validate_supported_buyer_order_request_local_work_payload(&payload)
        .expect_err("unsupported payload should not validate as supported");

    assert_eq!(
        validation.support_state,
        BuyerOrderRequestSupportState::Unsupported
    );
    assert_eq!(validation.support_issues, vec!["buyer_pubkey_required"]);
    assert_eq!(unsupported, validation);
    assert!(supported_error.to_string().contains("support_status.state"));
}

#[test]
fn buyer_order_request_payload_rejects_payment_actions() {
    let mut payload = supported_payload();
    payload["payment_display"]["allows_payment_action"] = json!(true);

    assert_invalid(payload, "allows_payment_action");
}

#[test]
fn buyer_order_request_payload_rejects_missing_identity() {
    for (path, expected) in [
        (vec!["document", "order", "listing_addr"], "listing_addr"),
        (
            vec!["document", "order", "listing_event_id"],
            "listing_event_id",
        ),
        (vec!["document", "order", "seller_pubkey"], "seller_pubkey"),
        (vec!["document", "order", "buyer_pubkey"], "buyer_pubkey"),
    ] {
        let mut payload = supported_payload();
        set_path(&mut payload, &path, json!(""));

        assert_invalid(payload, expected);
    }
}

#[test]
fn buyer_order_request_payload_rejects_missing_items() {
    let mut payload = supported_payload();
    payload["document"]["order"]["items"] = json!([]);

    assert_invalid(payload, "items");
}

#[test]
fn buyer_order_request_payload_rejects_invalid_item_identity() {
    let mut missing_bin = supported_payload();
    missing_bin["document"]["order"]["items"][0]["bin_id"] = json!("");
    assert_invalid(missing_bin, "items[0].bin_id");

    let mut zero_count = supported_payload();
    zero_count["document"]["order"]["items"][0]["bin_count"] = json!(0);
    assert_invalid(zero_count, "items[0].bin_count");
}

#[test]
fn buyer_order_request_payload_rejects_invalid_economics() {
    let mut missing_economics = supported_payload();
    missing_economics["document"]["order"]["economics"] = Value::Null;
    assert_invalid(missing_economics, "economics");

    let mut mismatched_currency = supported_payload();
    mismatched_currency["document"]["order"]["economics"]["items"][0]["unit_price_currency"] =
        json!("CAD");
    assert_invalid(mismatched_currency, "unit_price_currency");

    let mut mismatched_items = supported_payload();
    mismatched_items["document"]["order"]["economics"]["items"] = json!([]);
    assert_invalid(mismatched_items, "economics.items");

    let mut mismatched_bin = supported_payload();
    mismatched_bin["document"]["order"]["economics"]["items"][0]["bin_id"] = json!("other-bin");
    assert_invalid(mismatched_bin, "economics.items[0].bin_id");
}

#[test]
fn buyer_order_request_payload_rejects_stale_or_conflicting_currentness() {
    let mut stale = supported_payload();
    stale["currentness"]["current"] = json!(false);
    assert_invalid(stale, "currentness.current");

    let mut wrong_order = supported_payload();
    wrong_order["currentness"]["order_id"] = json!("ord_other");
    assert_invalid(wrong_order, "currentness.order_id");
}

#[test]
fn buyer_order_request_payload_rejects_malformed_support_status() {
    let mut supported_with_issue = supported_payload();
    supported_with_issue["support_status"]["issues"] = json!(["unit_price_required"]);
    assert_invalid(supported_with_issue, "support_status.issues");

    let mut unsupported_without_issue = supported_payload();
    unsupported_without_issue["support_status"] = json!({
        "state": "unsupported",
        "issues": []
    });
    assert_invalid(unsupported_without_issue, "support_status.issues");
}

fn supported_payload() -> Value {
    json!({
        "record_kind": BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND,
        "scope": "app",
        "exportability": {
            "state": "exportable"
        },
        "support_status": {
            "state": "supported",
            "issues": []
        },
        "currentness": {
            "current": true,
            "source": "app_sqlite_order",
            "record_id": "app:local_work:order_request:ord_1",
            "order_id": "ord_1",
            "order_updated_at": "2026-05-24T12:00:00Z",
            "created_at_ms": 1777777777000_i64
        },
        "payment_display": {
            "state": "not_recorded",
            "allows_payment_action": false
        },
        "document": {
            "version": 1,
            "kind": BUYER_ORDER_REQUEST_DOCUMENT_KIND,
            "order": {
                "order_id": "ord_1",
                "listing_addr": "30402:seller_pubkey:listing_key",
                "listing_event_id": "event-listing-1",
                "buyer_pubkey": "buyer_pubkey",
                "seller_pubkey": "seller_pubkey",
                "items": [
                    {
                        "bin_id": "dozen-eggs",
                        "bin_count": 2
                    }
                ],
                "economics": {
                    "quote_id": "app-order:ord_1",
                    "quote_version": 1,
                    "pricing_basis": "listing_event",
                    "currency": "USD",
                    "items": [
                        {
                            "bin_id": "dozen-eggs",
                            "bin_count": 2,
                            "quantity_amount": "1",
                            "quantity_unit": "dozen",
                            "unit_price_amount": "8.00",
                            "unit_price_currency": "USD",
                            "line_subtotal": {
                                "amount": "16.00",
                                "currency": "USD"
                            }
                        }
                    ],
                    "discounts": [],
                    "adjustments": [],
                    "subtotal": {
                        "amount": "16.00",
                        "currency": "USD"
                    },
                    "discount_total": {
                        "amount": "0",
                        "currency": "USD"
                    },
                    "adjustment_total": {
                        "amount": "0",
                        "currency": "USD"
                    },
                    "total": {
                        "amount": "16.00",
                        "currency": "USD"
                    }
                }
            },
            "buyer_actor": {
                "account_id": "buyer-account",
                "pubkey": "buyer_pubkey",
                "source": BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT
            },
            "listing_lookup": "30402:seller_pubkey:listing_key"
        }
    })
}

fn assert_invalid(payload: Value, expected: &str) {
    let error =
        validate_buyer_order_request_local_work_payload(&payload).expect_err("invalid payload");
    assert!(
        error.to_string().contains(expected),
        "expected error to contain {expected}, got {error}"
    );
}

fn set_path(payload: &mut Value, path: &[&str], value: Value) {
    let mut current = payload;
    for segment in &path[..path.len() - 1] {
        current = current.get_mut(*segment).expect("path segment");
    }
    current[path[path.len() - 1]] = value;
}
