#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use base64::Engine;
#[cfg(target_arch = "wasm32")]
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use radroots_events::RadrootsEventEnvelope;
use radroots_replica_sync::RadrootsReplicaSyncRequest;
#[cfg(target_arch = "wasm32")]
use radroots_replica_sync::{
    RadrootsReplicaIdFactory, RadrootsReplicaIngestOutcome,
    radroots_replica_ingest_event_with_factory, radroots_replica_sync_all,
};
#[cfg(target_arch = "wasm32")]
use radroots_sdk_sql_wasm_runtime::WasmSqlExecutor;
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
impl RadrootsReplicaIdFactory for WasmIdFactory {
    fn new_d_tag(&self) -> String {
        let uuid = Uuid::now_v7();
        URL_SAFE_NO_PAD.encode(uuid.as_bytes())
    }
}

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

pub fn parse_request_model(request_json: &str) -> Result<RadrootsReplicaSyncRequest, String> {
    serde_json::from_str(request_json).map_err(|error| error.to_string())
}

pub fn parse_event_model(event_json: &str) -> Result<RadrootsEventEnvelope, String> {
    let envelope: NostrEventEnvelope =
        serde_json::from_str(event_json).map_err(|error| error.to_string())?;
    let author = match (envelope.author, envelope.pubkey) {
        (Some(author), Some(pubkey)) if author != pubkey => {
            return Err("author/pubkey mismatch".to_owned());
        }
        (Some(author), _) => author,
        (None, Some(pubkey)) => pubkey,
        (None, None) => return Err("missing author/pubkey".to_owned()),
    };
    Ok(RadrootsEventEnvelope {
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
#[wasm_bindgen(js_name = replica_sync_sync_all)]
pub fn replica_sync_sync_all(request_json: &str) -> Result<JsValue, JsValue> {
    let request = parse_request_model(request_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let bundle = radroots_replica_sync_all(&exec, &request).map_err(err_js)?;
    serde_wasm_bindgen::to_value(&bundle).map_err(err_js)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = replica_sync_ingest_event)]
pub fn replica_sync_ingest_event(event_json: &str) -> Result<JsValue, JsValue> {
    let event = parse_event_model(event_json).map_err(err_js)?;
    let exec = WasmSqlExecutor::new();
    let factory = WasmIdFactory;
    let outcome =
        radroots_replica_ingest_event_with_factory(&exec, &event, &factory).map_err(err_js)?;
    let value = match outcome {
        RadrootsReplicaIngestOutcome::Applied => "applied",
        RadrootsReplicaIngestOutcome::Skipped => "skipped",
    };
    Ok(JsValue::from_str(value))
}

#[cfg(test)]
mod tests {
    use super::{parse_event_model, parse_request_model};

    fn event_json(author: Option<&str>, pubkey: Option<&str>) -> String {
        let mut fields = vec![
            r#""id":"event-id""#.to_owned(),
            r#""created_at":123"#.to_owned(),
            r#""kind":30023"#.to_owned(),
            r#""tags":[["d","one"]]"#.to_owned(),
            r#""content":"content""#.to_owned(),
            r#""sig":"sig""#.to_owned(),
        ];
        if let Some(author) = author {
            fields.push(format!(r#""author":"{author}""#));
        }
        if let Some(pubkey) = pubkey {
            fields.push(format!(r#""pubkey":"{pubkey}""#));
        }
        format!("{{{}}}", fields.join(","))
    }

    #[test]
    fn parse_event_accepts_matching_author_and_pubkey() {
        let event = parse_event_model(&event_json(Some("author"), Some("author"))).expect("event");
        assert_eq!(event.author, "author");
        assert_eq!(event.tags, vec![vec!["d".to_owned(), "one".to_owned()]]);
    }

    #[test]
    fn parse_event_accepts_author_without_pubkey() {
        let event = parse_event_model(&event_json(Some("author"), None)).expect("event");
        assert_eq!(event.author, "author");
    }

    #[test]
    fn parse_event_accepts_pubkey_without_author() {
        let event = parse_event_model(&event_json(None, Some("pubkey"))).expect("event");
        assert_eq!(event.author, "pubkey");
    }

    #[test]
    fn parse_event_rejects_author_pubkey_mismatch() {
        let error =
            parse_event_model(&event_json(Some("author"), Some("pubkey"))).expect_err("error");
        assert_eq!(error, "author/pubkey mismatch");
    }

    #[test]
    fn parse_event_rejects_missing_author_and_pubkey() {
        let error = parse_event_model(&event_json(None, None)).expect_err("error");
        assert_eq!(error, "missing author/pubkey");
    }

    #[test]
    fn parse_event_rejects_malformed_json() {
        let error = parse_event_model("{").expect_err("error");
        assert!(error.contains("EOF"));
    }

    #[test]
    fn parse_request_rejects_malformed_json() {
        let error = parse_request_model("{").expect_err("error");
        assert!(error.contains("EOF"));
    }
}
