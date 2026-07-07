use super::*;
use radroots_transport_nostr::{
    RadrootsRelayPublishRequest, RadrootsRelayTargetSet, RadrootsRelayUrlPolicy,
};
use radroots_transport_publish_protocol::{
    NostrPublishTargetSourcePolicy, RETICULUM_PREVIEW_ENDPOINT_URI, TransportPublishDeliveryPolicy,
    TransportPublishEventRequest, TransportPublishEventResponse, TransportPublishJobStatus,
    TransportPublishJobView, TransportPublishOutcomeKind, TransportPublishPreviewBehavior,
    TransportPublishTarget, TransportPublishTargetOutcome, TransportPublishTargetPolicy,
    TransportPublishTargetSource,
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

fn signed_event() -> RadrootsSignedNostrEvent {
    RadrootsSignedNostrEvent {
        id: "a".repeat(64),
        pubkey: "b".repeat(64),
        created_at: 1_700_000_000,
        kind: 30_402,
        tags: vec![vec!["d".to_owned(), "listing-1".to_owned()]],
        content: "{\"name\":\"carrots\"}".to_owned(),
        sig: "c".repeat(128),
        raw_json: serde_json::json!({
            "id": "a".repeat(64),
            "pubkey": "b".repeat(64),
            "created_at": 1_700_000_000u32,
            "kind": 30402u32,
            "tags": [["d", "listing-1"]],
            "content": "{\"name\":\"carrots\"}",
            "sig": "c".repeat(128)
        })
        .to_string(),
    }
}

fn publish_request() -> TransportPublishEventRequest {
    TransportPublishEventRequest {
        event: signed_event_wire(&signed_event()),
        target_policy: TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            vec!["wss://relay.example.com".to_owned()],
        ),
        delivery_policy: TransportPublishDeliveryPolicy::Any,
        idempotency_key: Some("idem-1".to_owned()),
        timeout_ms: Some(10_000),
    }
}

fn job(outcome_kind: TransportPublishOutcomeKind) -> TransportPublishJobView {
    TransportPublishJobView {
        job_id: "job-1".to_owned(),
        status: TransportPublishJobStatus::DeliverySatisfied,
        terminal: true,
        delivery_satisfied: true,
        event_id: "a".repeat(64),
        pubkey: "b".repeat(64),
        event_kind: 30_402,
        target_policy: TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            vec!["wss://relay.example.com".to_owned()],
        ),
        delivery_policy: TransportPublishDeliveryPolicy::Any,
        target_count: 1,
        acknowledged_count: usize::from(outcome_kind.counts_toward_satisfaction()),
        retryable_count: usize::from(outcome_kind.is_retryable()),
        terminal_count: usize::from(outcome_kind.is_terminal_failure()),
        requested_at_ms: 1_700_000_000_000,
        completed_at_ms: Some(1_700_000_000_100),
        last_error: None,
        targets: vec![TransportPublishTargetOutcome {
            transport_kind: "nostr".to_owned(),
            endpoint_uri: "wss://relay.example.com".to_owned(),
            source: TransportPublishTargetSource::Request,
            attempted: true,
            outcome_kind,
            message: Some("relay outcome".to_owned()),
            latency_ms: Some(7),
        }],
    }
}

