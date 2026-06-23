use super::*;
use crate::farm::RadrootsFarmRef;
use crate::listing::{
    RadrootsListingAvailability, RadrootsListingBin, RadrootsListingDeliveryMethod,
    RadrootsListingLocation, RadrootsListingProduct, RadrootsListingStatus,
};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
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
    spawn_http_server_with_content_length(status, response_body, response_body.len())
}

fn spawn_http_server_with_content_length(
    status: &str,
    response_body: &str,
    content_length: usize,
) -> (String, JoinHandle<RecordedHttpRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let endpoint = format!("http://{}/rpc", listener.local_addr().expect("addr"));
    let status = status.to_owned();
    let response_body = response_body.to_owned();
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
                let content_length = header_text
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().expect("content length"))
                    })
                    .unwrap_or(0);
                while request.len() < headers_end + content_length {
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

fn sample_listing() -> RadrootsListing {
    RadrootsListing {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".parse().expect("listing d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
            pubkey: "a".repeat(64),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        },
        product: RadrootsListingProduct {
            key: "coffee".into(),
            title: "Coffee".into(),
            category: "coffee".into(),
            summary: Some("Single origin coffee".into()),
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: "bin-1".parse().expect("primary bin id"),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".parse().expect("bin id"),
            quantity: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(1000u32),
                RadrootsCoreUnit::MassG,
            ),
            price_per_canonical_unit: RadrootsCoreQuantityPrice {
                amount: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(20u32),
                    RadrootsCoreCurrency::USD,
                ),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1u32),
                    RadrootsCoreUnit::MassG,
                ),
            },
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }],
        resource_area: None,
        plot: None,
        discounts: None,
        inventory_available: Some(RadrootsCoreDecimal::from(5u32)),
        availability: Some(RadrootsListingAvailability::Status {
            status: RadrootsListingStatus::Active,
        }),
        delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
        location: Some(RadrootsListingLocation {
            primary: "North Farm".into(),
            city: None,
            region: None,
            country: None,
            lat: None,
            lng: None,
            geohash: None,
        }),
        images: None,
    }
}

fn sample_profile() -> RadrootsProfile {
    RadrootsProfile {
        name: "North Farm".into(),
        display_name: Some("North Farm".into()),
        nip05: None,
        about: Some("Organic coffee".into()),
        website: Some("https://example.com".into()),
        picture: None,
        banner: None,
        lud06: None,
        lud16: None,
        bot: None,
    }
}

fn sample_farm() -> RadrootsFarm {
    RadrootsFarm {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        name: "North Farm".into(),
        about: Some("Organic coffee".into()),
        website: None,
        picture: None,
        banner: None,
        location: None,
        tags: Some(vec!["coffee".into()]),
    }
}

fn sample_listing_event(kind: u32) -> RadrootsNostrEvent {
    let listing = sample_listing();
    let parts = listing::build_draft(&listing).expect("listing draft");
    RadrootsNostrEvent {
        id: "event-1".into(),
        author: listing.farm.pubkey,
        created_at: 1,
        kind,
        tags: parts.as_wire_parts().tags.clone(),
        content: parts.as_wire_parts().content.clone(),
        sig: String::new(),
    }
}

fn sample_authority() -> SdkRadrootsdSignerAuthority {
    SdkRadrootsdSignerAuthority {
        provider_runtime_id: "local-runtime".into(),
        account_identity_id: "account-1".into(),
        provider_signer_session_id: Some("provider-session-secret".into()),
    }
}

fn sample_listing_publish_request() -> SdkRadrootsdListingPublishRequest {
    SdkRadrootsdListingPublishRequest {
        listing: sample_listing(),
        kind: Some(KIND_LISTING),
        signer_session_id: "signer-session-secret".into(),
        signer_authority: Some(sample_authority()),
        idempotency_key: Some("idem-1".into()),
    }
}

fn assert_message(error: RadrootsdError, fragment: &str) {
    let message = error.to_string();
    assert!(
        message.contains(fragment),
        "expected {message:?} to contain {fragment:?}"
    );
}

