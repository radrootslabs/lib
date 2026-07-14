use super::*;
use radroots_event::wire::RadrootsNip01EventWire;
use radroots_transport::RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI;
use radroots_transport_publish_protocol::{
    NostrPublishTargetSourcePolicy, TransportPublishDeliveryPolicy, TransportPublishEventRequest,
    TransportPublishEventResponse, TransportPublishJobStatus, TransportPublishJobView,
    TransportPublishOutcomeKind, TransportPublishPreviewBehavior, TransportPublishTarget,
    TransportPublishTargetOutcome, TransportPublishTargetPolicy, TransportPublishTargetSource,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

struct RecordedHttpRequest {
    request_line: String,
    headers: Vec<(String, String)>,
    body: String,
}

fn spawn_http_server(
    status: &str,
    response_body: &str,
) -> (String, JoinHandle<RecordedHttpRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let endpoint = format!("http://{}/rpc", listener.local_addr().expect("addr"));
    let status = status.to_owned();
    let response_body = response_body.to_owned();
    let content_length = response_body.len();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                let headers_end = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .expect("headers end")
                    + 4;
                let header_text = String::from_utf8_lossy(&request[..headers_end]);
                let request_content_length = header_text
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().expect("content length"))
                    })
                    .unwrap_or(0);
                while request.len() < headers_end + request_content_length {
                    let read = stream.read(&mut buffer).expect("read body");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
                break;
            }
        }
        let request_text = String::from_utf8_lossy(&request);
        let (headers_text, body) = request_text.split_once("\r\n\r\n").expect("request body");
        let mut header_lines = headers_text.lines();
        let request_line = header_lines.next().expect("request line").to_owned();
        let headers = header_lines
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.to_ascii_lowercase(), value.trim().to_owned()))
            })
            .collect::<Vec<_>>();
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {content_length}\r\nconnection: close\r\n\r\n{response_body}",
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        RecordedHttpRequest {
            request_line,
            headers,
            body: body.to_owned(),
        }
    });
    (endpoint, handle)
}

fn signed_event() -> RadrootsSignedEvent {
    let mut wire = RadrootsNip01EventWire {
        id: String::new(),
        pubkey: "b".repeat(64),
        created_at: 1_700_000_000,
        kind: 30_402,
        tags: vec![vec!["d".to_owned(), "listing-1".to_owned()]],
        content: "{\"name\":\"carrots\"}".to_owned(),
        sig: "c".repeat(128),
        extra: Default::default(),
    };
    wire.id = wire.computed_event_id().expect("event id").into_string();
    let raw_json = serde_json::to_string(&wire).expect("raw event json");
    RadrootsSignedEvent::from_wire_verified_id(wire, raw_json).expect("signed event")
}

fn publish_request() -> TransportPublishEventRequest {
    TransportPublishEventRequest {
        raw_event_json: signed_event().raw_json().to_owned(),
        target_policy: TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            vec!["wss://relay.example.com".to_owned()],
        ),
        delivery_policy: TransportPublishDeliveryPolicy::Any,
        idempotency_key: Some("idem-1".to_owned()),
        timeout_ms: Some(10_000),
    }
}

fn job_status_for_outcome(outcome_kind: TransportPublishOutcomeKind) -> TransportPublishJobStatus {
    if outcome_kind.counts_toward_accepted_delivery() {
        TransportPublishJobStatus::DeliverySatisfied
    } else if outcome_kind.is_retryable() {
        TransportPublishJobStatus::DeliveryUnsatisfiedRetryable
    } else if outcome_kind.is_terminal_failure() {
        TransportPublishJobStatus::DeliveryUnsatisfiedTerminal
    } else if outcome_kind == TransportPublishOutcomeKind::DeferredUntilImplemented {
        TransportPublishJobStatus::DeliveryDeferred
    } else if outcome_kind == TransportPublishOutcomeKind::PreviewUnavailable {
        TransportPublishJobStatus::DeliveryPreviewUnavailable
    } else {
        TransportPublishJobStatus::DeliveryUnsatisfiedRetryable
    }
}

