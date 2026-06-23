use super::*;
use radroots_publish_proxy_protocol::{
    PublishJobStatus, PublishJobView, PublishRelayOutcome, PublishRelayOutcomeKind,
    PublishRelaySource,
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

fn publish_request() -> PublishEventRequest {
    PublishEventRequest {
        event: signed_event_wire(&signed_event()),
        relays: vec!["wss://relay.example.com".to_owned()],
        relay_policy: PublishRelayPolicy::RequestThenAuthorWriteThenDaemonDefault,
        delivery_policy: PublishDeliveryPolicy::Any,
        idempotency_key: Some("idem-1".to_owned()),
        timeout_ms: Some(10_000),
    }
}

fn job(outcome_kind: PublishRelayOutcomeKind) -> PublishJobView {
    PublishJobView {
        job_id: "job-1".to_owned(),
        status: PublishJobStatus::DeliverySatisfied,
        terminal: true,
        delivery_satisfied: true,
        event_id: "a".repeat(64),
        pubkey: "b".repeat(64),
        event_kind: 30_402,
        relay_policy: PublishRelayPolicy::RequestThenAuthorWriteThenDaemonDefault,
        delivery_policy: PublishDeliveryPolicy::Any,
        relay_count: 1,
        acknowledged_count: usize::from(outcome_kind.counts_toward_quorum()),
        retryable_count: usize::from(outcome_kind.is_retryable()),
        terminal_count: usize::from(outcome_kind.is_terminal_failure()),
        requested_at_ms: 1_700_000_000_000,
        completed_at_ms: Some(1_700_000_000_100),
        last_error: None,
        relays: vec![PublishRelayOutcome {
            relay_url: "wss://relay.example.com".to_owned(),
            source: PublishRelaySource::Request,
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
            "job": job(PublishRelayOutcomeKind::Accepted)
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
}

#[test]
fn publish_event_request_json_uses_signed_event_contract() {
    let value = publish_event_request_json(&publish_request()).expect("request json");

    assert_eq!(value["event"]["id"], "a".repeat(64));
    assert_eq!(value["event"]["pubkey"], "b".repeat(64));
    assert_eq!(value["event"]["kind"], 30_402);
    assert_eq!(value["relays"][0], "wss://relay.example.com");
    assert_eq!(
        value["relay_policy"],
        "request_then_author_write_then_daemon_default"
    );
    assert_eq!(value["delivery_policy"]["mode"], "any");
    assert_eq!(value["idempotency_key"], "idem-1");
    let rendered = value.to_string();
    assert!(!rendered.contains("signer_session_id"));
    assert!(!rendered.contains("bridge."));
}

#[test]
fn decode_jsonrpc_response_validates_envelope_and_errors() {
    let response: PublishEventResponse = decode_jsonrpc_response(
        METHOD_EVENT,
        SDK_RADROOTSD_PROXY_REQUEST_ID,
        publish_response_json().as_str(),
    )
    .expect("response");
    assert_eq!(response.job.event_id, "a".repeat(64));

    let error = decode_jsonrpc_response::<PublishEventResponse>(
        METHOD_EVENT,
        SDK_RADROOTSD_PROXY_REQUEST_ID,
        r#"{"jsonrpc":"2.0","id":"radroots-sdk-publish-event","error":{"code":-32001,"message":"principal unauthorized"}}"#,
    )
    .expect_err("jsonrpc error");
    assert!(matches!(
        error,
        RadrootsdError::JsonRpc { code: -32001, .. }
    ));
    assert_message(error, "principal unauthorized");

    assert!(matches!(
        decode_jsonrpc_response::<PublishEventResponse>(
            METHOD_EVENT,
            "expected",
            r#"{"jsonrpc":"2.0","id":"other","result":{}}"#
        ),
        Err(RadrootsdError::MalformedResponse(_))
    ));
    assert!(matches!(
        decode_jsonrpc_response::<PublishEventResponse>(
            METHOD_EVENT,
            "expected",
            r#"{"jsonrpc":"2.0","id":"expected"}"#
        ),
        Err(RadrootsdError::MalformedResponse(_))
    ));
}

#[test]
fn daemon_outcomes_map_to_relay_transport_receipts() {
    let payment = proxy_receipt_from_response(PublishEventResponse {
        deduplicated: false,
        job: job(PublishRelayOutcomeKind::PaymentRequired),
    })
    .expect("payment receipt");
    assert_eq!(
        payment.relays[0].outcome.kind,
        RadrootsRelayOutcomeKind::PaymentRequired
    );
    assert_eq!(payment.terminal_count, 1);

    let skipped = proxy_receipt_from_response(PublishEventResponse {
        deduplicated: true,
        job: job(PublishRelayOutcomeKind::SkippedAlreadyAccepted),
    })
    .expect("skipped receipt");
    assert_eq!(
        skipped.relays[0].outcome.kind,
        RadrootsRelayOutcomeKind::SkippedAlreadyAccepted
    );
    assert!(skipped.quorum_met);
}

#[tokio::test]
async fn publish_event_posts_publish_proxy_jsonrpc() {
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
    assert_eq!(body["params"]["relays"][0], "wss://relay.example.com");
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
async fn adapter_rejects_invalid_request_before_transport() {
    let adapter =
        RadrootsdProxyPublishAdapter::new(RadrootsdProxyConfig::new("http://127.0.0.1:9/rpc"));
    let mut request = RadrootsdProxyPublishRequest {
        signed_event: signed_event(),
        relays: Vec::new(),
        delivery_policy: PublishDeliveryPolicy::Quorum { quorum: 0 },
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