#[test]
fn auth_headers_omit_authorization_when_auth_is_none() {
    let headers = auth_headers(&RadrootsdAuth::None).expect("headers");

    assert!(!headers.contains_key(AUTHORIZATION));
}

#[test]
fn auth_headers_build_bearer_authorization() {
    let headers = auth_headers(&RadrootsdAuth::BearerToken("sdk-token".into())).expect("headers");

    assert_eq!(
        headers
            .get(AUTHORIZATION)
            .expect("authorization")
            .to_str()
            .expect("authorization str"),
        "Bearer sdk-token"
    );
}

#[test]
fn auth_headers_reject_invalid_bearer_header_values() {
    let error = auth_headers(&RadrootsdAuth::BearerToken("bad\ntoken".into())).expect_err("error");

    assert!(matches!(error, RadrootsdError::InvalidAuthHeader(_)));
}

#[test]
fn bridge_listing_publish_request_json_preserves_request_contract() {
    let value =
        bridge_listing_publish_request_json(&sample_listing_publish_request()).expect("json");

    assert_eq!(value["kind"], KIND_LISTING);
    assert_eq!(value["signer_session_id"], "signer-session-secret");
    assert_eq!(
        value["signer_authority"]["provider_signer_session_id"],
        "provider-session-secret"
    );
    assert_eq!(value["idempotency_key"], "idem-1");
    assert_eq!(value["listing"]["product"]["title"], "Coffee");
}

#[test]
fn listing_publish_request_from_event_parses_listing_and_rejects_wrong_kind() {
    let event = sample_listing_event(KIND_LISTING);
    let request = SdkRadrootsdListingPublishRequest::from_event(
        &event,
        "signer-session-secret",
        Some(sample_authority()),
        Some("idem-1".to_owned()),
    )
    .expect("request");

    assert_eq!(request.kind, Some(KIND_LISTING));
    assert_eq!(request.signer_session_id, "signer-session-secret");
    assert_eq!(request.idempotency_key.as_deref(), Some("idem-1"));
    assert_eq!(request.listing.product.title, "Coffee");

    let wrong_kind = sample_listing_event(1);
    assert_eq!(
        SdkRadrootsdListingPublishRequest::from_event(&wrong_kind, "session", None, None)
            .expect_err("wrong kind"),
        listing::RadrootsListingParseError::InvalidKind(1)
    );

    let mut malformed = sample_listing_event(KIND_LISTING);
    malformed.tags = Vec::new();
    let malformed_error =
        SdkRadrootsdListingPublishRequest::from_event(&malformed, "session", None, None)
            .expect_err("malformed listing");
    assert!(!malformed_error.to_string().is_empty());
}

#[tokio::test]
async fn jsonrpc_call_rejects_invalid_auth_before_transport() {
    let error = jsonrpc_call::<_, Value>(
        "http://127.0.0.1:9/rpc",
        &RadrootsdAuth::BearerToken("bad\ntoken".to_owned()),
        "1",
        "listing.publish",
        &json!({}),
        core::time::Duration::from_millis(10),
    )
    .await
    .expect_err("invalid auth");
    assert!(matches!(error, RadrootsdError::InvalidAuthHeader(_)));
}