fn job(outcome_kind: TransportPublishOutcomeKind) -> TransportPublishJobView {
    let status = job_status_for_outcome(outcome_kind);
    TransportPublishJobView {
        job_id: "job-1".to_owned(),
        status,
        terminal: matches!(
            status,
            TransportPublishJobStatus::DeliverySatisfied
                | TransportPublishJobStatus::DeliveryUnsatisfiedTerminal
                | TransportPublishJobStatus::DeliveryDeferred
                | TransportPublishJobStatus::DeliveryPreviewUnavailable
                | TransportPublishJobStatus::Rejected
        ),
        delivery_satisfied: status == TransportPublishJobStatus::DeliverySatisfied,
        event_id: "a".repeat(64),
        pubkey: "b".repeat(64),
        event_kind: 30_402,
        target_policy: TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            vec!["wss://relay.example.com".to_owned()],
        ),
        delivery_policy: TransportPublishDeliveryPolicy::Any,
        target_count: 1,
        acknowledged_count: usize::from(outcome_kind.counts_toward_accepted_delivery()),
        retryable_count: usize::from(outcome_kind.is_retryable()),
        terminal_count: usize::from(outcome_kind.is_terminal_failure()),
        requested_at_ms: 1_700_000_000_000,
        completed_at_ms: Some(1_700_000_000_100),
        last_error: None,
        targets: vec![TransportPublishTargetOutcome {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: "wss://relay.example.com".to_owned(),
            target_scope: None,
            target_label: None,
            source: TransportPublishTargetSource::Request,
            attempted: true,
            outcome_kind,
            message: Some("relay outcome".to_owned()),
            latency_ms: Some(7),
        }],
    }
}

fn explicit_nostr_job(
    endpoints: Vec<String>,
    delivery_policy: TransportPublishDeliveryPolicy,
) -> TransportPublishJobView {
    let targets = endpoints
        .iter()
        .map(|endpoint| TransportPublishTargetOutcome {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: endpoint.clone(),
            target_scope: None,
            target_label: None,
            source: TransportPublishTargetSource::Request,
            attempted: true,
            outcome_kind: TransportPublishOutcomeKind::Accepted,
            message: Some("relay outcome".to_owned()),
            latency_ms: Some(7),
        })
        .collect::<Vec<_>>();
    TransportPublishJobView {
        job_id: "job-explicit-nostr".to_owned(),
        status: TransportPublishJobStatus::DeliverySatisfied,
        terminal: true,
        delivery_satisfied: true,
        event_id: "a".repeat(64),
        pubkey: "b".repeat(64),
        event_kind: 30_402,
        target_policy: TransportPublishTargetPolicy::explicit_targets(
            endpoints
                .iter()
                .map(|endpoint| TransportPublishTarget::nostr(endpoint.as_str()))
                .collect::<Vec<_>>(),
        ),
        delivery_policy,
        target_count: targets.len(),
        acknowledged_count: targets.len(),
        retryable_count: 0,
        terminal_count: 0,
        requested_at_ms: 1_700_000_000_000,
        completed_at_ms: Some(1_700_000_000_100),
        last_error: None,
        targets,
    }
}

fn reticulum_deferred_job() -> TransportPublishJobView {
    TransportPublishJobView {
        job_id: "job-reticulum".to_owned(),
        status: TransportPublishJobStatus::DeliveryDeferred,
        terminal: true,
        delivery_satisfied: false,
        event_id: "a".repeat(64),
        pubkey: "b".repeat(64),
        event_kind: 30_402,
        target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
            TransportPublishTarget::reticulum_preview(
                TransportPublishPreviewBehavior::DeferDeliveryPlans,
            ),
        ]),
        delivery_policy: TransportPublishDeliveryPolicy::Any,
        target_count: 1,
        acknowledged_count: 0,
        retryable_count: 0,
        terminal_count: 0,
        requested_at_ms: 1_700_000_000_000,
        completed_at_ms: Some(1_700_000_000_100),
        last_error: Some("delivery_deferred_until_implemented".to_owned()),
        targets: vec![TransportPublishTargetOutcome {
            transport_kind: "reticulum".to_owned(),
            endpoint_uri: RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI.to_owned(),
            target_scope: None,
            target_label: None,
            source: TransportPublishTargetSource::ReticulumPreview,
            attempted: false,
            outcome_kind: TransportPublishOutcomeKind::DeferredUntilImplemented,
            message: Some("reticulum preview unavailable".to_owned()),
            latency_ms: None,
        }],
    }
}

