use wasm_bindgen::prelude::*;

pub mod error;
pub mod utils;

#[wasm_bindgen(js_name = exec_sql)]
pub fn exec_sql(sql: &str, params_json: &str) -> JsValue {
    radroots_sql_wasm_bridge::exec(sql, params_json)
}

#[wasm_bindgen(js_name = query_sql)]
pub fn query_sql(sql: &str, params_json: &str) -> JsValue {
    radroots_sql_wasm_bridge::query(sql, params_json)
}

#[wasm_bindgen(js_name = begin_tx)]
pub fn begin_tx() {
    radroots_sql_wasm_bridge::begin_tx()
}

#[wasm_bindgen(js_name = commit_tx)]
pub fn commit_tx() {
    radroots_sql_wasm_bridge::commit_tx()
}

#[wasm_bindgen(js_name = rollback_tx)]
pub fn rollback_tx() {
    radroots_sql_wasm_bridge::rollback_tx()
}
