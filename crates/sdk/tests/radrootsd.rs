#![cfg(feature = "radrootsd-client")]

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::kinds::KIND_LISTING_DRAFT;
use radroots_sdk::adapters::radrootsd::{
    SdkRadrootsdBridgeJob, SdkRadrootsdBridgePublishResponse, SdkRadrootsdListingPublishRequest,
    SdkRadrootsdSignerAuthority, SdkRadrootsdSignerSessionConnectRequest,
    SdkRadrootsdSignerSessionMode,
};
use radroots_sdk::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingLocation,
    RadrootsListingProduct, RadrootsListingStatus, RadrootsTradeListingParseError,
};
use radroots_sdk::{
    RadrootsNostrEvent, RadrootsSdkClient, RadrootsSdkConfig, RadrootsdAuth, RadrootsdConfig,
    SdkConfigError, SdkEnvironment, SdkPublishError, SdkRadrootsdListingPublishOptions,
    SdkRadrootsdPublishReceipt, SdkRadrootsdSessionError, SdkRadrootsdSignerSessionHandle,
    SdkRadrootsdSignerSessionRole, SdkRadrootsdSignerSessionView, SdkTransportMode,
    SdkTransportReceipt, SignerConfig,
};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct JsonRpcServer {
    endpoint: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl JsonRpcServer {
    async fn spawn(
        expected_auth: Option<&str>,
        response_body: Value,
    ) -> TestResult<(Self, oneshot::Receiver<Value>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let endpoint = format!("http://{addr}/jsonrpc");
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let (request_tx, request_rx) = oneshot::channel();
        let expected_auth = expected_auth.map(str::to_owned);
        let response_text = response_body.to_string();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accept = listener.accept() => {
                        let Ok((mut stream, _)) = accept else {
                            break;
                        };
                        let mut buffer = Vec::new();
                        let mut chunk = [0_u8; 4096];
                        let header_end = loop {
                            let Ok(read) = stream.read(&mut chunk).await else {
                                return;
                            };
                            if read == 0 {
                                return;
                            }
                            buffer.extend_from_slice(&chunk[..read]);
                            if let Some(index) = find_headers_end(&buffer) {
                                break index;
                            }
                        };

                        let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
                        let content_length = parse_content_length(headers.as_str()).unwrap_or(0);
                        let body_start = header_end + 4;
                        while buffer.len().saturating_sub(body_start) < content_length {
                            let Ok(read) = stream.read(&mut chunk).await else {
                                return;
                            };
                            if read == 0 {
                                break;
                            }
                            buffer.extend_from_slice(&chunk[..read]);
                        }

                        if let Some(expected_auth) = expected_auth.as_deref() {
                            let actual_auth = parse_authorization(headers.as_str());
                            if actual_auth.as_deref() != Some(expected_auth) {
                                let _ = write_http_response(
                                    &mut stream,
                                    401,
                                    json!({
                                        "jsonrpc": "2.0",
                                        "id": "sdk-test",
                                        "error": {
                                            "code": -32001,
                                            "message": format!(
                                                "unexpected authorization header: {:?}",
                                                actual_auth
                                            ),
                                        }
                                    })
                                    .to_string()
                                    .as_str(),
                                )
                                .await;
                                return;
                            }
                        }

                        let body = &buffer[body_start..body_start + content_length];
                        let Ok(request_json) = serde_json::from_slice::<Value>(body) else {
                            return;
                        };
                        let _ = request_tx.send(request_json);
                        let _ = write_http_response(&mut stream, 200, response_text.as_str()).await;
                        break;
                    }
                }
            }
        });

        Ok((
            Self {
                endpoint,
                shutdown_tx: Some(shutdown_tx),
            },
            request_rx,
        ))
    }

    fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }
}

impl Drop for JsonRpcServer {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> Option<usize> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if !name.eq_ignore_ascii_case("content-length") {
            return None;
        }
        value.trim().parse().ok()
    })
}

fn parse_authorization(headers: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if !name.eq_ignore_ascii_case("authorization") {
            return None;
        }
        Some(value.trim().to_owned())
    })
}

