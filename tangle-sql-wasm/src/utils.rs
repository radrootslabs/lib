use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

use radroots_sql_core::SqlError;

pub fn parse_optional_json<T>(json: &str) -> Result<Option<T>, serde_json::Error>
where
    T: DeserializeOwned,
{
    if json.trim().is_empty() {
        return Ok(None);
    }
    let value: Option<T> = serde_json::from_str(json)?;
    Ok(value)
}

pub fn value_to_js<T>(value: T) -> Result<JsValue, JsValue>
where
    T: Serialize,
{
    let json = serde_json::to_string(&value)
        .map_err(|err| radroots_sql_wasm_core::err_js(SqlError::from(err)))?;
    Ok(JsValue::from_str(&json))
}
