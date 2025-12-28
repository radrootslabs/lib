#![cfg(target_arch = "wasm32")]

use radroots_sql_core::{
    WasmSqlExecutor,
    export_lock_begin,
    export_lock_end,
    with_export_lock_bypass,
};
use radroots_sql_wasm_core::{err_js, parse_json};
use radroots_tangle_db::migrations;
use radroots_tangle_db::{export_manifest, TangleDbExportManifestRs};
use radroots_tangle_events::radroots_tangle_sync_status;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;

use radroots_tangle_db_schema::farm::{
    IFarmCreate,
    IFarmDelete,
    IFarmFindMany,
    IFarmFindOne,
    IFarmUpdate,
};

use radroots_tangle_db_schema::farm_gcs_location::{
    IFarmGcsLocationCreate,
    IFarmGcsLocationDelete,
    IFarmGcsLocationFindMany,
    IFarmGcsLocationFindOne,
    IFarmGcsLocationUpdate,
};

use radroots_tangle_db_schema::farm_member::{
    IFarmMemberCreate,
    IFarmMemberDelete,
    IFarmMemberFindMany,
    IFarmMemberFindOne,
    IFarmMemberUpdate,
};

use radroots_tangle_db_schema::farm_member_claim::{
    IFarmMemberClaimCreate,
    IFarmMemberClaimDelete,
    IFarmMemberClaimFindMany,
    IFarmMemberClaimFindOne,
    IFarmMemberClaimUpdate,
};

use radroots_tangle_db_schema::farm_tag::{
    IFarmTagCreate,
    IFarmTagDelete,
    IFarmTagFindMany,
    IFarmTagFindOne,
    IFarmTagUpdate,
};

use radroots_tangle_db_schema::gcs_location::{
    IGcsLocationCreate,
    IGcsLocationDelete,
    IGcsLocationFindMany,
    IGcsLocationFindOne,
    IGcsLocationUpdate,
};

use radroots_tangle_db_schema::log_error::{
    ILogErrorCreate,
    ILogErrorDelete,
    ILogErrorFindMany,
    ILogErrorFindOne,
    ILogErrorUpdate,
};

use radroots_tangle_db_schema::media_image::{
    IMediaImageCreate,
    IMediaImageDelete,
    IMediaImageFindMany,
    IMediaImageFindOne,
    IMediaImageUpdate,
};

use radroots_tangle_db_schema::nostr_profile::{
    INostrProfileCreate,
    INostrProfileDelete,
    INostrProfileFindMany,
    INostrProfileFindOne,
    INostrProfileUpdate,
};

use radroots_tangle_db_schema::nostr_event_state::{
    INostrEventStateCreate,
    INostrEventStateDelete,
    INostrEventStateFindMany,
    INostrEventStateFindOne,
    INostrEventStateUpdate,
};

use radroots_tangle_db_schema::nostr_relay::{
    INostrRelayCreate,
    INostrRelayDelete,
    INostrRelayFindMany,
    INostrRelayFindOne,
    INostrRelayUpdate,
};

use radroots_tangle_db_schema::trade_product::{
    ITradeProductCreate,
    ITradeProductDelete,
    ITradeProductFindMany,
    ITradeProductFindOne,
    ITradeProductUpdate,
};

use radroots_tangle_db_schema::plot::{
    IPlotCreate,
    IPlotDelete,
    IPlotFindMany,
    IPlotFindOne,
    IPlotUpdate,
};

use radroots_tangle_db_schema::plot_gcs_location::{
    IPlotGcsLocationCreate,
    IPlotGcsLocationDelete,
    IPlotGcsLocationFindMany,
    IPlotGcsLocationFindOne,
    IPlotGcsLocationUpdate,
};

use radroots_tangle_db_schema::plot_tag::{
    IPlotTagCreate,
    IPlotTagDelete,
    IPlotTagFindMany,
    IPlotTagFindOne,
    IPlotTagUpdate,
};

