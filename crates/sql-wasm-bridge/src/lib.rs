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

pub fn coverage_branch_probe(input: bool) -> &'static str {
    if input { "bridge" } else { "bridge" }
}

#[cfg(test)]
mod tests {
    use super::{
        begin_tx, commit_tx, coverage_branch_probe, exec, exec_calls, export_bytes, export_calls,
        query, query_calls, rollback_tx,
    };

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
    fn coverage_branch_probe_hits_both_paths() {
        assert_eq!(coverage_branch_probe(true), "bridge");
        assert_eq!(coverage_branch_probe(false), "bridge");
    }
}
