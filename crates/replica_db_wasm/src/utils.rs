use serde::Serialize;
use wasm_bindgen::prelude::*;

use radroots_sql_core::SqlError;

pub fn value_to_js<T>(value: T) -> Result<JsValue, JsValue>
where
    T: Serialize,
{
    let json = serde_json::to_string(&value)
        .map_err(|err| radroots_sql_wasm_core::err_js(SqlError::from(err)))?;
    Ok(JsValue::from_str(&json))
}