fn publish_response_json() -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": SDK_RADROOTSD_PROXY_REQUEST_ID,
        "result": {
            "deduplicated": false,
            "job": job(TransportPublishOutcomeKind::Accepted)
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

    assert_eq!(value["event"]["id"], "a".repeat(64));
    assert_eq!(value["event"]["pubkey"], "b".repeat(64));
    assert_eq!(value["event"]["kind"], 30_402);
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

#[test]
fn daemon_outcomes_map_to_relay_transport_receipts() {
    let payment = proxy_relay_receipt_from_response(TransportPublishEventResponse {
        deduplicated: false,
        job: job(TransportPublishOutcomeKind::PaymentRequired),
    })
    .expect("payment receipt");
    assert_eq!(
        payment.relays[0].outcome.kind,
        RadrootsRelayOutcomeKind::PaymentRequired
    );
    assert_eq!(payment.terminal_count, 1);

    let skipped = proxy_relay_receipt_from_response(TransportPublishEventResponse {
        deduplicated: true,
        job: job(TransportPublishOutcomeKind::SkippedAlreadyAccepted),
    })
    .expect("skipped receipt");
    assert_eq!(
        skipped.relays[0].outcome.kind,
        RadrootsRelayOutcomeKind::SkippedAlreadyAccepted
    );
    assert!(skipped.quorum_met);

    let cases = [
        (
            TransportPublishOutcomeKind::Accepted,
            RadrootsRelayOutcomeKind::Accepted,
        ),
        (
            TransportPublishOutcomeKind::DuplicateAccepted,
            RadrootsRelayOutcomeKind::DuplicateAccepted,
        ),
        (
            TransportPublishOutcomeKind::Blocked,
            RadrootsRelayOutcomeKind::Blocked,
        ),
        (
            TransportPublishOutcomeKind::RateLimited,
            RadrootsRelayOutcomeKind::RateLimited,
        ),
        (
            TransportPublishOutcomeKind::Invalid,
            RadrootsRelayOutcomeKind::Invalid,
        ),
        (
            TransportPublishOutcomeKind::PowRequired,
            RadrootsRelayOutcomeKind::PowRequired,
        ),
        (
            TransportPublishOutcomeKind::Restricted,
            RadrootsRelayOutcomeKind::Restricted,
        ),
        (
            TransportPublishOutcomeKind::AuthRequired,
            RadrootsRelayOutcomeKind::AuthRequired,
        ),
        (
            TransportPublishOutcomeKind::Muted,
            RadrootsRelayOutcomeKind::Muted,
        ),
        (
            TransportPublishOutcomeKind::Unsupported,
            RadrootsRelayOutcomeKind::Unsupported,
        ),
        (
            TransportPublishOutcomeKind::Error,
            RadrootsRelayOutcomeKind::Error,
        ),
        (
            TransportPublishOutcomeKind::Timeout,
            RadrootsRelayOutcomeKind::Timeout,
        ),
        (
            TransportPublishOutcomeKind::ConnectionFailed,
            RadrootsRelayOutcomeKind::ConnectionFailed,
        ),
        (
            TransportPublishOutcomeKind::TargetRejected,
            RadrootsRelayOutcomeKind::RelayUrlRejected,
        ),
        (
            TransportPublishOutcomeKind::Unknown,
            RadrootsRelayOutcomeKind::Unknown,
        ),
    ];
    for (proxy_kind, relay_kind) in cases {
        let receipt = proxy_relay_receipt_from_response(TransportPublishEventResponse {
            deduplicated: false,
            job: job(proxy_kind),
        })
        .expect("receipt");
        assert_eq!(receipt.relays[0].outcome.kind, relay_kind);
    }
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
    assert_eq!(body["params"]["event"]["content"], "{\"name\":\"carrots\"}");
    assert_eq!(body["params"]["target_policy"]["kind"], "nostr");
    assert_eq!(
        body["params"]["target_policy"]["relay_urls"][0],
        "wss://relay.example.com"
    );
}

#[tokio::test]
async fn publish_signed_event_posts_typed_proxy_request() {
    let (endpoint, handle) = spawn_http_server("200 OK", publish_response_json().as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(
        RadrootsdProxyConfig::new(endpoint)
            .with_auth(RadrootsdAuth::BearerToken("sdk-token".into()))
            .with_request_timeout_ms(7_000),
    );

    let receipt = adapter
        .publish_signed_event(RadrootsdProxyPublishRequest {
            signed_event: signed_event(),
            target_policy: TransportPublishTargetPolicy::explicit_targets(vec![
                TransportPublishTarget::nostr("wss://relay.example.com"),
            ]),
            delivery_policy: TransportPublishDeliveryPolicy::All,
            idempotency_key: Some("idem-typed".to_owned()),
            timeout_ms: adapter.config().request_timeout_ms,
        })
        .await
        .expect("typed publish");

    assert!(receipt.job.delivery_satisfied);
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
    assert_eq!(body["params"]["idempotency_key"], "idem-typed");
    assert_eq!(body["params"]["timeout_ms"], 7_000);
}

#[tokio::test]
async fn publish_signed_event_preserves_typed_reticulum_preview_behavior() {
    let (endpoint, handle) = spawn_http_server("200 OK", publish_response_json().as_str());
    let adapter = RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new(endpoint));

    adapter
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

    let recorded = handle.join().expect("server thread");
    let body: serde_json::Value = serde_json::from_str(recorded.body.as_str()).expect("body");
    assert_eq!(body["params"]["target_policy"]["kind"], "explicit_targets");
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["transport_kind"],
        "reticulum"
    );
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["endpoint_uri"],
        RETICULUM_PREVIEW_ENDPOINT_URI
    );
    assert_eq!(
        body["params"]["target_policy"]["targets"][0]["preview_behavior"],
        "defer_delivery_plans"
    );
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
async fn relay_publish_adapter_derives_delivery_policy_and_timeout() {
    for (target_count, satisfaction_policy, expected_policy) in [
        (
            2,
            radroots_transport::RadrootsTransportSatisfactionPolicy::all_accepted(),
            TransportPublishDeliveryPolicy::All,
        ),
        (
            2,
            radroots_transport::RadrootsTransportSatisfactionPolicy::any_accepted(),
            TransportPublishDeliveryPolicy::Any,
        ),
        (
            3,
            radroots_transport::RadrootsTransportSatisfactionPolicy::quorum_accepted(2),
            TransportPublishDeliveryPolicy::Quorum { quorum: 2 },
        ),
    ] {
        let response_body = publish_response_json();
        let (endpoint, handle) = spawn_http_server("200 OK", response_body.as_str());
        let adapter = RadrootsdProxyPublishAdapter::new(
            RadrootsdProxyConfig::new(endpoint).with_request_timeout_ms(4_000),
        );
        let relays = (0..target_count)
            .map(|index| format!("wss://relay-{index}.example.com"))
            .collect::<Vec<_>>();
        let targets =
            RadrootsRelayTargetSet::new(&relays, RadrootsRelayUrlPolicy::Public).expect("targets");

        let receipts = adapter
            .publish(
                RadrootsRelayPublishRequest::new(signed_event(), targets, 10)
                    .with_satisfaction_policy(satisfaction_policy),
            )
            .await
            .expect("adapter publish");

        assert_eq!(receipts[0].outcome.kind, RadrootsRelayOutcomeKind::Accepted);
        let recorded = handle.join().expect("server thread");
        let body: serde_json::Value =
            serde_json::from_str(recorded.body.as_str()).expect("request body");
        assert_eq!(body["params"]["timeout_ms"], 4_000);
        assert_eq!(
            serde_json::from_value::<TransportPublishDeliveryPolicy>(
                body["params"]["delivery_policy"].clone()
            )
            .expect("delivery policy"),
            expected_policy
        );
    }
}