#[test]
fn debug_output_redacts_auth_and_signer_secrets() {
    let auth = RadrootsdAuth::BearerToken("token-secret".into());
    let none_auth = RadrootsdAuth::None;
    let bunker = SdkRadrootsdSignerSessionConnectRequest::bunker("bunker://session");
    let connect =
        SdkRadrootsdSignerSessionConnectRequest::nostrconnect("nostrconnect://session", "nsec")
            .with_signer_authority(sample_authority());
    let profile_request = SdkRadrootsdProfilePublishRequest {
        profile: sample_profile(),
        profile_type: Some(RadrootsProfileType::Farm),
        signer_session_id: "profile-session-secret".into(),
        signer_authority: Some(sample_authority()),
        idempotency_key: Some("profile-idem".into()),
    };
    let farm_request = SdkRadrootsdFarmPublishRequest {
        farm: sample_farm(),
        kind: Some(30_000),
        signer_session_id: "farm-session-secret".into(),
        signer_authority: Some(sample_authority()),
        idempotency_key: Some("farm-idem".into()),
    };
    let listing_request = sample_listing_publish_request();
    let job = SdkRadrootsdBridgeJob {
        job_id: "job-1".into(),
        command: "bridge.listing.publish".into(),
        status: "accepted".into(),
        terminal: false,
        recovered_after_restart: false,
        signer_mode: "bunker".into(),
        signer_session_id: Some("signer-session-secret".into()),
        event_kind: KIND_LISTING,
        event_id: Some("event-1".into()),
        event_addr: Some("30402:pubkey:d-tag".into()),
        relay_count: 2,
        acknowledged_relay_count: 1,
    };
    let job_view = SdkRadrootsdBridgeJobView {
        job_id: "job-view-1".into(),
        command: "bridge.listing.publish".into(),
        idempotency_key: Some("view-idem".into()),
        status: SdkRadrootsdBridgeJobStatus::Accepted,
        terminal: false,
        recovered_after_restart: true,
        requested_at_unix: 1,
        completed_at_unix: Some(2),
        signer_mode: "nostrconnect".into(),
        signer_session_id: Some("view-session-secret".into()),
        event_kind: KIND_LISTING,
        event_id: Some("event-1".into()),
        event_addr: Some("30402:pubkey:d-tag".into()),
        delivery_policy: SdkRadrootsdBridgeDeliveryPolicy::Quorum,
        delivery_quorum: Some(2),
        relay_count: 3,
        acknowledged_relay_count: 2,
        required_acknowledged_relay_count: 2,
        attempt_count: 1,
        attempt_summaries: vec!["ok".into()],
        relay_results: vec![SdkRadrootsdBridgeRelayPublishResult {
            relay_url: "wss://relay.example.com".into(),
            acknowledged: true,
            detail: Some("accepted".into()),
        }],
        relay_outcome_summary: "2/3 acknowledged".into(),
    };

    let rendered = format!(
        "{none_auth:?} {auth:?} {bunker:?} {connect:?} {profile_request:?} {farm_request:?} {listing_request:?} {job:?} {job_view:?}"
    );

    assert!(rendered.contains("None"));
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("token-secret"));
    assert!(!rendered.contains("nsec"));
    assert!(!rendered.contains("provider-session-secret"));
    assert!(!rendered.contains("signer-session-secret"));
    assert!(!rendered.contains("profile-session-secret"));
    assert!(!rendered.contains("farm-session-secret"));
    assert!(!rendered.contains("view-session-secret"));
    assert!(!rendered.contains("signer_mode: \"bunker\""));
}

#[test]
fn http_status_error_omits_raw_body() {
    let error = http_status_error(reqwest::StatusCode::UNAUTHORIZED, "missing secret token");

    let message = error.to_string();
    assert!(message.contains("radrootsd returned http 401"));
    assert!(message.contains("response body omitted"));
    assert!(!message.contains("missing secret token"));

    assert_message(
        http_status_error(reqwest::StatusCode::BAD_GATEWAY, ""),
        "response body empty",
    );
}

#[test]
fn radrootsd_error_display_covers_all_variants() {
    assert_message(
        RadrootsdError::InvalidAuthHeader("bad header".into()),
        "invalid radrootsd bearer token header",
    );
    assert_message(RadrootsdError::Http("http".into()), "http");
    assert_message(RadrootsdError::JsonRpc("jsonrpc".into()), "jsonrpc");
    assert_message(
        RadrootsdError::MalformedResponse("malformed".into()),
        "malformed",
    );
}