fn publish_response_json_for_job(job: TransportPublishJobView) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": SDK_RADROOTSD_PROXY_REQUEST_ID,
        "result": {
            "deduplicated": false,
            "job": job
        }
    })
    .to_string()
}

fn publish_response_json() -> String {
    publish_response_json_for_job(job(TransportPublishOutcomeKind::Accepted))
}

fn reticulum_deferred_response_json() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": SDK_RADROOTSD_PROXY_REQUEST_ID,
        "result": {
            "deduplicated": false,
            "job": reticulum_deferred_job()
        }
    })
    .to_string()
}

fn assert_message(error: RadrootsdError, fragment: &str) {
    let message = error.to_string();
    assert!(
        message.contains(fragment),
        "expected {message:?} to contain {fragment:?}"
    );
}

#[test]
fn auth_headers_omit_or_redact_bearer_authorization() {
    let none = auth_headers(&RadrootsdAuth::None).expect("none auth");
    assert!(!none.contains_key(AUTHORIZATION));
    assert_eq!(format!("{:?}", RadrootsdAuth::None), "None");

    let bearer = auth_headers(&RadrootsdAuth::BearerToken("sdk-token".into())).expect("bearer");
    assert_eq!(
        bearer
            .get(AUTHORIZATION)
            .expect("authorization")
            .to_str()
            .expect("authorization str"),
        "Bearer sdk-token"
    );

    let error = auth_headers(&RadrootsdAuth::BearerToken("bad\ntoken".into())).expect_err("error");
    assert!(matches!(error, RadrootsdError::InvalidAuthHeader(_)));
    assert_eq!(
        format!("{:?}", RadrootsdAuth::BearerToken("token-secret".into())),
        "BearerToken(<redacted>)"
    );
    assert!(
        RadrootsdError::InvalidAuthHeader("bad header".to_owned())
            .to_string()
            .contains("invalid radrootsd bearer token header")
    );
    assert_eq!(
        RadrootsdError::InvalidRequest("invalid request".to_owned()).to_string(),
        "invalid request"
    );
    assert_eq!(
        RadrootsdError::Http("http failed".to_owned()).to_string(),
        "http failed"
    );
    assert_eq!(
        RadrootsdError::MalformedResponse("bad envelope".to_owned()).to_string(),
        "bad envelope"
    );
}

#[test]
fn proxy_config_builders_preserve_typed_runtime_options() {
    let config = RadrootsdProxyConfig::new("http://127.0.0.1:8080/rpc")
        .with_auth(RadrootsdAuth::BearerToken("sdk-token".to_owned()))
        .with_timeout(Duration::from_millis(250))
        .with_request_timeout_ms(1_500);
    let adapter = RadrootsdProxyPublishAdapter::new(config.clone());

    assert_eq!(adapter.config(), &config);
    assert_eq!(adapter.config().endpoint, "http://127.0.0.1:8080/rpc");
    assert_eq!(
        adapter.config().auth,
        RadrootsdAuth::BearerToken("sdk-token".to_owned())
    );
    assert_eq!(adapter.config().timeout, Duration::from_millis(250));
    assert_eq!(adapter.config().request_timeout_ms, Some(1_500));
}

#[test]
fn publish_event_request_json_uses_signed_event_contract() {
    let value = publish_event_request_json(&publish_request()).expect("request json");

    let raw_event_json = value["raw_event_json"].as_str().expect("raw event json");
    let raw_event: serde_json::Value = serde_json::from_str(raw_event_json).expect("raw event");
    assert_eq!(raw_event["id"], "a".repeat(64));
    assert_eq!(raw_event["pubkey"], "b".repeat(64));
    assert_eq!(raw_event["kind"], 30_402);
    assert_eq!(value["target_policy"]["kind"], "nostr");
    assert_eq!(
        value["target_policy"]["source_policy"],
        "request_then_author_write_then_daemon_default"
    );
    assert_eq!(
        value["target_policy"]["relay_urls"][0],
        "wss://relay.example.com"
    );
    assert_eq!(value["delivery_policy"]["mode"], "any");
    assert_eq!(value["idempotency_key"], "idem-1");
    let rendered = value.to_string();
    assert!(!rendered.contains("signer_session_id"));
    assert!(!rendered.contains("bridge."));
}

