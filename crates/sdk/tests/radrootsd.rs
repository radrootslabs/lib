#![cfg(feature = "radrootsd-client")]

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::kinds::KIND_LISTING_DRAFT;
use radroots_sdk::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingLocation,
    RadrootsListingProduct, RadrootsListingStatus, RadrootsTradeListingParseError,
};
use radroots_sdk::{
    RadrootsNostrEvent, RadrootsSdkClient, RadrootsSdkConfig, RadrootsdAuth, RadrootsdConfig,
    SdkEnvironment, SdkPublishError, SdkRadrootsdBridgeJob, SdkRadrootsdBridgePublishResponse,
    SdkRadrootsdListingPublishRequest, SdkRadrootsdPublishReceipt, SdkRadrootsdSignerAuthority,
    SdkTransportMode, SdkTransportReceipt, SignerConfig,
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
    let event = sdk_event("seller", 1_720_000_000, draft);
    let request = SdkRadrootsdListingPublishRequest::from_event(
        &event,
        "session-123",
        None,
        Some("idem-1".to_owned()),
    )?;

    let receipt = client.listing().publish_via_radrootsd(&request).await?;
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
async fn radrootsd_listing_publish_rejects_draft_only_signer_mode() -> TestResult<()> {
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::DraftOnly;
    let client = RadrootsSdkClient::from_config(config)?;
    let request = SdkRadrootsdListingPublishRequest {
        listing: sample_listing(),
        kind: None,
        signer_session_id: "session-123".to_owned(),
        signer_authority: None,
        idempotency_key: None,
    };

    let error = client
        .listing()
        .publish_via_radrootsd(&request)
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
    let request = SdkRadrootsdListingPublishRequest {
        listing: sample_listing(),
        kind: None,
        signer_session_id: "session-123".to_owned(),
        signer_authority: None,
        idempotency_key: None,
    };

    let error = client
        .listing()
        .publish_via_radrootsd(&request)
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
    let request = SdkRadrootsdListingPublishRequest {
        listing: sample_listing(),
        kind: None,
        signer_session_id: "session-123".to_owned(),
        signer_authority: None,
        idempotency_key: None,
    };

    let error = client
        .listing()
        .publish_via_radrootsd(&request)
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