#[test]
fn decode_jsonrpc_response_returns_result() {
    let response: SdkRadrootsdBridgePublishResponse = decode_jsonrpc_response(
        "bridge.listing.publish",
        "radroots-sdk-listing-publish",
        r#"{
                "jsonrpc": "2.0",
                "id": "radroots-sdk-listing-publish",
                "result": {
                    "deduplicated": false,
                    "job": {
                        "job_id": "job-1",
                        "command": "bridge.listing.publish",
                        "status": "accepted",
                        "terminal": false,
                        "recovered_after_restart": false,
                        "signer_mode": "bunker",
                        "signer_session_id": "signer-session-secret",
                        "event_kind": 30402,
                        "event_id": "event-1",
                        "event_addr": "30402:pubkey:d-tag",
                        "relay_count": 2,
                        "acknowledged_relay_count": 1
                    }
                }
            }"#,
    )
    .expect("response");

    assert!(!response.deduplicated);
    assert_eq!(response.job.job_id, "job-1");
    assert_eq!(
        response.job.signer_session_id.as_deref(),
        Some("signer-session-secret")
    );
}

#[test]
fn decode_jsonrpc_response_returns_jsonrpc_error() {
    let error = decode_jsonrpc_response::<SdkRadrootsdBridgePublishResponse>(
        "bridge.listing.publish",
        "radroots-sdk-listing-publish",
        r#"{
                "jsonrpc": "2.0",
                "id": "radroots-sdk-listing-publish",
                "error": { "code": -32001, "message": "signer unavailable" }
            }"#,
    )
    .expect_err("error");

    assert!(matches!(error, RadrootsdError::JsonRpc(_)));
    assert_message(
        error,
        "radrootsd bridge.listing.publish failed -32001: signer unavailable",
    );
}

#[test]
fn decode_jsonrpc_response_rejects_result_plus_error() {
    let error = decode_jsonrpc_response::<serde_json::Value>(
        "bridge.listing.publish",
        "radroots-sdk-listing-publish",
        r#"{
                "jsonrpc": "2.0",
                "id": "radroots-sdk-listing-publish",
                "result": { "ok": true },
                "error": { "code": -32002, "message": "ambiguous response" }
            }"#,
    )
    .expect_err("error");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(
        error,
        "radrootsd bridge.listing.publish returned result and error: -32002 ambiguous response",
    );
}

#[test]
fn decode_jsonrpc_response_rejects_missing_result_and_error() {
    let error = decode_jsonrpc_response::<serde_json::Value>(
        "bridge.listing.publish",
        "radroots-sdk-listing-publish",
        r#"{ "jsonrpc": "2.0", "id": "radroots-sdk-listing-publish" }"#,
    )
    .expect_err("error");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(
        error,
        "radrootsd bridge.listing.publish returned neither result nor error",
    );
}

#[test]
fn decode_jsonrpc_response_rejects_malformed_json() {
    let error = decode_jsonrpc_response::<serde_json::Value>(
        "bridge.listing.publish",
        "radroots-sdk-listing-publish",
        r#"{ "result": "#,
    )
    .expect_err("error");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(error, "decode radrootsd bridge.listing.publish response");
}

#[test]
fn decode_jsonrpc_response_rejects_invalid_version() {
    let error = decode_jsonrpc_response::<serde_json::Value>(
        "bridge.listing.publish",
        "radroots-sdk-listing-publish",
        r#"{
                "jsonrpc": "1.0",
                "id": "radroots-sdk-listing-publish",
                "result": { "ok": true }
            }"#,
    )
    .expect_err("error");

    assert_message(error, "returned invalid jsonrpc version");
}

#[test]
fn decode_jsonrpc_response_rejects_mismatched_id() {
    let error = decode_jsonrpc_response::<serde_json::Value>(
        "bridge.listing.publish",
        "radroots-sdk-listing-publish",
        r#"{
                "jsonrpc": "2.0",
                "id": "other-id",
                "result": { "ok": true }
            }"#,
    )
    .expect_err("error");

    assert_message(error, "returned mismatched jsonrpc id");
}

