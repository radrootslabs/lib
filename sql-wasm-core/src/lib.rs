#[cfg(target_arch = "wasm32")]
use radroots_sql_core::error::SqlError;

#[cfg(target_arch = "wasm32")]
use radroots_sql_core::utils;

#[cfg(target_arch = "wasm32")]
use serde::de::DeserializeOwned;

#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
use js_sys::Uint8Array;
#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
use std::sync::OnceLock;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "embedded")]
mod embedded;
#[cfg(feature = "embedded")]
pub use embedded::EmbeddedSqlEngine;

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

#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
pub fn embedded_engine() -> Result<&'static EmbeddedSqlEngine, SqlError> {
    static ENGINE: OnceLock<EmbeddedSqlEngine> = OnceLock::new();
    if let Some(engine) = ENGINE.get() {
        return Ok(engine);
    }
    let engine = EmbeddedSqlEngine::new()?;
    let _ = ENGINE.set(engine);
    ENGINE.get().ok_or(SqlError::Internal)
}

#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = exec_sql)]
pub fn exec_sql(sql: &str, params_json: &str) -> JsValue {
    let outcome = match embedded_engine().and_then(|engine| engine.exec(sql, params_json)) {
        Ok(outcome) => outcome,
        Err(err) => return err_js(err),
    };
    let payload = serde_json::json!({
        "changes": outcome.changes,
        "last_insert_id": outcome.last_insert_id,
        "lastInsertRowid": outcome.last_insert_id,
    });
    match serde_wasm_bindgen::to_value(&payload) {
        Ok(value) => value,
        Err(err) => err_js(SqlError::SerializationError(err.to_string())),
    }
}

#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = query_sql)]
pub fn query_sql(sql: &str, params_json: &str) -> JsValue {
    let rows = match embedded_engine().and_then(|engine| engine.query_rows(sql, params_json)) {
        Ok(rows) => rows,
        Err(err) => return err_js(err),
    };
    match serde_wasm_bindgen::to_value(&rows) {
        Ok(value) => value,
        Err(err) => err_js(SqlError::SerializationError(err.to_string())),
    }
}

#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
pub fn export_bytes() -> JsValue {
    let bytes = match embedded_engine().and_then(|engine| engine.export_bytes()) {
        Ok(bytes) => bytes,
        Err(err) => return err_js(err),
    };
    let array = Uint8Array::from(bytes.as_slice());
    JsValue::from(array)
}

#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = begin_tx)]
pub fn begin_tx() {
    if let Ok(engine) = embedded_engine() {
        let _ = engine.begin_tx();
    }
}

#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = commit_tx)]
pub fn commit_tx() {
    if let Ok(engine) = embedded_engine() {
        let _ = engine.commit_tx();
    }
}

#[cfg(all(feature = "embedded", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = rollback_tx)]
pub fn rollback_tx() {
    if let Ok(engine) = embedded_engine() {
        let _ = engine.rollback_tx();
    }
}

#[cfg(all(feature = "bridge", not(feature = "embedded"), target_arch = "wasm32"))]
#[wasm_bindgen(js_name = exec_sql)]
pub fn exec_sql(sql: &str, params_json: &str) -> JsValue {
    radroots_sql_wasm_bridge::exec(sql, params_json)
}

#[cfg(all(feature = "bridge", not(feature = "embedded"), target_arch = "wasm32"))]
#[wasm_bindgen(js_name = query_sql)]
pub fn query_sql(sql: &str, params_json: &str) -> JsValue {
    radroots_sql_wasm_bridge::query(sql, params_json)
}

#[cfg(all(feature = "bridge", not(feature = "embedded"), target_arch = "wasm32"))]
pub fn export_bytes() -> JsValue {
    radroots_sql_wasm_bridge::export_bytes()
}

#[cfg(all(feature = "bridge", not(feature = "embedded"), target_arch = "wasm32"))]
#[wasm_bindgen(js_name = begin_tx)]
pub fn begin_tx() {
    radroots_sql_wasm_bridge::begin_tx()
}

#[cfg(all(feature = "bridge", not(feature = "embedded"), target_arch = "wasm32"))]
#[wasm_bindgen(js_name = commit_tx)]
pub fn commit_tx() {
    radroots_sql_wasm_bridge::commit_tx()
}

#[cfg(all(feature = "bridge", not(feature = "embedded"), target_arch = "wasm32"))]
#[wasm_bindgen(js_name = rollback_tx)]
pub fn rollback_tx() {
    radroots_sql_wasm_bridge::rollback_tx()
}