async fn write_http_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) -> Result<(), std::io::Error> {
    let status_text = match status {
        200 => "OK",
        401 => "Unauthorized",
        _ => "Internal Server Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {status_text}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await
}

fn sample_listing() -> RadrootsListing {
    RadrootsListing {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
        farm: RadrootsListingFarmRef {
            pubkey: "seller".into(),
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
        primary_bin_id: "bin-1".into(),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".into(),
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

fn sdk_event(
    author: &str,
    created_at: u32,
    draft: radroots_sdk::listing::RadrootsListingDraft,
) -> RadrootsNostrEvent {
    let parts = draft.into_wire_parts();
    RadrootsNostrEvent {
        id: "event-1".to_owned(),
        author: author.to_owned(),
        created_at,
        kind: parts.kind,
        tags: parts.tags,
        content: parts.content,
        sig: "f".repeat(128),
    }
}

fn radrootsd_test_client(endpoint: &str) -> Result<RadrootsSdkClient, SdkConfigError> {
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::Nip46;
    config.radrootsd = RadrootsdConfig {
        endpoint: Some(endpoint.to_owned()),
        auth: RadrootsdAuth::BearerToken("sdk-secret".to_owned()),
    };
    RadrootsSdkClient::from_config(config)
}

fn sample_session_view_json(session_id: &str) -> Value {
    json!({
        "session_id": session_id,
        "role": "outbound_remote_signer",
        "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "user_pubkey": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "relays": ["wss://radroots.org"],
        "permissions": ["sign_event:30402"],
        "name": "Radroots Signer",
        "url": "https://radroots.org/signers/demo",
        "image": "https://radroots.org/signers/demo.png",
        "auth_required": false,
        "authorized": true,
        "auth_url": null,
        "expires_in_secs": 120,
        "signer_authority": {
            "provider_runtime_id": "runtime-1",
            "account_identity_id": "identity-1",
            "provider_signer_session_id": "provider-session-123"
        }
    })
}

#[test]
fn radrootsd_debug_redacts_signer_session_values() {
    let signer_authority = SdkRadrootsdSignerAuthority {
        provider_runtime_id: "runtime-1".to_owned(),
        account_identity_id: "identity-1".to_owned(),
        provider_signer_session_id: Some("provider-session-123".to_owned()),
    };
    let request = SdkRadrootsdListingPublishRequest {
        listing: sample_listing(),
        kind: Some(30402),
        signer_session_id: "session-123".to_owned(),
        signer_authority: Some(signer_authority),
        idempotency_key: Some("idem-1".to_owned()),
    };
    let job = SdkRadrootsdBridgeJob {
        job_id: "job-1".to_owned(),
        command: "bridge.listing.publish".to_owned(),
        status: "published".to_owned(),
        terminal: true,
        recovered_after_restart: false,
        signer_mode: "nip46_session:session-123".to_owned(),
        signer_session_id: Some("session-123".to_owned()),
        event_kind: 30402,
        event_id: Some("event-1".to_owned()),
        event_addr: Some("30402:seller:listing-1".to_owned()),
        relay_count: 1,
        acknowledged_relay_count: 1,
    };
    let response = SdkRadrootsdBridgePublishResponse {
        deduplicated: false,
        job,
    };
    let receipt = SdkRadrootsdPublishReceipt {
        accepted: true,
        deduplicated: false,
        job_id: Some("job-1".to_owned()),
        status: Some("published".to_owned()),
        signer_mode: Some("nip46_session:session-123".to_owned()),
        signer_session_id: Some("session-123".to_owned()),
        event_addr: Some("30402:seller:listing-1".to_owned()),
        relay_count: Some(1),
        acknowledged_relay_count: Some(1),
    };

    let request_debug = format!("{request:?}");
    let response_debug = format!("{response:?}");
    let receipt_debug = format!("{receipt:?}");

    assert!(!request_debug.contains("session-123"));
    assert!(!request_debug.contains("provider-session-123"));
    assert!(request_debug.contains("<redacted>"));

    assert!(!response_debug.contains("session-123"));
    assert!(response_debug.contains("<redacted>"));

    assert!(!receipt_debug.contains("session-123"));
    assert!(receipt_debug.contains("<redacted>"));

    let options = SdkRadrootsdListingPublishOptions {
        signer_session_id: "session-123".to_owned(),
        idempotency_key: Some("idem-1".to_owned()),
    };
    let options_debug = format!("{options:?}");
    assert!(!options_debug.contains("session-123"));
    assert!(options_debug.contains("<redacted>"));

    let connect_request = SdkRadrootsdSignerSessionConnectRequest::nostrconnect(
        "nostrconnect://bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
        "client-secret-key",
    )
    .with_signer_authority(SdkRadrootsdSignerAuthority {
        provider_runtime_id: "runtime-1".to_owned(),
        account_identity_id: "identity-1".to_owned(),
        provider_signer_session_id: Some("provider-session-123".to_owned()),
    });
    let connect_request_debug = format!("{connect_request:?}");
    assert!(!connect_request_debug.contains("client-secret-key"));
    assert!(!connect_request_debug.contains("provider-session-123"));
    assert!(connect_request_debug.contains("<redacted>"));
}

#[tokio::test]
async fn radrootsd_signer_session_connect_returns_opaque_handle() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-connect",
            "result": {
                "session_id": "session-123",
                "mode": "Nostrconnect",
                "remote_signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "relays": ["wss://radroots.org"]
            }
        }),
    )
    .await?;

    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::Nip46;
    config.radrootsd = RadrootsdConfig {
        endpoint: Some(server.endpoint().to_owned()),
        auth: RadrootsdAuth::BearerToken("sdk-secret".to_owned()),
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let request = SdkRadrootsdSignerSessionConnectRequest::nostrconnect(
        "nostrconnect://bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
        "client-secret-key",
    );

    let handle: SdkRadrootsdSignerSessionHandle = client
        .radrootsd()
        .signer_sessions()
        .connect(&request)
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "nip46.connect");
    assert_eq!(
        request_json["params"]["url"],
        "nostrconnect://bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret"
    );
    assert_eq!(
        request_json["params"]["client_secret_key"],
        "client-secret-key"
    );
    assert_eq!(handle.mode(), SdkRadrootsdSignerSessionMode::Nostrconnect);
    assert_eq!(
        handle.remote_signer_pubkey(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        handle.client_pubkey(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(handle.relays(), &["wss://radroots.org".to_owned()]);

    let handle_debug = format!("{handle:?}");
    assert!(!handle_debug.contains("session-123"));
    assert!(handle_debug.contains("<redacted>"));

    let options = SdkRadrootsdListingPublishOptions::from_signer_session(&handle);
    let options_debug = format!("{options:?}");
    assert!(!options_debug.contains("session-123"));
    assert!(options_debug.contains("<redacted>"));

    Ok(())
}

#[tokio::test]
async fn radrootsd_signer_session_connect_bunker_supports_bunker_mode() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-connect",
            "result": {
                "session_id": "session-bunker",
                "mode": "Bunker",
                "remote_signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "relays": ["wss://radroots.org"]
            }
        }),
    )
    .await?;

    let client = radrootsd_test_client(server.endpoint())?;
    let handle: SdkRadrootsdSignerSessionHandle = client
        .radrootsd()
        .signer_sessions()
        .connect_bunker(
            "bunker://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
        )
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "nip46.connect");
    assert_eq!(
        request_json["params"]["url"],
        "bunker://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret"
    );
    assert!(request_json["params"]["client_secret_key"].is_null());
    assert_eq!(handle.mode(), SdkRadrootsdSignerSessionMode::Bunker);

    Ok(())
}