use radroots_tangle_db_schema::nostr_profile_relay::{
    INostrProfileRelayRelation,
};

use radroots_tangle_db_schema::trade_product_location::{
    ITradeProductLocationRelation,
};

use radroots_tangle_db_schema::trade_product_media::{
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

#[wasm_bindgen(js_name = tangle_db_export_json)]
pub fn tangle_db_export_json() -> Result<JsValue, JsValue> {
    let exec = WasmSqlExecutor::new();
    let dump = radroots_tangle_db::backup::export_database_backup(&exec).map_err(err_js)?;
    value_to_js(dump)
}

#[wasm_bindgen(js_name = tangle_db_import_json)]
pub fn tangle_db_import_json(dump_json: &str) -> Result<(), JsValue> {
    let exec = WasmSqlExecutor::new();
    radroots_tangle_db::backup::restore_database_backup_json(&exec, dump_json).map_err(err_js)
}

#[wasm_bindgen(js_name = tangle_db_export_begin)]
pub fn tangle_db_export_begin() -> Result<JsValue, JsValue> {
    export_lock_begin().map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let result = with_export_lock_bypass(|| export_snapshot(&exec));
    match result {
        Ok(value) => Ok(value),
        Err(err) => {
            export_lock_end();
            Err(err)
        }
    }
}

#[wasm_bindgen(js_name = tangle_db_export_finish)]
pub fn tangle_db_export_finish() -> Result<(), JsValue> {
    export_lock_end();
    Ok(())
}

fn export_snapshot(exec: &WasmSqlExecutor) -> Result<JsValue, JsValue> {
    let status = radroots_tangle_sync_status(exec).map_err(|err| {
        err_js(radroots_sql_core::SqlError::InvalidArgument(err.to_string()))
    })?;
    if status.pending_count > 0 {
        return Err(err_js(radroots_sql_core::SqlError::InvalidArgument(
            format!(
                "tangle db export requires synced state (pending {}/{})",
                status.pending_count, status.expected_count
            ),
        )));
    }
    let manifest = export_manifest(exec).map_err(err_js)?;
    export_snapshot_value(manifest)
}

fn export_snapshot_value(manifest: TangleDbExportManifestRs) -> Result<JsValue, JsValue> {
    let bytes_js = radroots_sql_wasm_core::export_bytes();
    export_snapshot_value_with_bytes(manifest, bytes_js)
}

fn export_snapshot_value_with_bytes(
    manifest: TangleDbExportManifestRs,
    bytes_js: JsValue,
) -> Result<JsValue, JsValue> {
    let manifest_js = serde_wasm_bindgen::to_value(&manifest)
        .map_err(|err| err_js(radroots_sql_core::SqlError::SerializationError(err.to_string())))?;
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("manifest_rs"), &manifest_js)
        .map_err(|_| err_js(radroots_sql_core::SqlError::Internal))?;
    js_sys::Reflect::set(&obj, &JsValue::from_str("db_bytes"), &bytes_js)
        .map_err(|_| err_js(radroots_sql_core::SqlError::Internal))?;
    Ok(JsValue::from(obj))
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::export_snapshot_value_with_bytes;
    use js_sys::{Reflect, Uint8Array};
    use wasm_bindgen::JsValue;

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn export_snapshot_value_includes_fields() {
        let manifest = radroots_tangle_db::TangleDbExportManifestRs {
            export_version: "1".to_string(),
            tangle_db_version: "0.0.0".to_string(),
            backup_format_version: "0.0.0".to_string(),
            schema_hash: "hash".to_string(),
            schema: Vec::new(),
            migrations: Vec::new(),
            table_counts: Vec::new(),
        };
        let bytes = Uint8Array::new_with_length(2);
        let js = export_snapshot_value_with_bytes(manifest, JsValue::from(bytes))
            .expect("snapshot");
        let manifest_rs = Reflect::get(&js, &JsValue::from_str("manifest_rs"))
            .expect("manifest_rs");
        let db_bytes = Reflect::get(&js, &JsValue::from_str("db_bytes"))
            .expect("db_bytes");
        assert!(manifest_rs.is_object());
        assert!(db_bytes.is_object());
    }
}

