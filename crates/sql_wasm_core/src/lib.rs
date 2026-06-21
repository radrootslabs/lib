#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg(feature = "bridge")]
use radroots_sql_core::error::SqlError;

#[cfg(feature = "bridge")]
use radroots_sql_core::utils;

#[cfg(feature = "bridge")]
use serde::de::DeserializeOwned;

#[cfg(feature = "bridge")]
use wasm_bindgen::JsValue;
#[cfg(all(feature = "bridge", target_arch = "wasm32"))]
use wasm_bindgen::prelude::*;

#[cfg(feature = "bridge")]
pub fn parse_json<T: DeserializeOwned>(s: &str) -> Result<T, SqlError> {
    utils::parse_json(s)
}

#[cfg(feature = "bridge")]
pub fn err_js(err: SqlError) -> JsValue {
    err_js_value(err)
}

#[cfg(all(feature = "bridge", target_arch = "wasm32"))]
fn err_js_value(err: SqlError) -> JsValue {
    match err_js_with_encoder(err, |err| {
        let value = err.to_json();
        serde_wasm_bindgen::to_value(&value).map_err(|_| ())
    }) {
        Ok(value) => value,
        Err(err) => JsValue::from_str(&err.to_string()),
    }
}

#[cfg(all(feature = "bridge", not(target_arch = "wasm32")))]
fn err_js_value(err: SqlError) -> JsValue {
    let _ = err.to_json();
    JsValue::NULL
}

#[cfg(all(feature = "bridge", target_arch = "wasm32"))]
fn err_js_with_encoder(
    err: SqlError,
    encode: impl FnOnce(&SqlError) -> Result<JsValue, ()>,
) -> Result<JsValue, SqlError> {
    encode(&err).map_err(|()| err)
}

#[cfg(feature = "bridge")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = exec_sql))]
pub fn exec_sql(sql: &str, params_json: &str) -> JsValue {
    radroots_sql_wasm_bridge::exec(sql, params_json)
}

#[cfg(feature = "bridge")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = query_sql))]
pub fn query_sql(sql: &str, params_json: &str) -> JsValue {
    radroots_sql_wasm_bridge::query(sql, params_json)
}

#[cfg(feature = "bridge")]
pub fn export_bytes() -> JsValue {
    radroots_sql_wasm_bridge::export_bytes()
}

#[cfg(feature = "bridge")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = begin_tx))]
pub fn begin_tx() {
    radroots_sql_wasm_bridge::begin_tx()
}

#[cfg(feature = "bridge")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = commit_tx))]
pub fn commit_tx() {
    radroots_sql_wasm_bridge::commit_tx()
}

#[cfg(feature = "bridge")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = rollback_tx))]
pub fn rollback_tx() {
    radroots_sql_wasm_bridge::rollback_tx()
}

#[cfg(all(test, feature = "bridge"))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::collections::BTreeMap;

    use radroots_sql_core::error::SqlError;

    use super::{
        begin_tx, commit_tx, err_js, exec_sql, export_bytes, parse_json, query_sql, rollback_tx,
    };

    #[test]
    fn parse_json_reports_valid_and_invalid_payloads() {
        let parsed: BTreeMap<String, u64> = parse_json(r#"{"count":2}"#).expect("parse json");
        assert_eq!(parsed.get("count"), Some(&2));
        assert!(matches!(
            parse_json::<BTreeMap<String, u64>>("{"),
            Err(SqlError::SerializationError(_))
        ));
    }

    #[test]
    fn err_js_accepts_sql_errors() {
        let _ = err_js(SqlError::Internal);
        let _ = err_js(SqlError::UnsupportedPlatform);
    }

    #[test]
    fn sql_entrypoints_delegate_to_bridge() {
        let _ = exec_sql("select 1", "[]");
        let _ = query_sql("select 2", "[2]");
        let _ = export_bytes();

        begin_tx();
        commit_tx();
        rollback_tx();
    }
}