#[tokio::test]
async fn publish_listing_uses_http_jsonrpc_request_path() {
    let (endpoint, handle) = spawn_http_server(
        "200 OK",
        r#"{
                "jsonrpc": "2.0",
                "id": "radroots-sdk-listing-publish",
                "result": {
                    "deduplicated": true,
                    "job": {
                        "job_id": "job-1",
                        "command": "bridge.listing.publish",
                        "status": "accepted",
                        "terminal": false,
                        "recovered_after_restart": false,
                        "signer_mode": "bunker",
                        "signer_session_id": "signer-session-secret",
                        "event_kind": 30402,
                        "event_id": "event-1",
                        "event_addr": "30402:pubkey:d-tag",
                        "relay_count": 2,
                        "acknowledged_relay_count": 1
                    }
                }
            }"#,
    );

    let response = publish_listing(
        &endpoint,
        &RadrootsdAuth::BearerToken("sdk-token".into()),
        &sample_listing_publish_request(),
        Duration::from_secs(5),
    )
    .await
    .expect("publish response");
    let request = handle.join().expect("request");
    let body = serde_json::from_str::<Value>(&request.body).expect("body json");

    assert!(response.deduplicated);
    assert_eq!(request.request_line, "POST /rpc HTTP/1.1");
    assert!(
        request
            .headers
            .iter()
            .any(|(name, value)| { name == "authorization" && value == "Bearer sdk-token" })
    );
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], "radroots-sdk-listing-publish");
    assert_eq!(body["method"], "bridge.listing.publish");
    assert_eq!(
        body["params"]["signer_authority"]["provider_signer_session_id"],
        "provider-session-secret"
    );
}

#[tokio::test]
async fn publish_listing_returns_jsonrpc_errors_from_http_path() {
    let (endpoint, handle) = spawn_http_server(
        "200 OK",
        r#"{
                "jsonrpc": "2.0",
                "id": "radroots-sdk-listing-publish",
                "error": { "code": -32001, "message": "signer unavailable" }
            }"#,
    );

    let error = publish_listing(
        &endpoint,
        &RadrootsdAuth::None,
        &sample_listing_publish_request(),
        Duration::from_secs(5),
    )
    .await
    .expect_err("error");
    handle.join().expect("request");

    assert!(matches!(error, RadrootsdError::JsonRpc(_)));
    assert_message(error, "signer unavailable");
}

#[tokio::test]
async fn publish_listing_sanitizes_http_status_body() {
    let (endpoint, handle) = spawn_http_server("500 Internal Server Error", "secret body");

    let error = publish_listing(
        &endpoint,
        &RadrootsdAuth::None,
        &sample_listing_publish_request(),
        Duration::from_secs(5),
    )
    .await
    .expect_err("error");
    handle.join().expect("request");

    let message = error.to_string();
    assert!(message.contains("radrootsd returned http 500"));
    assert!(!message.contains("secret body"));
}

#[tokio::test]
async fn publish_listing_reports_malformed_http_response_body() {
    let (endpoint, handle) = spawn_http_server("200 OK", r#"{ "result": "#);

    let error = publish_listing(
        &endpoint,
        &RadrootsdAuth::None,
        &sample_listing_publish_request(),
        Duration::from_secs(5),
    )
    .await
    .expect_err("error");
    handle.join().expect("request");

    assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
    assert_message(error, "decode radrootsd bridge.listing.publish response");
}

#[tokio::test]
async fn publish_listing_reports_http_response_body_read_errors() {
    let body = r#"{ "jsonrpc": "2.0", "id": "radroots-sdk-listing-publish" }"#;
    let (endpoint, handle) = spawn_http_server_with_content_length("200 OK", body, body.len() + 64);

    let error = publish_listing(
        &endpoint,
        &RadrootsdAuth::None,
        &sample_listing_publish_request(),
        Duration::from_secs(5),
    )
    .await
    .expect_err("error");
    handle.join().expect("request");

    assert!(matches!(error, RadrootsdError::Http(_)));
    assert_message(error, "read radrootsd response body");
}

#[tokio::test]
async fn publish_listing_reports_transport_send_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused port");
    let endpoint = format!("http://{}/rpc", listener.local_addr().expect("addr"));
    drop(listener);

    let error = publish_listing(
        &endpoint,
        &RadrootsdAuth::None,
        &sample_listing_publish_request(),
        Duration::from_millis(250),
    )
    .await
    .expect_err("error");

    assert!(matches!(error, RadrootsdError::Http(_)));
    assert_message(error, "send radrootsd bridge.listing.publish request");
}
