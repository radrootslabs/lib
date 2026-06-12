use serde_json::Value;

use crate::LocalEventsError;
use crate::models::validate_non_empty;

pub const BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND: &str = "buyer_order_request_v1";
pub const BUYER_ORDER_REQUEST_DOCUMENT_KIND: &str = "order_draft_v1";
pub const BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT: &str = "resolved_account";
pub const BUYER_ORDER_REQUEST_ACTOR_SOURCE_UNRESOLVED_APP: &str = "app_unresolved";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuyerOrderRequestSupportState {
    Supported,
    Unsupported,
}

impl BuyerOrderRequestSupportState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuyerOrderRequestLocalWorkValidation {
    pub order_id: String,
    pub support_state: BuyerOrderRequestSupportState,
    pub support_issues: Vec<String>,
}

pub fn buyer_order_request_local_work_record_id(
    order_id: &str,
) -> Result<String, LocalEventsError> {
    let order_id = order_id.trim();
    validate_non_empty("order_id", order_id)?;
    Ok(format!("app:local_work:order_request:{order_id}"))
}

pub fn validate_buyer_order_request_local_work_payload(
    payload: &Value,
) -> Result<BuyerOrderRequestLocalWorkValidation, LocalEventsError> {
    validate_string_field(
        payload,
        &["record_kind"],
        BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND,
    )?;
    validate_string_field(payload, &["scope"], "app")?;
    validate_string_field(
        payload,
        &["document", "kind"],
        BUYER_ORDER_REQUEST_DOCUMENT_KIND,
    )?;
    validate_bool_field(payload, &["currentness", "current"], true)?;
    validate_string_field(payload, &["currentness", "source"], "app_sqlite_order")?;
    validate_string_field(payload, &["payment_display", "state"], "not_recorded")?;
    validate_bool_field(
        payload,
        &["payment_display", "allows_payment_action"],
        false,
    )?;

    let order_id = validate_required_string(payload, &["document", "order", "order_id"])?;
    let currentness_order_id = validate_required_string(payload, &["currentness", "order_id"])?;
    if currentness_order_id != order_id {
        return Err(invalid_field(
            "currentness.order_id",
            "must match document.order.order_id",
        ));
    }
    validate_required_string(payload, &["currentness", "record_id"])?;
    validate_positive_i64(payload, &["currentness", "created_at_ms"])?;
    validate_required_string(payload, &["currentness", "order_updated_at"])?;

    let support_state = validate_support_status(payload)?;
    validate_exportability(payload, support_state)?;
    validate_order_identity(payload, support_state)?;
    validate_order_items(payload)?;
    validate_order_economics(payload)?;

    Ok(BuyerOrderRequestLocalWorkValidation {
        order_id: order_id.to_owned(),
        support_state,
        support_issues: support_issues(payload)?,
    })
}

pub fn validate_supported_buyer_order_request_local_work_payload(
    payload: &Value,
) -> Result<BuyerOrderRequestLocalWorkValidation, LocalEventsError> {
    let validation = validate_buyer_order_request_local_work_payload(payload)?;
    if validation.support_state != BuyerOrderRequestSupportState::Supported {
        return Err(invalid_field(
            "support_status.state",
            "must be supported for exportable app order work",
        ));
    }
    Ok(validation)
}

pub fn validate_unsupported_buyer_order_request_local_work_payload(
    payload: &Value,
) -> Result<BuyerOrderRequestLocalWorkValidation, LocalEventsError> {
    let validation = validate_buyer_order_request_local_work_payload(payload)?;
    if validation.support_state != BuyerOrderRequestSupportState::Unsupported {
        return Err(invalid_field(
            "support_status.state",
            "must be unsupported for unsupported app order work",
        ));
    }
    Ok(validation)
}

