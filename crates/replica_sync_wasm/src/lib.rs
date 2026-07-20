#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use base64::Engine;
#[cfg(target_arch = "wasm32")]
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use radroots_event::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts};
#[cfg(any(target_arch = "wasm32", test))]
use radroots_replica_sync::RadrootsReplicaIngestOutcome;
use radroots_replica_sync::RadrootsReplicaSyncRequest;
#[cfg(target_arch = "wasm32")]
use radroots_replica_sync::{
    RadrootsReplicaIdFactory, radroots_replica_ingest_event_with_factory, radroots_replica_sync_all,
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
#[serde(deny_unknown_fields)]
struct EventEnvelopeInput {
    id: String,
    author: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

pub fn parse_request_model(request_json: &str) -> Result<RadrootsReplicaSyncRequest, String> {
    serde_json::from_str(request_json).map_err(|error| error.to_string())
}

pub fn parse_event_model(event_json: &str) -> Result<RadrootsEventEnvelope, String> {
    let envelope: EventEnvelopeInput =
        serde_json::from_str(event_json).map_err(|error| error.to_string())?;
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: envelope.id,
        author: envelope.author,
        created_at: envelope.created_at,
        kind: envelope.kind,
        tags: envelope.tags,
        content: envelope.content,
        sig: envelope.sig,
    })
    .map_err(|error| error.to_string())
}

#[cfg(any(target_arch = "wasm32", test))]
fn ingest_outcome_label(outcome: RadrootsReplicaIngestOutcome) -> &'static str {
    match outcome {
        RadrootsReplicaIngestOutcome::Applied => "applied",
        RadrootsReplicaIngestOutcome::Excluded => "excluded",
        RadrootsReplicaIngestOutcome::Rejected => "rejected",
        RadrootsReplicaIngestOutcome::Skipped => "skipped",
    }
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
    Ok(JsValue::from_str(ingest_outcome_label(outcome)))
}

#[cfg(test)]
mod tests {
    use super::{ingest_outcome_label, parse_event_model, parse_request_model};
    use radroots_replica_sync::RadrootsReplicaIngestOutcome;

    fn event_json(author: Option<&str>, pubkey: Option<&str>) -> String {
        let mut fields = vec![
            format!(r#""id":"{}""#, "0".repeat(64)),
            r#""created_at":123"#.to_owned(),
            r#""kind":30023"#.to_owned(),
            r#""tags":[["d","one"]]"#.to_owned(),
            r#""content":"content""#.to_owned(),
            format!(r#""sig":"{}""#, "f".repeat(128)),
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
    fn parse_event_accepts_author_domain_envelope() {
        let author = "a".repeat(64);
        let event = parse_event_model(&event_json(Some(author.as_str()), None)).expect("event");
        assert_eq!(event.author_str(), author);
        assert_eq!(
            event.tags_as_vec(),
            vec![vec!["d".to_owned(), "one".to_owned()]]
        );
    }

    #[test]
    fn parse_event_rejects_pubkey_wire_alias() {
        let author = "a".repeat(64);
        let error = parse_event_model(&event_json(Some(author.as_str()), Some(author.as_str())))
            .expect_err("error");
        assert!(error.contains("unknown field `pubkey`"));
    }

    #[test]
    fn parse_event_rejects_missing_author() {
        let error = parse_event_model(&event_json(None, None)).expect_err("error");
        assert!(error.contains("missing field `author`"));
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

    #[test]
    fn ingest_outcome_labels_cover_the_replica_contract() {
        assert_eq!(
            ingest_outcome_label(RadrootsReplicaIngestOutcome::Applied),
            "applied"
        );
        assert_eq!(
            ingest_outcome_label(RadrootsReplicaIngestOutcome::Excluded),
            "excluded"
        );
        assert_eq!(
            ingest_outcome_label(RadrootsReplicaIngestOutcome::Rejected),
            "rejected"
        );
        assert_eq!(
            ingest_outcome_label(RadrootsReplicaIngestOutcome::Skipped),
            "skipped"
        );
    }
}
