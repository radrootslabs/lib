#![cfg(any(target_arch = "wasm32", coverage_nightly))]
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use base64::Engine;
#[cfg(target_arch = "wasm32")]
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
#[cfg(target_arch = "wasm32")]
use radroots_events::RadrootsNostrEvent;
#[cfg(target_arch = "wasm32")]
use radroots_sql_core::WasmSqlExecutor;
#[cfg(target_arch = "wasm32")]
use radroots_tangle_events::{
    RadrootsTangleIdFactory, RadrootsTangleIngestOutcome, RadrootsTangleSyncRequest,
    radroots_tangle_ingest_event_with_factory, radroots_tangle_sync_all,
};
#[cfg(target_arch = "wasm32")]
use serde::Deserialize;
#[cfg(target_arch = "wasm32")]
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
fn err_js<E: ToString>(err: E) -> JsValue {
    JsValue::from_str(&err.to_string())
}

#[cfg(target_arch = "wasm32")]
struct WasmIdFactory;

#[cfg(target_arch = "wasm32")]
impl RadrootsTangleIdFactory for WasmIdFactory {
    fn new_d_tag(&self) -> String {
        let uuid = Uuid::now_v7();
        URL_SAFE_NO_PAD.encode(uuid.as_bytes())
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Deserialize)]
struct NostrEventEnvelope {
    id: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    pubkey: Option<String>,
    created_at: u32,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

#[cfg(target_arch = "wasm32")]
fn parse_request(request_json: &str) -> Result<RadrootsTangleSyncRequest, JsValue> {
    serde_json::from_str(request_json).map_err(err_js)
}

#[cfg(target_arch = "wasm32")]
fn parse_event(event_json: &str) -> Result<RadrootsNostrEvent, JsValue> {
    let envelope: NostrEventEnvelope = serde_json::from_str(event_json).map_err(err_js)?;
    let author = match (envelope.author, envelope.pubkey) {
        (Some(author), Some(pubkey)) if author != pubkey => {
            return Err(JsValue::from_str("author/pubkey mismatch"));
        }
        (Some(author), _) => author,
        (None, Some(pubkey)) => pubkey,
        (None, None) => return Err(JsValue::from_str("missing author/pubkey")),
    };
    Ok(RadrootsNostrEvent {
        id: envelope.id,
        author,
        created_at: envelope.created_at,
        kind: envelope.kind,
        tags: envelope.tags,
        content: envelope.content,
        sig: envelope.sig,
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = tangle_events_sync_all)]
pub fn tangle_events_sync_all(request_json: &str) -> Result<JsValue, JsValue> {
    let request = parse_request(request_json)?;
    let exec = WasmSqlExecutor::new();
    let bundle = radroots_tangle_sync_all(&exec, &request).map_err(err_js)?;
    serde_wasm_bindgen::to_value(&bundle).map_err(err_js)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = tangle_events_ingest_event)]
pub fn tangle_events_ingest_event(event_json: &str) -> Result<JsValue, JsValue> {
    let event = parse_event(event_json)?;
    let exec = WasmSqlExecutor::new();
    let factory = WasmIdFactory;
    let outcome =
        radroots_tangle_ingest_event_with_factory(&exec, &event, &factory).map_err(err_js)?;
    let value = match outcome {
        RadrootsTangleIngestOutcome::Applied => "applied",
        RadrootsTangleIngestOutcome::Skipped => "skipped",
    };
    Ok(JsValue::from_str(value))
}

#[cfg(coverage_nightly)]
pub fn coverage_branch_probe(input: bool) -> &'static str {
    if input {
        "tangle-events-wasm"
    } else {
        "tangle-events-wasm"
    }
}

#[cfg(all(test, coverage_nightly))]
mod tests {
    use super::coverage_branch_probe;

    #[test]
    fn coverage_branch_probe_hits_both_paths() {
        assert_eq!(coverage_branch_probe(true), "tangle-events-wasm");
        assert_eq!(coverage_branch_probe(false), "tangle-events-wasm");
    }
}
