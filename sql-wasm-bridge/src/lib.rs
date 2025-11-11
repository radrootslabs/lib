use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = __radroots_sql_wasm_exec)]
    fn js_exec(sql: &str, params_json: &str) -> JsValue;

    #[wasm_bindgen(js_name = __radroots_sql_wasm_query)]
    fn js_query(sql: &str, params_json: &str) -> JsValue;
}

const SAVEPOINT: &str = "radroots_schema_tx";

pub fn exec(sql: &str, params_json: &str) -> JsValue {
    js_exec(sql, params_json)
}

pub fn query(sql: &str, params_json: &str) -> JsValue {
    js_query(sql, params_json)
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