#[tokio::test]
async fn radrootsd_signer_session_status_returns_typed_view() -> TestResult<()> {
    let (connect_server, _) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-connect",
            "result": {
                "session_id": "session-123",
                "mode": "Nostrconnect",
                "remote_signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "relays": ["wss://radroots.org"]
            }
        }),
    )
    .await?;
    let connect_client = radrootsd_test_client(connect_server.endpoint())?;
    let handle: SdkRadrootsdSignerSessionHandle = connect_client
        .radrootsd()
        .signer_sessions()
        .connect_nostrconnect(
            "nostrconnect://bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
            "client-secret-key",
        )
        .await?;

    let (status_server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-session-status",
            "result": sample_session_view_json("session-123")
        }),
    )
    .await?;
    let status_client = radrootsd_test_client(status_server.endpoint())?;
    let session: SdkRadrootsdSignerSessionView = status_client
        .radrootsd()
        .signer_sessions()
        .status(handle.session())
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "nip46.session.status");
    assert_eq!(request_json["params"]["session_id"], "session-123");
    assert_eq!(session.session(), handle.session());
    assert_eq!(
        session.role,
        SdkRadrootsdSignerSessionRole::OutboundRemoteSigner
    );
    assert_eq!(
        session.client_pubkey,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(
        session.signer_pubkey,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        session.user_pubkey.as_deref(),
        Some("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc")
    );
    assert_eq!(session.relays, vec!["wss://radroots.org".to_owned()]);
    assert_eq!(session.permissions, vec!["sign_event:30402".to_owned()]);
    assert_eq!(session.name.as_deref(), Some("Radroots Signer"));
    assert_eq!(
        session.url.as_deref(),
        Some("https://radroots.org/signers/demo")
    );
    assert_eq!(
        session.image.as_deref(),
        Some("https://radroots.org/signers/demo.png")
    );
    assert!(session.authorized);
    assert!(!session.auth_required);
    assert_eq!(session.expires_in_secs, Some(120));
    assert_eq!(
        session
            .signer_authority
            .as_ref()
            .map(|value| value.provider_runtime_id.as_str()),
        Some("runtime-1")
    );

    let debug = format!("{session:?}");
    assert!(!debug.contains("session-123"));
    assert!(debug.contains("<redacted>"));

    Ok(())
}

