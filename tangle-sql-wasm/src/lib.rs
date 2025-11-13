#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

use radroots_sql_core::WasmSqlExecutor;
use radroots_tangle_schema::log_error::{
    ILogErrorDelete, ILogErrorFields, ILogErrorFieldsFilter, ILogErrorFieldsPartial,
    ILogErrorFindMany, ILogErrorFindOne, ILogErrorUpdate, LogErrorQueryBindValues,
};
use radroots_tangle_sql::{log_error, migrations};

pub mod utils;
pub use utils::*;

#[wasm_bindgen(js_name = tangle_db_run_migrations)]
pub fn tangle_db_run_migrations() -> Result<(), JsValue> {
    let exec = WasmSqlExecutor::new();
    migrations::run_all_up(&exec).map_err(radroots_sql_wasm_core::err_js)
}

#[wasm_bindgen(js_name = tangle_db_reset_database)]
pub fn tangle_db_reset_database() -> Result<(), JsValue> {
    let exec = WasmSqlExecutor::new();
    migrations::run_all_down(&exec).map_err(radroots_sql_wasm_core::err_js)
}

#[wasm_bindgen(js_name = tangle_db_log_error_create)]
pub fn tangle_db_log_error_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let payload = radroots_sql_wasm_core::parse_json::<ILogErrorFields>(opts_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let out = log_error::create(&exec, &payload)
        .map_err(|err| radroots_sql_wasm_core::err_js(err.error))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_find_many)]
pub fn tangle_db_log_error_find_many(filter_json: &str) -> Result<JsValue, JsValue> {
    let filter = parse_optional_json::<ILogErrorFieldsFilter>(filter_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let opts = ILogErrorFindMany { filter };
    let out = log_error::find_many(&exec, &opts)
        .map_err(|err| radroots_sql_wasm_core::err_js(err.error))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_find_one)]
pub fn tangle_db_log_error_find_one(bind_json: &str) -> Result<JsValue, JsValue> {
    let bind = radroots_sql_wasm_core::parse_json::<LogErrorQueryBindValues>(bind_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let opts = ILogErrorFindOne { on: bind };
    let out = log_error::find_one(&exec, &opts)
        .map_err(|err| radroots_sql_wasm_core::err_js(err.error))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_update)]
pub fn tangle_db_log_error_update(id: &str, fields_json: &str) -> Result<JsValue, JsValue> {
    let fields = radroots_sql_wasm_core::parse_json::<ILogErrorFieldsPartial>(fields_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let opts = ILogErrorUpdate {
        on: LogErrorQueryBindValues::Id { id: id.to_owned() },
        fields,
    };
    let out =
        log_error::update(&exec, &opts).map_err(|err| radroots_sql_wasm_core::err_js(err.error))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_delete)]
pub fn tangle_db_log_error_delete(bind_json: &str) -> Result<JsValue, JsValue> {
    let bind = radroots_sql_wasm_core::parse_json::<LogErrorQueryBindValues>(bind_json)
        .map_err(radroots_sql_wasm_core::err_js)?;
    let exec = WasmSqlExecutor::new();
    let opts = ILogErrorDelete { on: bind };
    let out =
        log_error::delete(&exec, &opts).map_err(|err| radroots_sql_wasm_core::err_js(err.error))?;
    value_to_js(out)
}
