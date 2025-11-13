use radroots_sql_core::error::SqlError;
use serde::de::DeserializeOwned;
use serde_json::json;
use wasm_bindgen::JsValue;

pub fn parse_optional_json<T>(input: &str) -> Result<Option<T>, SqlError>
where
    T: DeserializeOwned,
{
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "null" {
        Ok(None)
    } else {
        let value = radroots_sql_wasm_core::parse_json::<T>(trimmed)?;
        Ok(Some(value))
    }
}

fn serialize_to_js_value<T>(value: &T) -> Result<JsValue, JsValue>
where
    T: serde::Serialize,
{
    serde_wasm_bindgen::to_value(value)
        .map_err(|e| radroots_sql_wasm_core::err_js(SqlError::SerializationError(e.to_string())))
}

pub fn outcome_to_js(outcome: radroots_sql_core::ExecOutcome) -> Result<JsValue, JsValue> {
    let payload = json!({
        "changes": outcome.changes,
        "last_insert_id": outcome.last_insert_id,
    });
    serialize_to_js_value(&payload)
}

pub fn value_to_js<T>(value: T) -> Result<JsValue, JsValue>
where
    T: serde::Serialize,
{
    serialize_to_js_value(&value)
}