fn validate_support_status(
    payload: &Value,
) -> Result<BuyerOrderRequestSupportState, LocalEventsError> {
    let state = validate_required_string(payload, &["support_status", "state"])?;
    let issues = support_issues(payload)?;
    match state {
        "supported" => {
            if !issues.is_empty() {
                return Err(invalid_field(
                    "support_status.issues",
                    "must be empty when support_status.state is supported",
                ));
            }
            Ok(BuyerOrderRequestSupportState::Supported)
        }
        "unsupported" => {
            if issues.is_empty() {
                return Err(invalid_field(
                    "support_status.issues",
                    "must contain at least one issue when support_status.state is unsupported",
                ));
            }
            Ok(BuyerOrderRequestSupportState::Unsupported)
        }
        _ => Err(invalid_field(
            "support_status.state",
            "must be supported or unsupported",
        )),
    }
}

fn validate_exportability(
    payload: &Value,
    support_state: BuyerOrderRequestSupportState,
) -> Result<(), LocalEventsError> {
    let state = validate_required_string(payload, &["exportability", "state"])?;
    let buyer_actor_source =
        validate_required_string(payload, &["document", "buyer_actor", "source"])?;
    match state {
        "exportable" => {
            validate_string_field(
                payload,
                &["document", "buyer_actor", "source"],
                BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT,
            )?;
            validate_buyer_pubkey(payload)?;
        }
        "identity_unresolved" => {
            validate_required_string(payload, &["exportability", "reason"])?;
            validate_string_field(
                payload,
                &["document", "buyer_actor", "source"],
                BUYER_ORDER_REQUEST_ACTOR_SOURCE_UNRESOLVED_APP,
            )?;
            if support_state == BuyerOrderRequestSupportState::Supported {
                return Err(invalid_field(
                    "exportability.state",
                    "supported app order work must be exportable",
                ));
            }
        }
        _ => {
            return Err(invalid_field(
                "exportability.state",
                "must be exportable or identity_unresolved",
            ));
        }
    }
    if buyer_actor_source == BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT {
        validate_buyer_pubkey(payload)?;
    }
    Ok(())
}

fn validate_order_identity(
    payload: &Value,
    support_state: BuyerOrderRequestSupportState,
) -> Result<(), LocalEventsError> {
    validate_required_string(payload, &["document", "order", "listing_addr"])?;
    validate_required_string(payload, &["document", "order", "listing_event_id"])?;
    validate_required_string(payload, &["document", "order", "seller_pubkey"])?;
    if support_state == BuyerOrderRequestSupportState::Supported {
        validate_buyer_pubkey(payload)?;
    }
    Ok(())
}

fn validate_buyer_pubkey(payload: &Value) -> Result<(), LocalEventsError> {
    let order_buyer_pubkey =
        validate_required_string(payload, &["document", "order", "buyer_pubkey"])?;
    let actor_buyer_pubkey =
        validate_required_string(payload, &["document", "buyer_actor", "pubkey"])?;
    if order_buyer_pubkey != actor_buyer_pubkey {
        return Err(invalid_field(
            "document.buyer_actor.pubkey",
            "must match document.order.buyer_pubkey",
        ));
    }
    Ok(())
}

fn validate_order_items(payload: &Value) -> Result<(), LocalEventsError> {
    let items = required_array(payload, &["document", "order", "items"])?;
    if items.is_empty() {
        return Err(invalid_field(
            "document.order.items",
            "must contain at least one item",
        ));
    }
    for (index, item) in items.iter().enumerate() {
        validate_required_string(item, &["bin_id"]).map_err(|_| {
            invalid_field_at(
                format!("document.order.items[{index}].bin_id"),
                "is required",
            )
        })?;
        validate_positive_u64(item, &["bin_count"]).map_err(|_| {
            invalid_field_at(
                format!("document.order.items[{index}].bin_count"),
                "must be positive",
            )
        })?;
    }
    Ok(())
}

