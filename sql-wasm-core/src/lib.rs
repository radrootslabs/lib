#[cfg(target_arch = "wasm32")]
use radroots_sql_core::error::SqlError;

#[cfg(target_arch = "wasm32")]
use radroots_sql_core::utils;

#[cfg(target_arch = "wasm32")]
use serde::de::DeserializeOwned;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
pub fn parse_json<T: DeserializeOwned>(s: &str) -> Result<T, SqlError> {
    utils::parse_json(s)
}

#[cfg(target_arch = "wasm32")]
pub fn err_js(err: SqlError) -> JsValue {
    let value = err.to_json();
    match serde_wasm_bindgen::to_value(&value) {
        Ok(v) => v,
        Err(_) => JsValue::from_str(&err.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = exec_sql)]
pub fn exec_sql(sql: &str, params_json: &str) -> JsValue {
    radroots_sql_wasm_bridge::exec(sql, params_json)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = query_sql)]
pub fn query_sql(sql: &str, params_json: &str) -> JsValue {
    radroots_sql_wasm_bridge::query(sql, params_json)
}

#[cfg(target_arch = "wasm32")]
pub fn export_bytes() -> JsValue {
    radroots_sql_wasm_bridge::export_bytes()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = begin_tx)]
pub fn begin_tx() {
    radroots_sql_wasm_bridge::begin_tx()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = commit_tx)]
pub fn commit_tx() {
    radroots_sql_wasm_bridge::commit_tx()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = rollback_tx)]
pub fn rollback_tx() {
    radroots_sql_wasm_bridge::rollback_tx()
}
