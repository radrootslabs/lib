#![cfg(target_arch = "wasm32")]

use radroots_sql_core::WasmSqlExecutor;
use radroots_sql_wasm_core::{err_js, parse_json};
use radroots_tangle_schema::log_error::{
    ILogErrorCreate, ILogErrorDelete, ILogErrorFindMany, ILogErrorFindOne, ILogErrorUpdate,
};
use radroots_tangle_sql::migrations;
use wasm_bindgen::prelude::*;

use radroots_tangle_schema::farm::{
    IFarmCreate,
    IFarmDelete,
    IFarmFindMany,
    IFarmFindOne,
    IFarmUpdate,
};

use radroots_tangle_schema::location_gcs::{
    ILocationGcsCreate,
    ILocationGcsDelete,
    ILocationGcsFindMany,
    ILocationGcsFindOne,
    ILocationGcsUpdate,
};

use radroots_tangle_schema::media_image::{
    IMediaImageCreate,
    IMediaImageDelete,
    IMediaImageFindMany,
    IMediaImageFindOne,
    IMediaImageUpdate,
};

use radroots_tangle_schema::nostr_profile::{
    INostrProfileCreate,
    INostrProfileDelete,
    INostrProfileFindMany,
    INostrProfileFindOne,
    INostrProfileUpdate,
};

use radroots_tangle_schema::nostr_relay::{
    INostrRelayCreate,
    INostrRelayDelete,
    INostrRelayFindMany,
    INostrRelayFindOne,
    INostrRelayUpdate,
};

use radroots_tangle_schema::trade_product::{
    ITradeProductCreate,
    ITradeProductDelete,
    ITradeProductFindMany,
    ITradeProductFindOne,
    ITradeProductUpdate,
};

use radroots_tangle_schema::farm_location::{
    IFarmLocationRelation,
};

use radroots_tangle_schema::nostr_profile_relay::{
    INostrProfileRelayRelation,
};

use radroots_tangle_schema::trade_product_location::{
    ITradeProductLocationRelation,
};

use radroots_tangle_schema::trade_product_media::{
    ITradeProductMediaRelation,
};

pub mod utils;
pub use utils::*;

#[wasm_bindgen(js_name = tangle_db_run_migrations)]
pub fn tangle_db_run_migrations() -> Result<(), JsValue> {
    let exec = WasmSqlExecutor::new();
    migrations::run_all_up(&exec).map_err(err_js)
}

#[wasm_bindgen(js_name = tangle_db_reset_database)]
pub fn tangle_db_reset_database() -> Result<(), JsValue> {
    let exec = WasmSqlExecutor::new();
    migrations::run_all_down(&exec).map_err(err_js)
}

#[wasm_bindgen(js_name = tangle_db_log_error_create)]
pub fn tangle_db_log_error_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::log_error::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_find_one)]
pub fn tangle_db_log_error_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::log_error::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_find_many)]
pub fn tangle_db_log_error_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::log_error::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_update)]
pub fn tangle_db_log_error_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::log_error::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_delete)]
pub fn tangle_db_log_error_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::log_error::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_create)]
pub fn tangle_db_farm_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::farm::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_find_one)]
pub fn tangle_db_farm_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::farm::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_find_many)]
pub fn tangle_db_farm_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::farm::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_update)]
pub fn tangle_db_farm_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::farm::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_delete)]
pub fn tangle_db_farm_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::farm::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_location_gcs_create)]
pub fn tangle_db_location_gcs_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILocationGcsCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::location_gcs::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_location_gcs_find_one)]
pub fn tangle_db_location_gcs_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILocationGcsFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::location_gcs::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_location_gcs_find_many)]
pub fn tangle_db_location_gcs_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILocationGcsFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::location_gcs::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_location_gcs_update)]
pub fn tangle_db_location_gcs_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILocationGcsUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::location_gcs::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_location_gcs_delete)]
pub fn tangle_db_location_gcs_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILocationGcsDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::location_gcs::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_create)]
pub fn tangle_db_media_image_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::media_image::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_find_one)]
pub fn tangle_db_media_image_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::media_image::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_find_many)]
pub fn tangle_db_media_image_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::media_image::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_update)]
pub fn tangle_db_media_image_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::media_image::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_delete)]
pub fn tangle_db_media_image_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::media_image::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_create)]
pub fn tangle_db_nostr_profile_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_profile::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_find_one)]
pub fn tangle_db_nostr_profile_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_profile::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_find_many)]
pub fn tangle_db_nostr_profile_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_profile::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_update)]
pub fn tangle_db_nostr_profile_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_profile::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_delete)]
pub fn tangle_db_nostr_profile_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_profile::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_create)]
pub fn tangle_db_nostr_relay_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_relay::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_find_one)]
pub fn tangle_db_nostr_relay_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_relay::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_find_many)]
pub fn tangle_db_nostr_relay_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_relay::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_update)]
pub fn tangle_db_nostr_relay_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_relay::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_delete)]
pub fn tangle_db_nostr_relay_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_relay::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_create)]
pub fn tangle_db_trade_product_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_find_one)]
pub fn tangle_db_trade_product_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_find_many)]
pub fn tangle_db_trade_product_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_update)]
pub fn tangle_db_trade_product_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_delete)]
pub fn tangle_db_trade_product_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_location_set)]
pub fn tangle_db_farm_location_set(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmLocationRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::farm_location::set(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_location_unset)]
pub fn tangle_db_farm_location_unset(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmLocationRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::farm_location::unset(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_relay_set)]
pub fn tangle_db_nostr_profile_relay_set(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileRelayRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_profile_relay::set(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_relay_unset)]
pub fn tangle_db_nostr_profile_relay_unset(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileRelayRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::nostr_profile_relay::unset(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_location_set)]
pub fn tangle_db_trade_product_location_set(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductLocationRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product_location::set(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_location_unset)]
pub fn tangle_db_trade_product_location_unset(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductLocationRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product_location::unset(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_media_set)]
pub fn tangle_db_trade_product_media_set(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductMediaRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product_media::set(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_media_unset)]
pub fn tangle_db_trade_product_media_unset(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductMediaRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_sql::trade_product_media::unset(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}