fn validate_order_economics(payload: &Value) -> Result<(), LocalEventsError> {
    let economics = value_at(payload, &["document", "order", "economics"]).ok_or_else(|| {
        invalid_field("document.order.economics", "is required for app order work")
    })?;
    if !economics.is_object() {
        return Err(invalid_field(
            "document.order.economics",
            "must be an object",
        ));
    }
    validate_string_field(economics, &["pricing_basis"], "listing_event")?;
    let currency = validate_required_string(economics, &["currency"])?;
    validate_currency("document.order.economics.currency", currency)?;
    let economics_items = required_array(economics, &["items"])?;
    let order_items = required_array(payload, &["document", "order", "items"])?;
    if economics_items.is_empty() {
        return Err(invalid_field(
            "document.order.economics.items",
            "must contain at least one item",
        ));
    }
    if economics_items.len() != order_items.len() {
        return Err(invalid_field(
            "document.order.economics.items",
            "must match document.order.items length",
        ));
    }
    for (index, item) in economics_items.iter().enumerate() {
        let order_item = &order_items[index];
        let economics_bin_id = validate_required_string(item, &["bin_id"]).map_err(|_| {
            invalid_field_at(
                format!("document.order.economics.items[{index}].bin_id"),
                "is required",
            )
        })?;
        let order_bin_id = validate_required_string(order_item, &["bin_id"])?;
        if economics_bin_id != order_bin_id {
            return Err(invalid_field_at(
                format!("document.order.economics.items[{index}].bin_id"),
                "must match document.order.items bin_id",
            ));
        }
        let economics_bin_count = validate_positive_u64(item, &["bin_count"]).map_err(|_| {
            invalid_field_at(
                format!("document.order.economics.items[{index}].bin_count"),
                "must be positive",
            )
        })?;
        let order_bin_count = validate_positive_u64(order_item, &["bin_count"])?;
        if economics_bin_count != order_bin_count {
            return Err(invalid_field_at(
                format!("document.order.economics.items[{index}].bin_count"),
                "must match document.order.items bin_count",
            ));
        }
        validate_required_string(item, &["quantity_amount"]).map_err(|_| {
            invalid_field_at(
                format!("document.order.economics.items[{index}].quantity_amount"),
                "is required",
            )
        })?;
        validate_required_string(item, &["quantity_unit"]).map_err(|_| {
            invalid_field_at(
                format!("document.order.economics.items[{index}].quantity_unit"),
                "is required",
            )
        })?;
        validate_required_string(item, &["unit_price_amount"]).map_err(|_| {
            invalid_field_at(
                format!("document.order.economics.items[{index}].unit_price_amount"),
                "is required",
            )
        })?;
        let unit_price_currency = validate_required_string(item, &["unit_price_currency"])?;
        if unit_price_currency != currency {
            return Err(invalid_field_at(
                format!("document.order.economics.items[{index}].unit_price_currency"),
                "must match document.order.economics.currency",
            ));
        }
        validate_money(item, &["line_subtotal"], currency)?;
    }
    validate_money(economics, &["subtotal"], currency)?;
    validate_money(economics, &["discount_total"], currency)?;
    validate_money(economics, &["adjustment_total"], currency)?;
    validate_money(economics, &["total"], currency)?;
    Ok(())
}

fn validate_money(payload: &Value, path: &[&str], currency: &str) -> Result<(), LocalEventsError> {
    let Some(money) = value_at(payload, path) else {
        return Err(missing_field(path));
    };
    validate_required_string(money, &["amount"])?;
    let money_currency = validate_required_string(money, &["currency"])?;
    if money_currency != currency {
        return Err(invalid_field(
            &format!("{}.currency", path.join(".")),
            "must match currency",
        ));
    }
    Ok(())
}

fn validate_string_field(
    payload: &Value,
    path: &[&str],
    expected: &str,
) -> Result<(), LocalEventsError> {
    let Some(value) = value_at(payload, path).and_then(Value::as_str) else {
        return Err(missing_field(path));
    };
    if value != expected {
        return Err(invalid_field(
            &path.join("."),
            &format!("must be `{expected}`"),
        ));
    }
    Ok(())
}