#[wasm_bindgen(js_name = tangle_db_farm_create)]
pub fn tangle_db_farm_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_find_one)]
pub fn tangle_db_farm_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_find_many)]
pub fn tangle_db_farm_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_update)]
pub fn tangle_db_farm_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_delete)]
pub fn tangle_db_farm_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_create)]
pub fn tangle_db_plot_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_find_one)]
pub fn tangle_db_plot_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_find_many)]
pub fn tangle_db_plot_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_update)]
pub fn tangle_db_plot_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_delete)]
pub fn tangle_db_plot_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_gcs_location_create)]
pub fn tangle_db_gcs_location_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IGcsLocationCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::gcs_location::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_gcs_location_find_one)]
pub fn tangle_db_gcs_location_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IGcsLocationFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::gcs_location::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_gcs_location_find_many)]
pub fn tangle_db_gcs_location_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IGcsLocationFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::gcs_location::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_gcs_location_update)]
pub fn tangle_db_gcs_location_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IGcsLocationUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::gcs_location::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_gcs_location_delete)]
pub fn tangle_db_gcs_location_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IGcsLocationDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::gcs_location::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_gcs_location_create)]
pub fn tangle_db_farm_gcs_location_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmGcsLocationCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_gcs_location::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_gcs_location_find_one)]
pub fn tangle_db_farm_gcs_location_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmGcsLocationFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_gcs_location::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_gcs_location_find_many)]
pub fn tangle_db_farm_gcs_location_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmGcsLocationFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_gcs_location::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_gcs_location_update)]
pub fn tangle_db_farm_gcs_location_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmGcsLocationUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_gcs_location::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_gcs_location_delete)]
pub fn tangle_db_farm_gcs_location_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmGcsLocationDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_gcs_location::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_gcs_location_create)]
pub fn tangle_db_plot_gcs_location_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotGcsLocationCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_gcs_location::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_gcs_location_find_one)]
pub fn tangle_db_plot_gcs_location_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotGcsLocationFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_gcs_location::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_gcs_location_find_many)]
pub fn tangle_db_plot_gcs_location_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotGcsLocationFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_gcs_location::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_gcs_location_update)]
pub fn tangle_db_plot_gcs_location_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotGcsLocationUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_gcs_location::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_gcs_location_delete)]
pub fn tangle_db_plot_gcs_location_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotGcsLocationDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_gcs_location::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_tag_create)]
pub fn tangle_db_farm_tag_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmTagCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_tag::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_tag_find_one)]
pub fn tangle_db_farm_tag_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmTagFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_tag::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_tag_find_many)]
pub fn tangle_db_farm_tag_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmTagFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_tag::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_tag_update)]
pub fn tangle_db_farm_tag_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmTagUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_tag::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_tag_delete)]
pub fn tangle_db_farm_tag_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmTagDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_tag::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_tag_create)]
pub fn tangle_db_plot_tag_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotTagCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_tag::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_tag_find_one)]
pub fn tangle_db_plot_tag_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotTagFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_tag::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_tag_find_many)]
pub fn tangle_db_plot_tag_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotTagFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_tag::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_tag_update)]
pub fn tangle_db_plot_tag_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotTagUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_tag::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_plot_tag_delete)]
pub fn tangle_db_plot_tag_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IPlotTagDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::plot_tag::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_create)]
pub fn tangle_db_farm_member_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_find_one)]
pub fn tangle_db_farm_member_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_find_many)]
pub fn tangle_db_farm_member_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_update)]
pub fn tangle_db_farm_member_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_delete)]
pub fn tangle_db_farm_member_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_claim_create)]
pub fn tangle_db_farm_member_claim_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberClaimCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member_claim::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_claim_find_one)]
pub fn tangle_db_farm_member_claim_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberClaimFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member_claim::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_claim_find_many)]
pub fn tangle_db_farm_member_claim_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberClaimFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member_claim::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_claim_update)]
pub fn tangle_db_farm_member_claim_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberClaimUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member_claim::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_farm_member_claim_delete)]
pub fn tangle_db_farm_member_claim_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IFarmMemberClaimDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::farm_member_claim::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_create)]
pub fn tangle_db_log_error_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::log_error::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_find_one)]
pub fn tangle_db_log_error_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::log_error::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_find_many)]
pub fn tangle_db_log_error_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::log_error::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_update)]
pub fn tangle_db_log_error_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::log_error::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_log_error_delete)]
pub fn tangle_db_log_error_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ILogErrorDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::log_error::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_create)]
pub fn tangle_db_media_image_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::media_image::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_find_one)]
pub fn tangle_db_media_image_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::media_image::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_find_many)]
pub fn tangle_db_media_image_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::media_image::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_update)]
pub fn tangle_db_media_image_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::media_image::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_media_image_delete)]
pub fn tangle_db_media_image_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: IMediaImageDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::media_image::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_create)]
pub fn tangle_db_nostr_profile_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_profile::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_find_one)]
pub fn tangle_db_nostr_profile_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_profile::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_find_many)]
pub fn tangle_db_nostr_profile_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_profile::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_update)]
pub fn tangle_db_nostr_profile_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_profile::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_delete)]
pub fn tangle_db_nostr_profile_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_profile::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_event_state_create)]
pub fn tangle_db_nostr_event_state_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrEventStateCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_event_state::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_event_state_find_one)]
pub fn tangle_db_nostr_event_state_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrEventStateFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_event_state::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_event_state_find_many)]
pub fn tangle_db_nostr_event_state_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrEventStateFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_event_state::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_event_state_update)]
pub fn tangle_db_nostr_event_state_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrEventStateUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_event_state::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_event_state_delete)]
pub fn tangle_db_nostr_event_state_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrEventStateDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_event_state::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_create)]
pub fn tangle_db_nostr_relay_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_relay::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_find_one)]
pub fn tangle_db_nostr_relay_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_relay::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_find_many)]
pub fn tangle_db_nostr_relay_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_relay::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_update)]
pub fn tangle_db_nostr_relay_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_relay::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_relay_delete)]
pub fn tangle_db_nostr_relay_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrRelayDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_relay::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_create)]
pub fn tangle_db_trade_product_create(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductCreate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product::create(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_find_one)]
pub fn tangle_db_trade_product_find_one(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductFindOne = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product::find_one(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_find_many)]
pub fn tangle_db_trade_product_find_many(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductFindMany = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product::find_many(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_update)]
pub fn tangle_db_trade_product_update(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductUpdate = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product::update(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_delete)]
pub fn tangle_db_trade_product_delete(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductDelete = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product::delete(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_relay_set)]
pub fn tangle_db_nostr_profile_relay_set(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileRelayRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_profile_relay::set(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_nostr_profile_relay_unset)]
pub fn tangle_db_nostr_profile_relay_unset(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: INostrProfileRelayRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::nostr_profile_relay::unset(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_location_set)]
pub fn tangle_db_trade_product_location_set(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductLocationRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product_location::set(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_location_unset)]
pub fn tangle_db_trade_product_location_unset(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductLocationRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product_location::unset(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_media_set)]
pub fn tangle_db_trade_product_media_set(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductMediaRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product_media::set(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}

#[wasm_bindgen(js_name = tangle_db_trade_product_media_unset)]
pub fn tangle_db_trade_product_media_unset(opts_json: &str) -> Result<JsValue, JsValue> {
    let opts: ITradeProductMediaRelation = parse_json(opts_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let out =
        radroots_tangle_db::trade_product_media::unset(&exec, &opts).map_err(|e| err_js(e.err))?;
    value_to_js(out)
}
