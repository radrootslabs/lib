#![cfg(target_arch = "wasm32")]
#![forbid(unsafe_code)]

use radroots_events::RadrootsNostrEvent;
use radroots_sql_core::WasmSqlExecutor;
use radroots_tangle_events::{
    radroots_tangle_ingest_event,
    radroots_tangle_sync_all,
    RadrootsTangleIngestOutcome,
    RadrootsTangleSyncRequest,
};
use wasm_bindgen::prelude::*;

fn err_js<E: ToString>(err: E) -> JsValue {
    JsValue::from_str(&err.to_string())
}

fn parse_request(request_json: &str) -> Result<RadrootsTangleSyncRequest, JsValue> {
    serde_json::from_str(request_json).map_err(err_js)
}

fn parse_event(event_json: &str) -> Result<RadrootsNostrEvent, JsValue> {
    serde_json::from_str(event_json).map_err(err_js)
}

#[wasm_bindgen(js_name = tangle_events_sync_all)]
pub fn tangle_events_sync_all(request_json: &str) -> Result<JsValue, JsValue> {
    let request = parse_request(request_json)?;
    let exec = WasmSqlExecutor::new();
    let bundle = radroots_tangle_sync_all(&exec, &request).map_err(err_js)?;
    serde_wasm_bindgen::to_value(&bundle).map_err(err_js)
}

#[wasm_bindgen(js_name = tangle_events_ingest_event)]
pub fn tangle_events_ingest_event(event_json: &str) -> Result<JsValue, JsValue> {
    let event = parse_event(event_json)?;
    let exec = WasmSqlExecutor::new();
    let outcome = radroots_tangle_ingest_event(&exec, &event).map_err(err_js)?;
    let value = match outcome {
        RadrootsTangleIngestOutcome::Applied => "applied",
        RadrootsTangleIngestOutcome::Skipped => "skipped",
    };
    Ok(JsValue::from_str(value))
}
