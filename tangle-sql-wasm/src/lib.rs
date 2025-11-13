#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

use radroots_sql_core::WasmSqlExecutor;
use radroots_tangle_schema::log_error::{
    ILogErrorFields, ILogErrorFieldsFilter, ILogErrorFieldsPartial, LogError,
    LogErrorQueryBindValues,
};
use radroots_tangle_sql::log_error;

pub mod utils;
pub use utils::*;

#[wasm_bindgen(js_name = tangle_log_error_create)]
pub fn tangle_log_error_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let payload = radroots_sql_wasm_core::parse_json::<ILogErrorFields>(opts_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let out = log_error::insert(&exec, payload).map_err(radroots_sql_wasm_core::err_js)?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_log_error_find_many)]
pub fn tangle_log_error_find_many(filter_json: &str) -> Result<JsValue, JsValue> {
    let filter = parse_optional_json::<ILogErrorFieldsFilter>(filter_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        log_error::find_many(&exec, filter.as_ref()).map_err(radroots_sql_wasm_core::err_js)?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_log_error_find_one)]
pub fn tangle_log_error_find_one(bind_json: &str) -> Result<JsValue, JsValue> {
    let bind = radroots_sql_wasm_core::parse_json::<LogErrorQueryBindValues>(bind_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let out: Option<LogError> =
        log_error::find_one(&exec, &bind).map_err(radroots_sql_wasm_core::err_js)?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_log_error_update)]
pub fn tangle_log_error_update(id: &str, fields_json: &str) -> Result<JsValue, JsValue> {
    let fields = radroots_sql_wasm_core::parse_json::<ILogErrorFieldsPartial>(fields_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let outcome = log_error::update(&exec, id, fields).map_err(radroots_sql_wasm_core::err_js)?;
    outcome_to_js(outcome)
}

#[wasm_bindgen(js_name = tangle_log_error_delete)]
pub fn tangle_log_error_delete(bind_json: &str) -> Result<JsValue, JsValue> {
    let bind = radroots_sql_wasm_core::parse_json::<LogErrorQueryBindValues>(bind_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let outcome = log_error::delete(&exec, &bind).map_err(radroots_sql_wasm_core::err_js)?;
    outcome_to_js(outcome)
}