#[tokio::test]
async fn radrootsd_signer_session_list_returns_typed_views() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-session-list",
            "result": [
                sample_session_view_json("session-123"),
                sample_session_view_json("session-456")
            ]
        }),
    )
    .await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let sessions: Vec<SdkRadrootsdSignerSessionView> =
        client.radrootsd().signer_sessions().list().await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "nip46.session.list");
    assert_eq!(sessions.len(), 2);
    assert_eq!(
        sessions[0].role,
        SdkRadrootsdSignerSessionRole::OutboundRemoteSigner
    );
    let debug = format!("{:?}", sessions[0].session());
    assert!(!debug.contains("session-123"));
    assert!(debug.contains("<redacted>"));

    Ok(())
}

#[tokio::test]
async fn radrootsd_signer_session_authorize_returns_typed_result() -> TestResult<()> {
    let (connect_server, _) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-connect",
            "result": {
                "session_id": "session-123",
                "mode": "Bunker",
                "remote_signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "relays": ["wss://radroots.org"]
            }
        }),
    )
    .await?;
    let connect_client = radrootsd_test_client(connect_server.endpoint())?;
    let handle: SdkRadrootsdSignerSessionHandle = connect_client
        .radrootsd()
        .signer_sessions()
        .connect_bunker(
            "bunker://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
        )
        .await?;

    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-session-authorize",
            "result": {
                "authorized": true,
                "replayed": true
            }
        }),
    )
    .await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let result = client
        .radrootsd()
        .signer_sessions()
        .authorize(handle.session())
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "nip46.session.authorize");
    assert_eq!(request_json["params"]["session_id"], "session-123");
    assert!(result.authorized);
    assert!(result.replayed);

    Ok(())
}

#[tokio::test]
async fn radrootsd_signer_session_require_auth_returns_typed_result() -> TestResult<()> {
    let (connect_server, _) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-connect",
            "result": {
                "session_id": "session-123",
                "mode": "Bunker",
                "remote_signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "relays": ["wss://radroots.org"]
            }
        }),
    )
    .await?;
    let connect_client = radrootsd_test_client(connect_server.endpoint())?;
    let handle: SdkRadrootsdSignerSessionHandle = connect_client
        .radrootsd()
        .signer_sessions()
        .connect_bunker(
            "bunker://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
        )
        .await?;

    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-session-require-auth",
            "result": {
                "required": true
            }
        }),
    )
    .await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let result = client
        .radrootsd()
        .signer_sessions()
        .require_auth(handle.session(), "https://radroots.org/auth")
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "nip46.session.require_auth");
    assert_eq!(request_json["params"]["session_id"], "session-123");
    assert_eq!(
        request_json["params"]["auth_url"],
        "https://radroots.org/auth"
    );
    assert!(result.required);

    Ok(())
}