fn validate_required_string<'a>(
    payload: &'a Value,
    path: &[&str],
) -> Result<&'a str, LocalEventsError> {
    let Some(value) = value_at(payload, path).and_then(Value::as_str) else {
        return Err(missing_field(path));
    };
    validate_non_empty(&path.join("."), value)?;
    Ok(value.trim())
}

fn validate_bool_field(
    payload: &Value,
    path: &[&str],
    expected: bool,
) -> Result<(), LocalEventsError> {
    let Some(value) = value_at(payload, path).and_then(Value::as_bool) else {
        return Err(missing_field(path));
    };
    if value != expected {
        return Err(invalid_field(
            &path.join("."),
            &format!("must be `{expected}`"),
        ));
    }
    Ok(())
}

fn validate_positive_i64(payload: &Value, path: &[&str]) -> Result<(), LocalEventsError> {
    match value_at(payload, path).and_then(Value::as_i64) {
        Some(value) if value > 0 => Ok(()),
        _ => Err(invalid_field(&path.join("."), "must be positive")),
    }
}

fn validate_positive_u64(payload: &Value, path: &[&str]) -> Result<u64, LocalEventsError> {
    match value_at(payload, path).and_then(Value::as_u64) {
        Some(value) if value > 0 => Ok(value),
        _ => Err(invalid_field(&path.join("."), "must be positive")),
    }
}

fn validate_currency(field: &str, value: &str) -> Result<(), LocalEventsError> {
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(invalid_field(
            field,
            "must be an uppercase ISO currency code",
        ));
    }
    Ok(())
}

fn required_array<'a>(
    payload: &'a Value,
    path: &[&str],
) -> Result<&'a Vec<Value>, LocalEventsError> {
    let Some(value) = value_at(payload, path).and_then(Value::as_array) else {
        return Err(missing_field(path));
    };
    Ok(value)
}

fn support_issues(payload: &Value) -> Result<Vec<String>, LocalEventsError> {
    let issues = required_array(payload, &["support_status", "issues"])?;
    let mut parsed = Vec::with_capacity(issues.len());
    for (index, issue) in issues.iter().enumerate() {
        let Some(issue) = issue.as_str() else {
            return Err(invalid_field_at(
                format!("support_status.issues[{index}]"),
                "must be a string",
            ));
        };
        validate_non_empty("support_status.issues", issue)?;
        parsed.push(issue.trim().to_owned());
    }
    Ok(parsed)
}

fn value_at<'a>(payload: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = payload;
    for part in path {
        current = current.get(*part)?;
    }
    Some(current)
}

fn missing_field(path: &[&str]) -> LocalEventsError {
    invalid_field(&path.join("."), "is required")
}

fn invalid_field(field: &str, requirement: &str) -> LocalEventsError {
    LocalEventsError::InvalidRecord(format!("local order field `{field}` {requirement}"))
}

