#![forbid(unsafe_code)]

use radroots_sql_core::{ExecOutcome, SqlError, SqlExecutor, utils};
use serde::de::DeserializeOwned;
use serde_json::Value;
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

#[cfg(all(not(target_arch = "wasm32"), test))]
use std::collections::VecDeque;

#[cfg(not(target_arch = "wasm32"))]
type RecordedCall = (String, String);

#[cfg(all(not(target_arch = "wasm32"), test))]
type NativeHostResult = Result<Value, SqlError>;

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

#[cfg(all(not(target_arch = "wasm32"), test))]
fn exec_results() -> &'static Mutex<VecDeque<NativeHostResult>> {
    static EXEC_RESULTS: OnceLock<Mutex<VecDeque<NativeHostResult>>> = OnceLock::new();
    EXEC_RESULTS.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(all(not(target_arch = "wasm32"), test))]
fn query_results() -> &'static Mutex<VecDeque<NativeHostResult>> {
    static QUERY_RESULTS: OnceLock<Mutex<VecDeque<NativeHostResult>>> = OnceLock::new();
    QUERY_RESULTS.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(all(not(target_arch = "wasm32"), test))]
fn push_exec_result(result: NativeHostResult) {
    let mut results = exec_results().lock().expect("exec results lock");
    results.push_back(result);
}

#[cfg(all(not(target_arch = "wasm32"), test))]
fn push_query_result(result: NativeHostResult) {
    let mut results = query_results().lock().expect("query results lock");
    results.push_back(result);
}

#[cfg(all(not(target_arch = "wasm32"), test))]
fn take_exec_result() -> Option<NativeHostResult> {
    exec_results()
        .lock()
        .expect("exec results lock")
        .pop_front()
}