#[tokio::test]
async fn radrootsd_signer_session_close_returns_typed_result() -> TestResult<()> {
    let (connect_server, _) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-connect",
            "result": {
                "session_id": "session-123",
                "mode": "Bunker",
                "remote_signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "relays": ["wss://radroots.org"]
            }
        }),
    )
    .await?;
    let connect_client = radrootsd_test_client(connect_server.endpoint())?;
    let handle: SdkRadrootsdSignerSessionHandle = connect_client
        .radrootsd()
        .signer_sessions()
        .connect_bunker(
            "bunker://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
        )
        .await?;

    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-session-close",
            "result": {
                "closed": true
            }
        }),
    )
    .await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let result = client
        .radrootsd()
        .signer_sessions()
        .close(handle.session())
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "nip46.session.close");
    assert_eq!(request_json["params"]["session_id"], "session-123");
    assert!(result.closed);

    Ok(())
}

#[tokio::test]
async fn radrootsd_signer_session_connect_rejects_relay_transport_mode() -> TestResult<()> {
    let client = RadrootsSdkClient::from_config(RadrootsSdkConfig::production())?;
    let request = SdkRadrootsdSignerSessionConnectRequest::bunker(
        "bunker://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
    );

    let error = client
        .radrootsd()
        .signer_sessions()
        .connect(&request)
        .await
        .expect_err("unsupported transport");

    assert!(matches!(
        error,
        SdkRadrootsdSessionError::UnsupportedTransport {
            transport: SdkTransportMode::RelayDirect,
            operation: "radrootsd.signer_sessions.connect",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn radrootsd_listing_publish_accepts_sdk_built_draft() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-listing-publish",
            "result": {
                "deduplicated": false,
                "job": {
                    "job_id": "job-1",
                    "command": "bridge.listing.publish",
                    "status": "published",
                    "terminal": true,
                    "recovered_after_restart": false,
                    "signer_mode": "nip46_session:session-123",
                    "signer_session_id": "session-123",
                    "event_kind": 30402,
                    "event_id": "event-1",
                    "event_addr": "30402:seller:listing-1",
                    "relay_count": 1,
                    "acknowledged_relay_count": 1
                }
            }
        }),
    )
    .await?;

    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::Nip46;
    config.radrootsd = RadrootsdConfig {
        endpoint: Some(server.endpoint().to_owned()),
        auth: RadrootsdAuth::BearerToken("sdk-secret".to_owned()),
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let draft = client.listing().build_draft(&sample_listing())?;
    let options = SdkRadrootsdListingPublishOptions {
        signer_session_id: "session-123".to_owned(),
        idempotency_key: Some("idem-1".to_owned()),
    };

    let receipt = client
        .listing()
        .publish_draft_via_radrootsd(draft, &options)
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.listing.publish");
    assert_eq!(request_json["params"]["signer_session_id"], "session-123");
    assert_eq!(request_json["params"]["idempotency_key"], "idem-1");
    assert_eq!(request_json["params"]["kind"], 30402);
    assert_eq!(
        request_json["params"]["listing"]["d_tag"],
        "AAAAAAAAAAAAAAAAAAAAAg"
    );

    assert_eq!(receipt.transport, SdkTransportMode::Radrootsd);
    assert_eq!(receipt.event_kind, Some(30402));
    assert_eq!(receipt.event_id, Some("event-1".to_owned()));
    match receipt.transport_receipt {
        SdkTransportReceipt::Radrootsd(rpc_receipt) => {
            assert!(rpc_receipt.accepted);
            assert!(!rpc_receipt.deduplicated);
            assert_eq!(rpc_receipt.job_id.as_deref(), Some("job-1"));
            assert_eq!(rpc_receipt.status.as_deref(), Some("published"));
            assert_eq!(
                rpc_receipt.signer_session_id.as_deref(),
                Some("session-123")
            );
            assert_eq!(
                rpc_receipt.event_addr.as_deref(),
                Some("30402:seller:listing-1")
            );
            assert_eq!(rpc_receipt.relay_count, Some(1));
            assert_eq!(rpc_receipt.acknowledged_relay_count, Some(1));
        }
        SdkTransportReceipt::RelayDirect(_) => panic!("unexpected relay receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn radrootsd_listing_publish_accepts_typed_listing_value() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-listing-publish",
            "result": {
                "deduplicated": false,
                "job": {
                    "job_id": "job-2",
                    "command": "bridge.listing.publish",
                    "status": "published",
                    "terminal": true,
                    "recovered_after_restart": false,
                    "signer_mode": "nip46_session:session-456",
                    "signer_session_id": "session-456",
                    "event_kind": 30402,
                    "event_id": "event-2",
                    "event_addr": "30402:seller:listing-2",
                    "relay_count": 1,
                    "acknowledged_relay_count": 1
                }
            }
        }),
    )
    .await?;

    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::Nip46;
    config.radrootsd = RadrootsdConfig {
        endpoint: Some(server.endpoint().to_owned()),
        auth: RadrootsdAuth::BearerToken("sdk-secret".to_owned()),
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let mut options = SdkRadrootsdListingPublishOptions::new("session-456");
    options.idempotency_key = Some("idem-2".to_owned());

    let receipt = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &options)
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.listing.publish");
    assert_eq!(request_json["params"]["signer_session_id"], "session-456");
    assert_eq!(request_json["params"]["idempotency_key"], "idem-2");
    assert_eq!(request_json["params"]["kind"], 30402);
    assert_eq!(
        request_json["params"]["listing"]["d_tag"],
        "AAAAAAAAAAAAAAAAAAAAAg"
    );

    assert_eq!(receipt.transport, SdkTransportMode::Radrootsd);
    assert_eq!(receipt.event_kind, Some(30402));
    assert_eq!(receipt.event_id, Some("event-2".to_owned()));

    Ok(())
}