#[test]
fn decode_jsonrpc_response_validates_envelope_and_errors() {
    let response: TransportPublishEventResponse = decode_jsonrpc_response(
        METHOD_EVENT,
        SDK_RADROOTSD_PROXY_REQUEST_ID,
        publish_response_json().as_str(),
    )
    .expect("response");
    assert_eq!(response.job.event_id, "a".repeat(64));

    let error = decode_jsonrpc_response::<TransportPublishEventResponse>(
        METHOD_EVENT,
        SDK_RADROOTSD_PROXY_REQUEST_ID,
        r#"{"jsonrpc":"2.0","id":"radroots-sdk-transport-publish-event","error":{"code":-32001,"message":"principal unauthorized"}}"#,
    )
    .expect_err("jsonrpc error");
    assert!(matches!(
        error,
        RadrootsdError::JsonRpc { code: -32001, .. }
    ));
    assert_message(error, "principal unauthorized");

    assert!(matches!(
        decode_jsonrpc_response::<serde_json::Value>(METHOD_EVENT, "expected", "not json"),
        Err(RadrootsdError::MalformedResponse(_))
    ));
    assert!(matches!(
        decode_jsonrpc_response::<serde_json::Value>(
            METHOD_EVENT,
            "expected",
            r#"{"jsonrpc":"2.0","id":"other","result":{}}"#
        ),
        Err(RadrootsdError::MalformedResponse(_))
    ));
    assert!(matches!(
        decode_jsonrpc_response::<serde_json::Value>(
            METHOD_EVENT,
            "expected",
            r#"{"jsonrpc":"2.0","id":"expected"}"#
        ),
        Err(RadrootsdError::MalformedResponse(_))
    ));
    assert!(matches!(
        decode_jsonrpc_response::<serde_json::Value>(
            METHOD_EVENT,
            "expected",
            r#"{"jsonrpc":"1.0","id":"expected","result":{}}"#
        ),
        Err(RadrootsdError::MalformedResponse(_))
    ));
    assert!(matches!(
        decode_jsonrpc_response::<serde_json::Value>(
            METHOD_EVENT,
            "expected",
            r#"{"jsonrpc":"2.0","id":"expected","result":{},"error":{"code":-32002,"message":"both"}}"#
        ),
        Err(RadrootsdError::MalformedResponse(_))
    ));
}

#[tokio::test]
async fn publish_event_posts_transport_publish_jsonrpc() {
    let (endpoint, handle) = spawn_http_server("200 OK", publish_response_json().as_str());

    let receipt = publish_event(
        endpoint.as_str(),
        &RadrootsdAuth::BearerToken("sdk-token".into()),
        &publish_request(),
        Duration::from_secs(2),
    )
    .await
    .expect("publish");

    assert_eq!(receipt.job.event_id, "a".repeat(64));
    let recorded = handle.join().expect("server thread");
    assert_eq!(recorded.request_line, "POST /rpc HTTP/1.1");
    assert!(
        recorded
            .headers
            .iter()
            .any(|(name, value)| name == "authorization" && value == "Bearer sdk-token")
    );
    let body: serde_json::Value = serde_json::from_str(recorded.body.as_str()).expect("body");
    assert_eq!(body["method"], METHOD_EVENT);
    assert_eq!(body["id"], SDK_RADROOTSD_PROXY_REQUEST_ID);
    let raw_event_json = body["params"]["raw_event_json"]
        .as_str()
        .expect("raw event json");
    let raw_event: serde_json::Value = serde_json::from_str(raw_event_json).expect("raw event");
    assert_eq!(raw_event["content"], "{\"name\":\"carrots\"}");
    assert_eq!(body["params"]["target_policy"]["kind"], "nostr");
    assert_eq!(
        body["params"]["target_policy"]["relay_urls"][0],
        "wss://relay.example.com"
    );
}

