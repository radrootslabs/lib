use serde_json::Value;

use crate::LocalEventsError;
use crate::models::validate_non_empty;

pub const BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND: &str = "buyer_order_request_v1";
pub const BUYER_ORDER_REQUEST_DOCUMENT_KIND: &str = "order_draft_v1";
pub const BUYER_ORDER_REQUEST_ACTOR_SOURCE_RESOLVED_ACCOUNT: &str = "resolved_account";
pub const BUYER_ORDER_REQUEST_ACTOR_SOURCE_UNRESOLVED_APP: &str = "app_unresolved";

pub fn buyer_order_request_local_work_record_id(
    order_id: &str,
) -> Result<String, LocalEventsError> {
    let order_id = order_id.trim();
    validate_non_empty("order_id", order_id)?;
    Ok(format!("app:local_work:order_request:{order_id}"))
}

pub fn validate_buyer_order_request_local_work_payload(
    payload: &Value,
) -> Result<(), LocalEventsError> {
    validate_string_field(
        payload,
        &["record_kind"],
        BUYER_ORDER_REQUEST_LOCAL_WORK_RECORD_KIND,
    )?;
    validate_string_field(
        payload,
        &["document", "kind"],
        BUYER_ORDER_REQUEST_DOCUMENT_KIND,
    )?;
    validate_required_string(payload, &["document", "order", "order_id"], "order_id")?;
    validate_bool_field(payload, &["currentness", "current"], true)?;
    validate_bool_field(payload, &["no_payment", "payment_required"], false)?;
    validate_bool_field(payload, &["no_payment", "settlement_deferred"], true)?;
    Ok(())
}

fn validate_string_field(
    payload: &Value,
    path: &[&str],
    expected: &str,
) -> Result<(), LocalEventsError> {
    let Some(value) = value_at(payload, path).and_then(Value::as_str) else {
        return Err(LocalEventsError::InvalidRecord(format!(
            "missing required local order field `{}`",
            path.join(".")
        )));
    };
    if value != expected {
        return Err(LocalEventsError::InvalidRecord(format!(
            "local order field `{}` must be `{expected}`",
            path.join(".")
        )));
    }
    Ok(())
}

fn validate_required_string(
    payload: &Value,
    path: &[&str],
    field: &str,
) -> Result<(), LocalEventsError> {
    let Some(value) = value_at(payload, path).and_then(Value::as_str) else {
        return Err(LocalEventsError::InvalidRecord(format!(
            "missing required local order field `{}`",
            path.join(".")
        )));
    };
    validate_non_empty(field, value)
}

fn validate_bool_field(
    payload: &Value,
    path: &[&str],
    expected: bool,
) -> Result<(), LocalEventsError> {
    let Some(value) = value_at(payload, path).and_then(Value::as_bool) else {
        return Err(LocalEventsError::InvalidRecord(format!(
            "missing required local order field `{}`",
            path.join(".")
        )));
    };
    if value != expected {
        return Err(LocalEventsError::InvalidRecord(format!(
            "local order field `{}` must be `{expected}`",
            path.join(".")
        )));
    }
    Ok(())
}

fn value_at<'a>(payload: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = payload;
    for part in path {
        current = current.get(*part)?;
    }
    Some(current)
}