#[tokio::test]
async fn radrootsd_listing_publish_rejects_draft_only_signer_mode() -> TestResult<()> {
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::DraftOnly;
    let client = RadrootsSdkClient::from_config(config)?;
    let options = SdkRadrootsdListingPublishOptions::new("session-123");

    let error = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &options)
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::Radrootsd,
            signer: SignerConfig::DraftOnly,
            required: SignerConfig::Nip46,
            operation: "listing.publish_via_radrootsd",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn radrootsd_listing_publish_rejects_local_identity_signer_mode() -> TestResult<()> {
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::LocalIdentity;
    let client = RadrootsSdkClient::from_config(config)?;
    let options = SdkRadrootsdListingPublishOptions::new("session-123");

    let error = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &options)
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::Radrootsd,
            signer: SignerConfig::LocalIdentity,
            required: SignerConfig::Nip46,
            operation: "listing.publish_via_radrootsd",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn radrootsd_listing_publish_rejects_relay_transport_mode() -> TestResult<()> {
    let client = RadrootsSdkClient::from_config(RadrootsSdkConfig::production())?;
    let options = SdkRadrootsdListingPublishOptions::new("session-123");

    let error = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &options)
        .await
        .expect_err("unsupported transport");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedTransport {
            transport: SdkTransportMode::RelayDirect,
            operation: "listing.publish_via_radrootsd",
        }
    ));

    Ok(())
}

#[test]
fn radrootsd_listing_request_from_event_rejects_listing_draft_kind() -> TestResult<()> {
    let draft = radroots_sdk::listing::build_draft(&sample_listing())?;
    let mut event = sdk_event("seller", 1_720_000_000, draft);
    event.kind = KIND_LISTING_DRAFT;

    assert!(matches!(
        SdkRadrootsdListingPublishRequest::from_event(&event, "session-123", None, None),
        Err(RadrootsTradeListingParseError::InvalidKind(
            KIND_LISTING_DRAFT
        ))
    ));

    Ok(())
}