#[tokio::test]
async fn publish_signed_event_posts_typed_proxy_request() {
    let mut response_job = explicit_nostr_job(
        vec!["wss://relay.example.com".to_owned()],
        TransportPublishDeliveryPolicy::All,
    );
    response_job.target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
        TransportPublishTarget::nostr("wss://relay.example.com")
            .with_scope("farm.local")
            .with_label("Farm relay"),
    ]);
    response_job.targets[0].target_scope = Some("farm.local".to_owned());
    response_job.targets[0].target_label = Some("Farm relay".to_owned());
    let response_json = publish_response_json_for_job(response_job);
    let (endpoint, handle) = spawn_http_server("200 OK", response_json.as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(
        RadrootsdProxyConfig::new(endpoint)
            .with_auth(RadrootsdAuth::BearerToken("sdk-token".into()))
            .with_request_timeout_ms(7_000),
    );

    let receipt = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com")
                    .with_scope("farm.local")
                    .with_label("Farm relay"),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::All,
            idempotency_key: Some("idem-typed".to_owned()),
            timeout_ms: adapter.config().request_timeout_ms,
        })
        .await
        .expect("typed publish");

    assert!(receipt.job.delivery_satisfied);
    assert_eq!(
        receipt.job.targets[0].target_scope.as_deref(),
        Some("farm.local")
    );
    assert_eq!(
        receipt.job.targets[0].target_label.as_deref(),
        Some("Farm relay")
    );
    let recorded = handle.join().expect("server thread");
    assert!(
        recorded
            .headers
            .iter()
            .any(|(name, value)| name == "authorization" && value == "Bearer sdk-token")
    );
    let body: serde_json::Value = serde_json::from_str(recorded.body.as_str()).expect("body");
    assert_eq!(body["params"]["delivery_policy"]["mode"], "all");
    assert_eq!(body["params"]["target_policy"]["kind"], "explicit_targets");
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["endpoint_uri"],
        "wss://relay.example.com"
    );
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["target_scope"],
        "farm.local"
    );
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["target_label"],
        "Farm relay"
    );
    assert_eq!(body["params"]["idempotency_key"], "idem-typed");
    assert_eq!(body["params"]["timeout_ms"], 7_000);
}

#[tokio::test]
async fn publish_signed_event_preserves_typed_reticulum_preview_behavior() {
    let response_json = reticulum_deferred_response_json();
    let (endpoint, handle) = spawn_http_server("200 OK", response_json.as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

    let response = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum_preview(
                    TransportPublishPreviewBehavior::DeferDeliveryPlans,
                ),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: Some("idem-reticulum".to_owned()),
            timeout_ms: None,
        })
        .await
        .expect("typed Reticulum publish request");
    assert_eq!(
        response.job.status,
        TransportPublishJobStatus::DeliveryDeferred
    );
    assert!(!response.job.delivery_satisfied);

    let recorded = handle.join().expect("server thread");
    let body: serde_json::Value = serde_json::from_str(recorded.body.as_str()).expect("body");
    assert_eq!(body["params"]["target_policy"]["kind"], "explicit_targets");
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["transport_kind"],
        "reticulum"
    );
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["endpoint_uri"],
        RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
    );
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["preview_behavior"],
        "defer_delivery_plans"
    );
}

#[tokio::test]
async fn publish_signed_event_rejects_mismatched_daemon_event_identity() {
    let mut response_job = job(TransportPublishOutcomeKind::Accepted);
    response_job.event_id = "0".repeat(64);
    let response_json = publish_response_json_for_job(response_job);
    let (endpoint, _handle) = spawn_http_server("200 OK", response_json.as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

    let error = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::nostr(
                NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
                vec!["wss://relay.example.com".to_owned()],
            ),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: Some("idem-mismatch".to_owned()),
            timeout_ms: None,
        })
        .await
        .expect_err("mismatched response");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(error, "event_id");
}

#[tokio::test]
async fn publish_signed_event_rejects_mismatched_daemon_pubkey_and_kind() {
    for (field, response_job) in [
        {
            let mut response_job = job(TransportPublishOutcomeKind::Accepted);
            response_job.pubkey = "0".repeat(64);
            ("pubkey", response_job)
        },
        {
            let mut response_job = job(TransportPublishOutcomeKind::Accepted);
            response_job.event_kind = 30_403;
            ("event_kind", response_job)
        },
    ] {
        let response_json = publish_response_json_for_job(response_job);
        let (endpoint, _handle) = spawn_http_server("200 OK", response_json.as_str());
        let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

        let error = adapter
            .publish_signed_event(RadrootsdProxyPublishRequest {
                signed_event: signed_event(),
                target_policy: TransportPublishTargetPolicy::nostr(
                    NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
                    vec!["wss://relay.example.com".to_owned()],
                ),
                delivery_policy: TransportPublishDeliveryPolicy::Any,
                idempotency_key: Some(format!("idem-mismatch-{field}")),
                timeout_ms: None,
            })
            .await
            .expect_err("mismatched response");

        assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
        assert_message(error, field);
    }
}

