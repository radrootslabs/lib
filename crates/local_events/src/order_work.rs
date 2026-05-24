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
    validate_bool_field(payload, &["no_payment", "payment_required"], false)?;
    validate_bool_field(payload, &["no_payment", "settlement_deferred"], true)?;
    validate_string_field(payload, &["no_payment", "payment_state"], "not_applicable")?;

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