fn invalid_field_at(field: String, requirement: &str) -> LocalEventsError {
    LocalEventsError::InvalidRecord(format!("local order field `{field}` {requirement}"))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn support_state_labels_and_record_id_validation_are_stable() {
        assert_eq!(
            BuyerOrderRequestSupportState::Supported.as_str(),
            "supported"
        );
        assert_eq!(
            BuyerOrderRequestSupportState::Unsupported.as_str(),
            "unsupported"
        );
        assert_eq!(
            buyer_order_request_local_work_record_id(" ord-a ").expect("record id"),
            "app:local_work:order_request:ord-a"
        );
        assert_error_contains(
            buyer_order_request_local_work_record_id(" "),
            "order_id must not be empty",
        );
    }

    #[test]
    fn private_validation_helpers_cover_successful_payload() {
        let payload = supported_payload();

        assert_eq!(
            validate_support_status(&payload).expect("support status"),
            BuyerOrderRequestSupportState::Supported
        );
        validate_exportability(&payload, BuyerOrderRequestSupportState::Supported)
            .expect("exportability");
        validate_order_identity(&payload, BuyerOrderRequestSupportState::Supported)
            .expect("identity");
        validate_order_items(&payload).expect("items");
        validate_order_economics(&payload).expect("economics");
        assert_eq!(
            validate_required_string(&payload, &["document", "order", "order_id"])
                .expect("order id"),
            "ord_1"
        );
        validate_bool_field(&payload, &["currentness", "current"], true).expect("bool");
        assert_eq!(
            support_issues(&payload).expect("support issues"),
            Vec::<String>::new()
        );
        assert!(value_at(&payload, &["document", "order"]).is_some());
    }

    #[test]
    fn payload_validation_rejects_top_level_contract_drift() {
        let mut wrong_kind = supported_payload();
        wrong_kind["record_kind"] = json!("other");
        assert_invalid(wrong_kind, "record_kind");

        let mut missing_scope = supported_payload();
        missing_scope["scope"] = Value::Null;
        assert_invalid(missing_scope, "scope");

        let mut wrong_document_kind = supported_payload();
        wrong_document_kind["document"]["kind"] = json!("other");
        assert_invalid(wrong_document_kind, "document.kind");

        let mut wrong_currentness_source = supported_payload();
        wrong_currentness_source["currentness"]["source"] = json!("other");
        assert_invalid(wrong_currentness_source, "currentness.source");

        let mut missing_order_updated = supported_payload();
        missing_order_updated["currentness"]["order_updated_at"] = Value::Null;
        assert_invalid(missing_order_updated, "order_updated_at");

        let mut bad_created_at = supported_payload();
        bad_created_at["currentness"]["created_at_ms"] = json!(0);
        assert_invalid(bad_created_at, "created_at_ms");

        let mut wrong_payment_state = supported_payload();
        wrong_payment_state["payment_display"]["state"] = json!("recorded");
        assert_invalid(wrong_payment_state, "payment_display.state");
    }

    #[test]
    fn support_and_exportability_rejections_cover_private_branches() {
        let mut invalid_state = supported_payload();
        invalid_state["support_status"]["state"] = json!("partial");
        assert_invalid(invalid_state, "support_status.state");

        let mut issue_not_string = supported_payload();
        issue_not_string["support_status"] = json!({
            "state": "unsupported",
            "issues": [42]
        });
        assert_invalid(issue_not_string, "support_status.issues[0]");

        let mut issue_empty = supported_payload();
        issue_empty["support_status"] = json!({
            "state": "unsupported",
            "issues": [" "]
        });
        assert_invalid(issue_empty, "support_status.issues");

        let mut supported_but_unresolved = unsupported_payload();
        supported_but_unresolved["support_status"] = json!({
            "state": "supported",
            "issues": []
        });
        assert_invalid(supported_but_unresolved, "exportability.state");

        let mut unknown_exportability = supported_payload();
        unknown_exportability["exportability"]["state"] = json!("queued");
        assert_invalid(unknown_exportability, "exportability.state");

        let mut missing_reason = unsupported_payload();
        missing_reason["exportability"]["reason"] = Value::Null;
        assert_invalid(missing_reason, "exportability.reason");

        let mut wrong_actor_source = unsupported_payload();
        wrong_actor_source["document"]["buyer_actor"]["source"] =
            json!(BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT);
        assert_invalid(wrong_actor_source, "buyer_actor.source");

        let mut mismatched_buyer = supported_payload();
        mismatched_buyer["document"]["buyer_actor"]["pubkey"] = json!("other");
        assert_invalid(mismatched_buyer, "buyer_actor.pubkey");

        let supported_error =
            validate_unsupported_buyer_order_request_local_work_payload(&supported_payload())
                .expect_err("supported payload is not unsupported");
        assert!(supported_error.to_string().contains("support_status.state"));
    }

    #[test]
    fn item_and_economics_rejections_cover_private_branches() {
        let mut economics_not_object = supported_payload();
        economics_not_object["document"]["order"]["economics"] = json!("bad");
        assert_invalid(economics_not_object, "economics");

        let mut bad_pricing_basis = supported_payload();
        bad_pricing_basis["document"]["order"]["economics"]["pricing_basis"] = json!("manual");
        assert_invalid(bad_pricing_basis, "pricing_basis");

        let mut bad_currency = supported_payload();
        bad_currency["document"]["order"]["economics"]["currency"] = json!("usd");
        assert_invalid(bad_currency, "currency");

        let mut economics_items_missing = supported_payload();
        economics_items_missing["document"]["order"]["economics"]["items"] = Value::Null;
        assert_invalid(economics_items_missing, "items");

        let mut economics_items_short = supported_payload();
        economics_items_short["document"]["order"]["economics"]["items"] = json!([]);
        assert_invalid(economics_items_short, "economics.items");

        let mut economics_bin_missing = supported_payload();
        economics_bin_missing["document"]["order"]["economics"]["items"][0]["bin_id"] = Value::Null;
        assert_invalid(economics_bin_missing, "economics.items[0].bin_id");

        let mut economics_count_bad = supported_payload();
        economics_count_bad["document"]["order"]["economics"]["items"][0]["bin_count"] = json!(0);
        assert_invalid(economics_count_bad, "economics.items[0].bin_count");

        let mut order_count_mismatch = supported_payload();
        order_count_mismatch["document"]["order"]["economics"]["items"][0]["bin_count"] = json!(3);
        assert_invalid(order_count_mismatch, "economics.items[0].bin_count");

        let mut quantity_amount_missing = supported_payload();
        quantity_amount_missing["document"]["order"]["economics"]["items"][0]["quantity_amount"] =
            Value::Null;
        assert_invalid(quantity_amount_missing, "quantity_amount");

        let mut quantity_unit_missing = supported_payload();
        quantity_unit_missing["document"]["order"]["economics"]["items"][0]["quantity_unit"] =
            Value::Null;
        assert_invalid(quantity_unit_missing, "quantity_unit");

        let mut unit_price_amount_missing = supported_payload();
        unit_price_amount_missing["document"]["order"]["economics"]["items"][0]["unit_price_amount"] =
            Value::Null;
        assert_invalid(unit_price_amount_missing, "unit_price_amount");

        let mut line_subtotal_missing = supported_payload();
        line_subtotal_missing["document"]["order"]["economics"]["items"][0]["line_subtotal"] =
            Value::Null;
        assert_invalid(line_subtotal_missing, "amount");

        let mut line_subtotal_currency = supported_payload();
        line_subtotal_currency["document"]["order"]["economics"]["items"][0]["line_subtotal"]["currency"] =
            json!("CAD");
        assert_invalid(line_subtotal_currency, "line_subtotal.currency");

        let mut subtotal_currency = supported_payload();
        subtotal_currency["document"]["order"]["economics"]["subtotal"]["currency"] = json!("CAD");
        assert_invalid(subtotal_currency, "subtotal.currency");

        let mut order_item_missing = supported_payload();
        order_item_missing["document"]["order"]["items"] = Value::Null;
        assert_invalid(order_item_missing, "document.order.items");
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
                }
            }
        })
    }

    fn unsupported_payload() -> Value {
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
        payload
    }

    fn assert_invalid(payload: Value, expected: &str) {
        assert_error_contains(
            validate_buyer_order_request_local_work_payload(&payload),
            expected,
        );
    }

    fn assert_error_contains<T: std::fmt::Debug>(
        result: Result<T, LocalEventsError>,
        expected: &str,
    ) {
        let error = result.expect_err("expected validation error");
        assert!(
            error.to_string().contains(expected),
            "expected error to contain {expected}, got {error}"
        );
    }
}