#[tokio::test]
async fn publish_signed_event_rejects_mismatched_daemon_delivery_policy() {
    let mut response_job = job(TransportPublishOutcomeKind::Accepted);
    response_job.delivery_policy = TransportPublishDeliveryPolicy::All;
    let response_json = publish_response_json_for_job(response_job);
    let (endpoint, _handle) = spawn_http_server("200 OK", response_json.as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

    let error = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::nostr(
                NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
                vec!["wss://relay.example.com".to_owned()],
            ),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: Some("idem-delivery-policy-mismatch".to_owned()),
            timeout_ms: None,
        })
        .await
        .expect_err("delivery policy mismatch");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(error, "delivery_policy");
}

#[tokio::test]
async fn publish_signed_event_rejects_mismatched_explicit_target_response() {
    let (endpoint, _handle) = spawn_http_server("200 OK", publish_response_json().as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

    let error = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::reticulum_preview(
                    TransportPublishPreviewBehavior::DeferDeliveryPlans,
                ),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: Some("idem-target-mismatch".to_owned()),
            timeout_ms: None,
        })
        .await
        .expect_err("target mismatch");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(error, "target_policy");
}

#[tokio::test]
async fn publish_signed_event_accepts_reordered_explicit_target_outcomes() {
    let mut response_job = explicit_nostr_job(
        vec![
            "wss://relay-a.example.com".to_owned(),
            "wss://relay-b.example.com".to_owned(),
        ],
        TransportPublishDeliveryPolicy::Any,
    );
    response_job.targets.reverse();
    let response_json = publish_response_json_for_job(response_job);
    let (endpoint, _handle) = spawn_http_server("200 OK", response_json.as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

    let response = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay-a.example.com"),
                TransportPublishTarget::nostr("wss://relay-b.example.com"),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: Some("idem-explicit-reordered-outcomes".to_owned()),
            timeout_ms: None,
        })
        .await
        .expect("reordered explicit target outcomes");

    assert!(response.job.delivery_satisfied);
    assert_eq!(
        response.job.targets[0].endpoint_uri,
        "wss://relay-b.example.com"
    );
    assert_eq!(
        response.job.targets[1].endpoint_uri,
        "wss://relay-a.example.com"
    );
}

#[tokio::test]
async fn publish_signed_event_rejects_mismatched_explicit_target_outcomes() {
    let mut response_job = explicit_nostr_job(
        vec!["wss://relay.example.com".to_owned()],
        TransportPublishDeliveryPolicy::Any,
    );
    response_job.targets[0].endpoint_uri = "wss://relay-other.example.com".to_owned();
    let response_json = publish_response_json_for_job(response_job);
    let (endpoint, _handle) = spawn_http_server("200 OK", response_json.as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

    let error = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com"),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: Some("idem-explicit-outcome-mismatch".to_owned()),
            timeout_ms: None,
        })
        .await
        .expect_err("explicit target outcome mismatch");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(error, "explicit target policy");
}

#[tokio::test]
async fn publish_signed_event_rejects_mismatched_scoped_explicit_target_outcomes() {
    let mut response_job = explicit_nostr_job(
        vec!["wss://relay.example.com".to_owned()],
        TransportPublishDeliveryPolicy::Any,
    );
    response_job.target_policy = TransportPublishTargetPolicy::explicit_targets(vec![
        TransportPublishTarget::nostr("wss://relay.example.com").with_scope("farm.local"),
    ]);
    response_job.targets[0].target_scope = Some("farm.remote".to_owned());
    let response_json = publish_response_json_for_job(response_job);
    let (endpoint, _handle) = spawn_http_server("200 OK", response_json.as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

    let error = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com").with_scope("farm.local"),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::Any,
            idempotency_key: Some("idem-scoped-explicit-outcome-mismatch".to_owned()),
            timeout_ms: None,
        })
        .await
        .expect_err("scoped explicit target outcome mismatch");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(error, "explicit target policy");
}