#[tokio::test]
async fn relay_publish_adapter_maps_proxy_errors_to_transport_errors() {
    let adapter = RadrootsdProxyPublishAdapter::new(
        RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc").with_timeout(Duration::from_millis(50)),
    );
    let targets = RadrootsRelayTargetSet::new(
        &["wss://relay.example.com".to_owned()],
        RadrootsRelayUrlPolicy::Public,
    )
    .expect("targets");

    let error = adapter
        .publish(RadrootsRelayPublishRequest::new(
            signed_event(),
            targets,
            1_700_000_000_000,
        ))
        .await
        .expect_err("transport error");

    assert!(matches!(
        error,
        radroots_transport_nostr::RadrootsRelayTransportError::Transport(message)
            if message.contains("radrootsd")
    ));
}

#[test]
fn relay_proxy_target_conversion_rejects_reticulum_targets_before_behavior_loss() {
    let target = radroots_transport::RadrootsTransportTarget::new(
        radroots_transport::RadrootsTransportKind::Reticulum,
        RETICULUM_PREVIEW_ENDPOINT_URI,
    )
    .expect("Reticulum target");

    let error = transport_publish_target(&target).expect_err("Reticulum rejected");

    assert!(matches!(
        error,
        radroots_transport_nostr::RadrootsRelayTransportError::Transport(message)
            if message.contains("Nostr-only") && message.contains("reticulum")
    ));
}

#[tokio::test]
async fn publish_signed_event_rejects_invalid_protocol_requests_before_http() {
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

    let mut invalid_event_kind = base.clone();
    invalid_event_kind.signed_event.kind = 70_000;
    let mut empty_event_tag = base.clone();
    empty_event_tag.signed_event.tags = vec![Vec::new()];
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
    let mut empty_idempotency = base;
    empty_idempotency.idempotency_key = Some(" ".to_owned());

    for request in [
        invalid_event_kind,
        empty_event_tag,
        invalid_quorum,
        too_many_targets,
        empty_endpoint_uri,
        empty_idempotency,
    ] {
        assert!(matches!(
            adapter.publish_signed_event(request).await,
            Err(RadrootsdError::InvalidRequest(_))
        ));
    }
}

#[test]
fn proxy_relay_receipt_from_response_rejects_invalid_daemon_job_contracts() {
    let mut empty_job_id = job(TransportPublishOutcomeKind::Accepted);
    empty_job_id.job_id = " ".to_owned();
    let mut invalid_event_id = job(TransportPublishOutcomeKind::Accepted);
    invalid_event_id.event_id = "not-an-event-id".to_owned();
    let mut invalid_pubkey = job(TransportPublishOutcomeKind::Accepted);
    invalid_pubkey.pubkey = "not-a-pubkey".to_owned();
    let mut invalid_kind = job(TransportPublishOutcomeKind::Accepted);
    invalid_kind.event_kind = 70_000;
    let mut invalid_quorum = job(TransportPublishOutcomeKind::Accepted);
    invalid_quorum.delivery_policy = TransportPublishDeliveryPolicy::Quorum { quorum: 0 };

    for job in [
        empty_job_id,
        invalid_event_id,
        invalid_pubkey,
        invalid_kind,
        invalid_quorum,
    ] {
        assert!(matches!(
            proxy_relay_receipt_from_response(TransportPublishEventResponse {
                deduplicated: false,
                job,
            }),
            Err(RadrootsdError::InvalidRequest(_))
        ));
    }
}

#[tokio::test]
async fn adapter_rejects_invalid_request_before_transport() {
    let adapter =
        RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc"));
    let mut request = RadrootsdProxyPublishRequest {
        signed_event: signed_event(),
        target_policy: TransportPublishTargetPolicy::nostr(
            NostrPublishTargetSourcePolicy::RequestThenAuthorWriteThenDaemonDefault,
            Vec::new(),
        ),
        delivery_policy: TransportPublishDeliveryPolicy::Quorum { quorum: 0 },
        idempotency_key: None,
        timeout_ms: None,
    };
    request.signed_event.id = "A".repeat(64);

    let error = adapter
        .publish_signed_event(request)
        .await
        .expect_err("invalid request");

    assert!(matches!(error, RadrootsdError::InvalidRequest(_)));
}
