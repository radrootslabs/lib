#![forbid(unsafe_code)]

use radroots_sql_core::{ExecOutcome, SqlError, SqlExecutor, utils};
use serde::de::DeserializeOwned;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen::JsValue;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = __radroots_sql_wasm_exec)]
    fn js_exec(sql: &str, params_json: &str) -> JsValue;

    #[wasm_bindgen(js_name = __radroots_sql_wasm_query)]
    fn js_query(sql: &str, params_json: &str) -> JsValue;

    #[wasm_bindgen(js_name = __radroots_sql_wasm_export_bytes)]
    fn js_export_bytes() -> JsValue;
}

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Mutex, OnceLock};

#[cfg(not(target_arch = "wasm32"))]
type RecordedCall = (String, String);

#[cfg(not(target_arch = "wasm32"))]
fn exec_calls() -> &'static Mutex<Vec<RecordedCall>> {
    static EXEC_CALLS: OnceLock<Mutex<Vec<RecordedCall>>> = OnceLock::new();
    EXEC_CALLS.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(not(target_arch = "wasm32"))]
fn query_calls() -> &'static Mutex<Vec<RecordedCall>> {
    static QUERY_CALLS: OnceLock<Mutex<Vec<RecordedCall>>> = OnceLock::new();
    QUERY_CALLS.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(not(target_arch = "wasm32"))]
fn export_calls() -> &'static Mutex<u64> {
    static EXPORT_CALLS: OnceLock<Mutex<u64>> = OnceLock::new();
    EXPORT_CALLS.get_or_init(|| Mutex::new(0))
}

#[cfg(not(target_arch = "wasm32"))]
fn js_exec(sql: &str, params_json: &str) -> JsValue {
    let mut calls = exec_calls().lock().expect("exec calls lock");
    calls.push((sql.to_string(), params_json.to_string()));
    JsValue::NULL
}

#[cfg(not(target_arch = "wasm32"))]
fn js_query(sql: &str, params_json: &str) -> JsValue {
    let mut calls = query_calls().lock().expect("query calls lock");
    calls.push((sql.to_string(), params_json.to_string()));
    JsValue::NULL
}

#[cfg(not(target_arch = "wasm32"))]
fn js_export_bytes() -> JsValue {
    let mut calls = export_calls().lock().expect("export calls lock");
    *calls += 1;
    JsValue::NULL
}

const SAVEPOINT: &str = "radroots_schema_tx";
const EXPORT_LOCK_ERR: &str = "replica db export in progress";

static EXPORT_LOCK_ACTIVE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static EXPORT_LOCK_BYPASS: Cell<bool> = const { Cell::new(false) };
}

pub struct WasmSqlExecutor;

impl WasmSqlExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WasmSqlExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlExecutor for WasmSqlExecutor {
    fn exec(&self, sql: &str, params_json: &str) -> Result<ExecOutcome, SqlError> {
        if export_lock_blocked() {
            return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
        }
        let js = exec(sql, params_json);
        let v: serde_json::Value = serde_wasm_bindgen::from_value(js)
            .map_err(|e| SqlError::SerializationError(e.to_string()))?;
        let changes = v.get("changes").and_then(|x| x.as_i64()).unwrap_or(0);
        let last_insert_id = v
            .get("last_insert_id")
            .or_else(|| v.get("lastInsertRowid"))
            .and_then(|x| x.as_i64())
            .unwrap_or(0);
        Ok(ExecOutcome {
            changes,
            last_insert_id,
        })
    }

    fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        let js = query(sql, params_json);
        let v: serde_json::Value = serde_wasm_bindgen::from_value(js)
            .map_err(|e| SqlError::SerializationError(e.to_string()))?;
        Ok(v.to_string())
    }

    fn begin(&self) -> Result<(), SqlError> {
        if export_lock_blocked() {
            return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
        }
        begin_tx();
        Ok(())
    }

    fn commit(&self) -> Result<(), SqlError> {
        if export_lock_blocked() {
            return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
        }
        commit_tx();
        Ok(())
    }

    fn rollback(&self) -> Result<(), SqlError> {
        if export_lock_blocked() {
            return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
        }
        rollback_tx();
        Ok(())
    }
}

pub fn parse_json<T: DeserializeOwned>(s: &str) -> Result<T, SqlError> {
    utils::parse_json(s)
}