#[tokio::test]
async fn publish_event_http_errors_omit_body_and_token_material() {
    let body = "{\"error\":\"token-secret content carrots\"}";
    let (endpoint, _handle) = spawn_http_server("503 Service Unavailable", body);

    let error = publish_event(
        endpoint.as_str(),
        &RadrootsdAuth::BearerToken("token-secret".into()),
        &publish_request(),
        Duration::from_secs(2),
    )
    .await
    .expect_err("http error");
    let message = error.to_string();

    assert!(message.contains("503"));
    assert!(message.contains("response body omitted"));
    assert!(!message.contains("token-secret"));
    assert!(!message.contains("carrots"));
}

#[tokio::test]
async fn publish_event_empty_http_error_reports_empty_body() {
    let (endpoint, _handle) = spawn_http_server("500 Internal Server Error", "");

    let error = publish_event(
        endpoint.as_str(),
        &RadrootsdAuth::None,
        &publish_request(),
        Duration::from_secs(2),
    )
    .await
    .expect_err("http error");

    assert!(error.to_string().contains("response body empty"));
}

#[tokio::test]
async fn publish_signed_event_rejects_invalid_target_requests_before_http() {
    let adapter =
        RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc"));
    let base = RadrootsdProxyPublishRequest {
        signed_event: signed_event(),
        target_policy: TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            vec!["wss://relay.example.com".to_owned()],
        ),
        delivery_policy: TransportPublishDeliveryPolicy::Any,
        idempotency_key: Some("idem-1".to_owned()),
        timeout_ms: Some(1_000),
    };

    let mut invalid_quorum = base.clone();
    invalid_quorum.delivery_policy = TransportPublishDeliveryPolicy::Quorum { quorum: 0 };
    let mut too_many_targets = base.clone();
    too_many_targets.target_policy = TransportPublishTargetPolicy::nostr(
        NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
        (0..=SDK_RADROOTSD_PROXY_MAX_TARGETS)
            .map(|index| format!("wss://relay-{index}.example.com"))
            .collect(),
    );
    let mut empty_endpoint_uri = base.clone();
    empty_endpoint_uri.target_policy = TransportPublishTargetPolicy::nostr(
        NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
        vec![" ".to_owned()],
    );
    let mut nostr_preview_behavior = base.clone();
    nostr_preview_behavior.target_policy =
        TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: "wss://relay.example.com".to_owned(),
            target_scope: None,
            target_label: None,
            preview_behavior: Some(TransportPublishPreviewBehavior::RejectDeliveryAttempts),
        }]);
    let mut explicit_proxy_target = base.clone();
    explicit_proxy_target.target_policy =
        TransportPublishTargetPolicy::explicit_targets(vec![TransportPublishTarget {
            transport_kind: "proxy".to_owned(),
            endpoint_uri: "radrootsd-proxy:publish".to_owned(),
            target_scope: None,
            target_label: None,
            preview_behavior: None,
        }]);
    let proxy_error = adapter
        .publish_signed_event(explicit_proxy_target)
        .await
        .expect_err("explicit proxy target");
    assert!(matches!(
        proxy_error,
        RadrootsdError::InvalidRequest(message)
            if message.contains("proxy") && message.contains("daemon explicit target")
    ));
    let mut empty_idempotency = base;
    empty_idempotency.idempotency_key = Some(" ".to_owned());

    for request in [
        invalid_quorum,
        too_many_targets,
        empty_endpoint_uri,
        nostr_preview_behavior,
        empty_idempotency,
    ] {
        assert!(matches!(
            adapter.publish_signed_event(request).await,
            Err(RadrootsdError::InvalidRequest(_))
        ));
    }
}

#[tokio::test]
async fn adapter_rejects_invalid_request_before_transport() {
    let adapter =
        RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc"));
    let request = RadrootsdProxyPublishRequest {
        signed_event: signed_event(),
        target_policy: TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            Vec::new(),
        ),
        delivery_policy: TransportPublishDeliveryPolicy::Quorum { quorum: 0 },
        idempotency_key: None,
        timeout_ms: None,
    };

    let error = adapter
        .publish_signed_event(request)
        .await
        .expect_err("invalid request");

    assert!(matches!(error, RadrootsdError::InvalidRequest(_)));
}