#[cfg(all(not(target_arch = "wasm32"), test))]
fn take_query_result() -> Option<NativeHostResult> {
    query_results()
        .lock()
        .expect("query results lock")
        .pop_front()
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
        let v = host_exec_json(sql, params_json)?;
        Ok(exec_outcome_from_json(&v))
    }

    fn query_raw(&self, sql: &str, params_json: &str) -> Result<String, SqlError> {
        let v = host_query_json(sql, params_json)?;
        Ok(query_raw_from_json(&v))
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

fn host_exec_json(sql: &str, params_json: &str) -> Result<Value, SqlError> {
    let js = exec(sql, params_json);
    #[cfg(all(not(target_arch = "wasm32"), test))]
    if let Some(result) = take_exec_result() {
        return result;
    }
    sql_value_from_js(js)
}

fn host_query_json(sql: &str, params_json: &str) -> Result<Value, SqlError> {
    let js = query(sql, params_json);
    #[cfg(all(not(target_arch = "wasm32"), test))]
    if let Some(result) = take_query_result() {
        return result;
    }
    sql_value_from_js(js)
}

#[cfg(target_arch = "wasm32")]
fn sql_value_from_js(js: JsValue) -> Result<Value, SqlError> {
    serde_wasm_bindgen::from_value(js).map_err(|e| SqlError::SerializationError(e.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
fn sql_value_from_js(_js: JsValue) -> Result<Value, SqlError> {
    Err(SqlError::UnsupportedPlatform)
}

fn exec_outcome_from_json(v: &Value) -> ExecOutcome {
    let changes = v.get("changes").and_then(|x| x.as_i64()).unwrap_or(0);
    let last_insert_id = v
        .get("last_insert_id")
        .or_else(|| v.get("lastInsertRowid"))
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    ExecOutcome {
        changes,
        last_insert_id,
    }
}

fn query_raw_from_json(v: &Value) -> String {
    v.to_string()
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
    use std::{
        collections::BTreeMap,
        sync::{Mutex, OnceLock},
    };

    use radroots_sql_core::{SqlError, SqlExecutor};
    use serde_json::json;

    use super::{
        WasmSqlExecutor, begin_tx, commit_tx, err_js, exec, exec_calls, exec_outcome_from_json,
        exec_results, export_bytes, export_calls, export_lock_active, export_lock_begin,
        export_lock_end, parse_json, push_exec_result, push_query_result, query, query_calls,
        query_raw_from_json, query_results, rollback_tx, with_export_lock_bypass,
    };

    fn native_test_lock() -> &'static Mutex<()> {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn reset_native_state() {
        exec_calls().lock().expect("exec calls lock").clear();
        query_calls().lock().expect("query calls lock").clear();
        *export_calls().lock().expect("export calls lock") = 0;
        exec_results().lock().expect("exec results lock").clear();
        query_results().lock().expect("query results lock").clear();
        export_lock_end();
    }

    #[test]
    fn parse_json_reports_valid_and_invalid_payloads() {
        let parsed: BTreeMap<String, u64> = parse_json(r#"{"count":2}"#).expect("parse json");
        assert_eq!(parsed.get("count"), Some(&2));
        assert_eq!(
            parse_json::<BTreeMap<String, u64>>("{").unwrap_err().code(),
            "ERR_SERIALIZATION"
        );
    }

    #[test]
    fn err_js_accepts_sql_errors() {
        let _ = err_js(SqlError::Internal);
        let _ = err_js(SqlError::UnsupportedPlatform);
    }

    #[test]
    fn exec_query_export_delegate_to_js_hooks() {
        let _guard = native_test_lock().lock().expect("native test lock");
        reset_native_state();

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
        let _guard = native_test_lock().lock().expect("native test lock");
        reset_native_state();

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
        let _guard = native_test_lock().lock().expect("native test lock");
        reset_native_state();

        assert!(!export_lock_active());
        export_lock_begin().expect("begin export lock");
        assert!(export_lock_active());
        assert!(with_export_lock_bypass(|| true));
        export_lock_end();
        assert!(!export_lock_active());
    }

    #[test]
    fn executor_decodes_exec_outcomes() {
        let _guard = native_test_lock().lock().expect("native test lock");
        reset_native_state();

        let executor = WasmSqlExecutor;
        push_exec_result(Ok(json!({"changes": 2, "lastInsertRowid": 99})));
        let outcome = executor
            .exec("insert into listing values (?)", r#"["bin-1"]"#)
            .expect("exec outcome");
        assert_eq!(outcome.changes, 2);
        assert_eq!(outcome.last_insert_id, 99);

        push_exec_result(Ok(json!({"changes": 3, "last_insert_id": 101})));
        let outcome = executor
            .exec("update listing set qty = ?", "[1]")
            .expect("exec outcome");
        assert_eq!(outcome.changes, 3);
        assert_eq!(outcome.last_insert_id, 101);

        let default_outcome = exec_outcome_from_json(&json!({}));
        assert_eq!(default_outcome.changes, 0);
        assert_eq!(default_outcome.last_insert_id, 0);

        let calls = exec_calls().lock().expect("exec calls lock").clone();
        assert!(calls.iter().any(|(sql, params)| {
            sql == "insert into listing values (?)" && params == r#"["bin-1"]"#
        }));
    }

    #[test]
    fn executor_decodes_query_results_and_host_errors() {
        let _guard = native_test_lock().lock().expect("native test lock");
        reset_native_state();

        let executor = WasmSqlExecutor::new();
        let rows = json!([{"id": "listing-1"}]);
        push_query_result(Ok(rows.clone()));
        assert_eq!(
            executor
                .query_raw("select id from listing", "[]")
                .expect("query rows"),
            query_raw_from_json(&rows)
        );

        assert_eq!(
            executor
                .query_raw("select id from listing", "[]")
                .unwrap_err()
                .code(),
            "ERR_UNSUPPORTED_PLATFORM"
        );

        push_exec_result(Err(SqlError::SerializationError(
            "host response was not an object".to_string(),
        )));
        assert_eq!(
            executor
                .exec("insert into listing values (?)", "[]")
                .unwrap_err()
                .code(),
            "ERR_SERIALIZATION"
        );
        assert_eq!(
            executor
                .exec("insert into listing values (?)", "[]")
                .unwrap_err()
                .code(),
            "ERR_UNSUPPORTED_PLATFORM"
        );
    }

    #[test]
    fn export_lock_rejects_nested_lock_and_write_trait_calls() {
        let _guard = native_test_lock().lock().expect("native test lock");
        reset_native_state();

        let executor = WasmSqlExecutor::new();
        export_lock_begin().expect("begin export lock");
        assert_eq!(
            export_lock_begin().unwrap_err().to_string(),
            "invalid argument: replica db export in progress"
        );
        assert_eq!(
            executor
                .exec("insert into listing values (?)", "[]")
                .unwrap_err()
                .code(),
            "ERR_INVALID_ARGUMENT"
        );
        assert_eq!(executor.begin().unwrap_err().code(), "ERR_INVALID_ARGUMENT");
        assert_eq!(
            executor.commit().unwrap_err().code(),
            "ERR_INVALID_ARGUMENT"
        );
        assert_eq!(
            executor.rollback().unwrap_err().code(),
            "ERR_INVALID_ARGUMENT"
        );

        with_export_lock_bypass(|| {
            push_exec_result(Ok(json!({"changes": 1, "last_insert_id": 7})));
            let outcome = executor
                .exec("insert into listing values (?)", "[]")
                .expect("bypassed exec");
            assert_eq!(outcome.changes, 1);
            executor.begin().expect("bypassed begin");
            executor.commit().expect("bypassed commit");
            executor.rollback().expect("bypassed rollback");
        });
        assert!(export_lock_active());
        export_lock_end();
    }
}