pub fn err_js(err: SqlError) -> JsValue {
    err_js_value(err)
}

#[cfg(target_arch = "wasm32")]
fn err_js_value(err: SqlError) -> JsValue {
    match err_js_with_encoder(err, |err| {
        let value = err.to_json();
        serde_wasm_bindgen::to_value(&value).map_err(|_| ())
    }) {
        Ok(value) => value,
        Err(err) => JsValue::from_str(&err.to_string()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn err_js_value(err: SqlError) -> JsValue {
    let _ = err.to_json();
    JsValue::NULL
}

#[cfg(target_arch = "wasm32")]
fn err_js_with_encoder(
    err: SqlError,
    encode: impl FnOnce(&SqlError) -> Result<JsValue, ()>,
) -> Result<JsValue, SqlError> {
    encode(&err).map_err(|()| err)
}

pub fn exec(sql: &str, params_json: &str) -> JsValue {
    js_exec(sql, params_json)
}

pub fn query(sql: &str, params_json: &str) -> JsValue {
    js_query(sql, params_json)
}

pub fn export_bytes() -> JsValue {
    js_export_bytes()
}

pub fn begin_tx() {
    let _ = js_exec(&format!("savepoint {}", SAVEPOINT), "[]");
}

pub fn commit_tx() {
    let _ = js_exec(&format!("release savepoint {}", SAVEPOINT), "[]");
}

pub fn rollback_tx() {
    let _ = js_exec(&format!("rollback to savepoint {}", SAVEPOINT), "[]");
    let _ = js_exec(&format!("release savepoint {}", SAVEPOINT), "[]");
}

pub fn export_lock_begin() -> Result<(), SqlError> {
    let was_active = EXPORT_LOCK_ACTIVE.swap(true, Ordering::SeqCst);
    if was_active {
        return Err(SqlError::InvalidArgument(EXPORT_LOCK_ERR.to_string()));
    }
    Ok(())
}

pub fn export_lock_end() {
    EXPORT_LOCK_ACTIVE.store(false, Ordering::SeqCst);
}

pub fn export_lock_active() -> bool {
    EXPORT_LOCK_ACTIVE.load(Ordering::SeqCst)
}

pub fn with_export_lock_bypass<T>(f: impl FnOnce() -> T) -> T {
    EXPORT_LOCK_BYPASS.with(|flag| {
        let prev = flag.replace(true);
        let out = f();
        flag.set(prev);
        out
    })
}

fn export_lock_blocked() -> bool {
    if !EXPORT_LOCK_ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    EXPORT_LOCK_BYPASS.with(|flag| !flag.get())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use radroots_sql_core::SqlError;

    use super::{
        begin_tx, commit_tx, err_js, exec, exec_calls, export_bytes, export_calls,
        export_lock_active, export_lock_begin, export_lock_end, parse_json, query, query_calls,
        rollback_tx, with_export_lock_bypass,
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
    fn exec_query_export_delegate_to_js_hooks() {
        let _ = exec("select 1", "[]");
        let _ = query("select 2", "[1]");
        let _ = export_bytes();

        let exec_len = exec_calls().lock().map(|calls| calls.len()).unwrap_or(0);
        let query_len = query_calls().lock().map(|calls| calls.len()).unwrap_or(0);
        let export_len = export_calls().lock().map(|calls| *calls).unwrap_or(0);
        assert!(exec_len >= 1);
        assert!(query_len >= 1);
        assert!(export_len >= 1);
    }

    #[test]
    fn tx_helpers_emit_expected_savepoint_statements() {
        begin_tx();
        commit_tx();
        rollback_tx();

        let calls = exec_calls()
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default();
        assert!(
            calls
                .iter()
                .any(|(sql, _)| sql == "savepoint radroots_schema_tx")
        );
        assert!(
            calls
                .iter()
                .any(|(sql, _)| sql == "release savepoint radroots_schema_tx")
        );
        assert!(
            calls
                .iter()
                .any(|(sql, _)| sql == "rollback to savepoint radroots_schema_tx")
        );
    }

    #[test]
    fn export_lock_tracks_state() {
        assert!(!export_lock_active());
        export_lock_begin().expect("begin export lock");
        assert!(export_lock_active());
        assert!(with_export_lock_bypass(|| true));
        export_lock_end();
        assert!(!export_lock_active());
    }
}
