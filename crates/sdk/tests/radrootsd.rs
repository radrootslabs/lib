#![cfg(feature = "radrootsd-client")]

use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_events::farm::{RadrootsFarm, RadrootsFarmLocation, RadrootsFarmRef};
use radroots_events::ids::RadrootsPublicKey;
use radroots_events::kinds::{
    KIND_FARM, KIND_LISTING, KIND_LISTING_DRAFT, KIND_ORDER_REQUEST, KIND_PROFILE,
};
use radroots_sdk::adapters::radrootsd::{
    SdkRadrootsdBridgeDeliveryPolicy, SdkRadrootsdBridgeJob, SdkRadrootsdBridgeJobStatus,
    SdkRadrootsdBridgePublishResponse, SdkRadrootsdListingPublishRequest,
    SdkRadrootsdSignerAuthority, SdkRadrootsdSignerSessionConnectRequest,
    SdkRadrootsdSignerSessionMode, SdkRadrootsdSignerSessionRole,
};
use radroots_sdk::client::{
    RadrootsSdkClient, SdkPublishError, SdkRadrootsdBridgeError, SdkRadrootsdFarmPublishOptions,
    SdkRadrootsdListingPublishOptions, SdkRadrootsdOrderRequestPublishOptions,
    SdkRadrootsdProfilePublishOptions, SdkRadrootsdPublishReceipt, SdkRadrootsdSessionError,
    SdkRadrootsdSignerSessionHandle, SdkRadrootsdSignerSessionView, SdkTransportReceipt,
};
use radroots_sdk::config::{
    RadrootsSdkConfig, RadrootsdAuth, RadrootsdConfig, SdkConfigError, SdkEnvironment,
    SdkTransportMode, SignerConfig,
};
use radroots_sdk::protocol::events::{RadrootsNostrEvent, RadrootsNostrEventPtr};
use radroots_sdk::protocol::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingLocation, RadrootsListingParseError,
    RadrootsListingProduct, RadrootsListingStatus,
};
use radroots_sdk::protocol::order::{
    RadrootsOrderEconomicItem, RadrootsOrderEconomicLine, RadrootsOrderEconomics,
    RadrootsOrderItem, RadrootsOrderPricingBasis, RadrootsOrderRequest,
};
use radroots_sdk::protocol::profile::{RadrootsProfile, RadrootsProfileType};
use serde_json::{Value, json};
use std::collections::VecDeque;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

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

struct JsonRpcSequenceServer {
    endpoint: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl JsonRpcSequenceServer {
    async fn spawn(
        expected_auth: Option<&str>,
        response_bodies: Vec<Value>,
    ) -> TestResult<(Self, mpsc::UnboundedReceiver<Value>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let endpoint = format!("http://{addr}/jsonrpc");
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let expected_auth = expected_auth.map(str::to_owned);
        let mut response_texts = response_bodies
            .into_iter()
            .map(|value| value.to_string())
            .collect::<VecDeque<_>>();

        tokio::spawn(async move {
            loop {
                if response_texts.is_empty() {
                    break;
                }

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
                        let Some(response_text) = response_texts.pop_front() else {
                            return;
                        };
                        let _ = write_http_response(&mut stream, 200, response_text.as_str()).await;
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

impl Drop for JsonRpcSequenceServer {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
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
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".parse().expect("listing d tag"),
        published_at: None,
        farm: RadrootsFarmRef {
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
        about: Some("Coffee farm".into()),
        website: Some("https://example.invalid/north-farm".into()),
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
        about: Some("Coffee farm".into()),
        website: Some("https://example.invalid/north-farm".into()),
        picture: None,
        banner: None,
        location: Some(RadrootsFarmLocation {
            primary: Some("North Farm".into()),
            city: Some("San Francisco".into()),
            region: Some("CA".into()),
            country: Some("US".into()),
            gcs: None,
        }),
        tags: Some(vec!["coffee".into()]),
    }
}

fn sample_order_request_economics() -> RadrootsOrderEconomics {
    RadrootsOrderEconomics {
        quote_id: "quote-1".parse().expect("quote id"),
        quote_version: 1,
        pricing_basis: RadrootsOrderPricingBasis::ListingEvent,
        currency: RadrootsCoreCurrency::USD,
        items: vec![RadrootsOrderEconomicItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 2,
            quantity_amount: RadrootsCoreDecimal::from(1u32),
            quantity_unit: RadrootsCoreUnit::MassG,
            unit_price_amount: RadrootsCoreDecimal::from(20u32),
            unit_price_currency: RadrootsCoreCurrency::USD,
            line_subtotal: RadrootsCoreMoney::new(
                RadrootsCoreDecimal::from(40u32),
                RadrootsCoreCurrency::USD,
            ),
        }],
        discounts: Vec::<RadrootsOrderEconomicLine>::new(),
        adjustments: Vec::<RadrootsOrderEconomicLine>::new(),
        subtotal: RadrootsCoreMoney::new(
            RadrootsCoreDecimal::from(40u32),
            RadrootsCoreCurrency::USD,
        ),
        discount_total: RadrootsCoreMoney::new(
            RadrootsCoreDecimal::from(0u32),
            RadrootsCoreCurrency::USD,
        ),
        adjustment_total: RadrootsCoreMoney::new(
            RadrootsCoreDecimal::from(0u32),
            RadrootsCoreCurrency::USD,
        ),
        total: RadrootsCoreMoney::new(RadrootsCoreDecimal::from(40u32), RadrootsCoreCurrency::USD),
    }
}

fn sample_order_request() -> RadrootsOrderRequest {
    let seller_pubkey: RadrootsPublicKey = "a".repeat(64).parse().expect("seller public key");

    RadrootsOrderRequest {
        order_id: "order-1".parse().expect("order id"),
        listing_addr: format!("{KIND_LISTING}:{seller_pubkey}:AAAAAAAAAAAAAAAAAAAAAg")
            .parse()
            .expect("listing address"),
        buyer_pubkey: "b".repeat(64).parse().expect("buyer public key"),
        seller_pubkey,
        items: vec![RadrootsOrderItem {
            bin_id: "bin-1".parse().expect("bin id"),
            bin_count: 2,
        }],
        economics: sample_order_request_economics(),
    }
}

fn listing_event_ptr_with_relays(relays: Option<&str>) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: "a".repeat(64),
        relays: relays.map(str::to_owned),
    }
}

fn sdk_event(
    author: &str,
    created_at: u32,
    draft: radroots_sdk::protocol::listing::RadrootsListingDraft,
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

fn sample_bridge_status_json() -> Value {
    json!({
        "enabled": true,
        "ready": true,
        "auth_mode": "bearer_token",
        "signer_mode": "selectable_per_request",
        "default_signer_mode": "embedded_service_identity",
        "supported_signer_modes": ["embedded_service_identity", "nip46_session"],
        "available_nip46_signer_sessions": 2,
        "relay_count": 1,
        "delivery_policy": "quorum",
        "delivery_quorum": 1,
        "publish_max_attempts": 3,
        "publish_initial_backoff_millis": 250,
        "publish_max_backoff_millis": 4000,
        "job_status_retention": 64,
        "retained_jobs": 4,
        "retained_idempotency_keys": 2,
        "accepted_jobs": 1,
        "published_jobs": 2,
        "failed_jobs": 1,
        "recovered_failed_jobs": 0,
        "methods": ["bridge.status", "bridge.job.status", "bridge.job.list", "bridge.listing.publish"]
    })
}

fn sample_bridge_job_json(job_id: &str) -> Value {
    sample_bridge_job_json_for(job_id, "bridge.listing.publish", 30402)
}

fn sample_bridge_job_json_for(job_id: &str, command: &str, event_kind: u32) -> Value {
    json!({
        "job_id": job_id,
        "command": command,
        "idempotency_key": "idem-bridge-1",
        "status": "published",
        "terminal": true,
        "recovered_after_restart": false,
        "requested_at_unix": 1720000000u64,
        "completed_at_unix": 1720000001u64,
        "signer_mode": "nip46_session",
        "signer_session_id": "session-123",
        "event_kind": event_kind,
        "event_id": "event-bridge-1",
        "event_addr": "30402:seller:listing-bridge-1",
        "delivery_policy": "quorum",
        "delivery_quorum": 1,
        "relay_count": 2,
        "acknowledged_relay_count": 1,
        "required_acknowledged_relay_count": 1,
        "attempt_count": 1,
        "attempt_summaries": ["attempt 1: 1/2 relays acknowledged"],
        "relay_results": [
            {
                "relay_url": "wss://radroots.org",
                "acknowledged": true,
                "detail": null
            },
            {
                "relay_url": "wss://backup.radroots.org",
                "acknowledged": false,
                "detail": "timeout"
            }
        ],
        "relay_outcome_summary": "quorum satisfied with 1/2 relay acknowledgements"
    })
}

async fn connected_bunker_session_handle(
    session_id: &str,
) -> TestResult<SdkRadrootsdSignerSessionHandle> {
    let (server, _) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-nip46-connect",
            "result": {
                "session_id": session_id,
                "mode": "Bunker",
                "remote_signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "relays": ["wss://radroots.org"]
            }
        }),
    )
    .await?;
    let client = radrootsd_test_client(server.endpoint())?;
    client
        .radrootsd()
        .signer_sessions()
        .connect_bunker(
            "bunker://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
        )
        .await
        .map_err(Into::into)
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
async fn radrootsd_signer_session_get_public_key_returns_typed_result() -> TestResult<()> {
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
            "id": "radroots-sdk-nip46-get-public-key",
            "result": {
                "pubkey": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            }
        }),
    )
    .await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let result = client
        .radrootsd()
        .signer_sessions()
        .get_public_key(handle.session())
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "nip46.get_public_key");
    assert_eq!(request_json["params"]["session_id"], "session-123");
    assert_eq!(
        result.pubkey,
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    );

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

    let handle = connected_bunker_session_handle("session-123").await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let draft = client.listing().build_draft(&sample_listing())?;
    let options = SdkRadrootsdListingPublishOptions::from_signer_session(&handle)
        .with_idempotency_key("idem-1");

    let receipt = client
        .listing()
        .publish_draft_via_radrootsd_with_options(draft, &options)
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

    let handle = connected_bunker_session_handle("session-456").await?;
    let client = radrootsd_test_client(server.endpoint())?;

    let receipt = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &handle)
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.listing.publish");
    assert_eq!(request_json["params"]["signer_session_id"], "session-456");
    assert!(request_json["params"]["idempotency_key"].is_null());
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
async fn radrootsd_profile_publish_accepts_typed_profile_value() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-profile-publish",
            "result": {
                "deduplicated": false,
                "job": {
                    "job_id": "job-profile-1",
                    "command": "bridge.profile.publish",
                    "status": "published",
                    "terminal": true,
                    "recovered_after_restart": false,
                    "signer_mode": "nip46_session:session-profile-1",
                    "signer_session_id": "session-profile-1",
                    "event_kind": 0,
                    "event_id": "event-profile-1",
                    "relay_count": 1,
                    "acknowledged_relay_count": 1
                }
            }
        }),
    )
    .await?;

    let handle = connected_bunker_session_handle("session-profile-1").await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let options = SdkRadrootsdProfilePublishOptions::from_signer_session(&handle)
        .with_idempotency_key("profile-idem-1")
        .with_signer_authority(SdkRadrootsdSignerAuthority {
            provider_runtime_id: "runtime-profile".to_owned(),
            account_identity_id: "identity-profile".to_owned(),
            provider_signer_session_id: Some("provider-session-profile".to_owned()),
        });

    let receipt = client
        .profile()
        .publish_profile_via_radrootsd_with_options(
            &sample_profile(),
            Some(RadrootsProfileType::Farm),
            &options,
        )
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.profile.publish");
    assert_eq!(
        request_json["params"]["signer_session_id"],
        "session-profile-1"
    );
    assert_eq!(request_json["params"]["profile_type"], "farm");
    assert_eq!(request_json["params"]["profile"]["name"], "North Farm");
    assert_eq!(request_json["params"]["idempotency_key"], "profile-idem-1");
    assert_eq!(
        request_json["params"]["signer_authority"]["provider_runtime_id"],
        "runtime-profile"
    );
    assert_eq!(receipt.event_kind, Some(KIND_PROFILE));
    assert_eq!(receipt.event_id, Some("event-profile-1".to_owned()));

    Ok(())
}

#[tokio::test]
async fn radrootsd_farm_publish_accepts_typed_farm_value() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-farm-publish",
            "result": {
                "deduplicated": false,
                "job": {
                    "job_id": "job-farm-1",
                    "command": "bridge.farm.publish",
                    "status": "published",
                    "terminal": true,
                    "recovered_after_restart": false,
                    "signer_mode": "nip46_session:session-farm-1",
                    "signer_session_id": "session-farm-1",
                    "event_kind": 30340,
                    "event_id": "event-farm-1",
                    "event_addr": "30340:seller:AAAAAAAAAAAAAAAAAAAAAA",
                    "relay_count": 1,
                    "acknowledged_relay_count": 1
                }
            }
        }),
    )
    .await?;

    let handle = connected_bunker_session_handle("session-farm-1").await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let options = SdkRadrootsdFarmPublishOptions::from_signer_session(&handle)
        .with_idempotency_key("farm-idem-1");

    let receipt = client
        .farm()
        .publish_farm_via_radrootsd_with_options(&sample_farm(), &options)
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.farm.publish");
    assert_eq!(
        request_json["params"]["signer_session_id"],
        "session-farm-1"
    );
    assert_eq!(request_json["params"]["kind"], KIND_FARM);
    assert_eq!(
        request_json["params"]["farm"]["d_tag"],
        "AAAAAAAAAAAAAAAAAAAAAA"
    );
    assert_eq!(request_json["params"]["idempotency_key"], "farm-idem-1");
    assert_eq!(receipt.event_kind, Some(KIND_FARM));
    assert_eq!(receipt.event_id, Some("event-farm-1".to_owned()));
    match receipt.transport_receipt {
        SdkTransportReceipt::Radrootsd(receipt) => {
            assert_eq!(
                receipt.event_addr,
                Some("30340:seller:AAAAAAAAAAAAAAAAAAAAAA".to_owned())
            );
        }
        SdkTransportReceipt::RelayDirect(_) => panic!("unexpected relay receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn radrootsd_listing_publish_with_options_forwards_typed_continuity_metadata()
-> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-listing-publish",
            "result": {
                "deduplicated": false,
                "job": {
                    "job_id": "job-3",
                    "command": "bridge.listing.publish",
                    "status": "published",
                    "terminal": true,
                    "recovered_after_restart": false,
                    "signer_mode": "nip46_session:session-789",
                    "signer_session_id": "session-789",
                    "event_kind": 30402,
                    "event_id": "event-3",
                    "event_addr": "30402:seller:listing-3",
                    "relay_count": 1,
                    "acknowledged_relay_count": 1
                }
            }
        }),
    )
    .await?;

    let handle = connected_bunker_session_handle("session-789").await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let options = SdkRadrootsdListingPublishOptions::from_signer_session(&handle)
        .with_idempotency_key("idem-3")
        .with_signer_authority(SdkRadrootsdSignerAuthority {
            provider_runtime_id: "runtime-1".to_owned(),
            account_identity_id: "identity-1".to_owned(),
            provider_signer_session_id: Some("provider-session-123".to_owned()),
        });

    let receipt = client
        .listing()
        .publish_listing_via_radrootsd_with_options(&sample_listing(), &options)
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.listing.publish");
    assert_eq!(request_json["params"]["signer_session_id"], "session-789");
    assert_eq!(request_json["params"]["idempotency_key"], "idem-3");
    assert_eq!(
        request_json["params"]["signer_authority"]["provider_runtime_id"],
        "runtime-1"
    );
    assert_eq!(
        request_json["params"]["signer_authority"]["account_identity_id"],
        "identity-1"
    );
    assert_eq!(
        request_json["params"]["signer_authority"]["provider_signer_session_id"],
        "provider-session-123"
    );
    assert_eq!(receipt.event_id, Some("event-3".to_owned()));

    Ok(())
}

#[tokio::test]
async fn radrootsd_listing_publish_rejects_draft_only_signer_mode() -> TestResult<()> {
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Production);
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::DraftOnly;
    let client = RadrootsSdkClient::from_config(config)?;
    let handle = connected_bunker_session_handle("session-123").await?;

    let error = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &handle)
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
    let handle = connected_bunker_session_handle("session-123").await?;

    let error = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &handle)
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
    let handle = connected_bunker_session_handle("session-123").await?;

    let error = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &handle)
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

#[tokio::test]
async fn radrootsd_order_request_publish_accepts_session_handle() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-order-request-publish",
            "result": {
                "deduplicated": false,
                "job": {
                    "job_id": "job-order-1",
                    "command": "bridge.order.request",
                    "status": "published",
                    "terminal": true,
                    "recovered_after_restart": false,
                    "signer_mode": "nip46_session:session-order-1",
                    "signer_session_id": "session-order-1",
                    "event_kind": KIND_ORDER_REQUEST,
                    "event_id": "event-order-1",
                    "event_addr": format!("{KIND_LISTING}:{}:AAAAAAAAAAAAAAAAAAAAAg", "a".repeat(64)),
                    "relay_count": 1,
                    "acknowledged_relay_count": 1
                }
            }
        }),
    )
    .await?;

    let handle = connected_bunker_session_handle("session-order-1").await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let options = SdkRadrootsdOrderRequestPublishOptions::from_signer_session(&handle)
        .with_idempotency_key("idem-order-1")
        .with_signer_authority(SdkRadrootsdSignerAuthority {
            provider_runtime_id: "runtime-1".to_owned(),
            account_identity_id: "identity-1".to_owned(),
            provider_signer_session_id: Some("provider-session-order-1".to_owned()),
        });

    let receipt = client
        .order()
        .publish_order_request_via_radrootsd_with_options(
            &sample_order_request(),
            &listing_event_ptr_with_relays(Some("wss://radroots.org")),
            &options,
        )
        .await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.order.request");
    assert_eq!(
        request_json["params"]["signer_session_id"],
        "session-order-1"
    );
    assert_eq!(request_json["params"]["idempotency_key"], "idem-order-1");
    assert_eq!(request_json["params"]["order"]["order_id"], "order-1");
    assert_eq!(
        request_json["params"]["listing_event"]["id"],
        "a".repeat(64)
    );
    assert_eq!(
        request_json["params"]["listing_event"]["relays"],
        "wss://radroots.org"
    );
    assert_eq!(
        request_json["params"]["signer_authority"]["provider_runtime_id"],
        "runtime-1"
    );
    assert_eq!(
        request_json["params"]["signer_authority"]["provider_signer_session_id"],
        "provider-session-order-1"
    );
    assert_eq!(receipt.event_kind, Some(KIND_ORDER_REQUEST));
    assert_eq!(receipt.event_id, Some("event-order-1".to_owned()));

    Ok(())
}

#[tokio::test]
async fn radrootsd_sdk_workflow_chains_session_listing_order_and_bridge_job() -> TestResult<()> {
    let (server, mut request_rx) = JsonRpcSequenceServer::spawn(
        Some("Bearer sdk-secret"),
        vec![
            json!({
                "jsonrpc": "2.0",
                "id": "radroots-sdk-nip46-connect",
                "result": {
                    "session_id": "session-workflow-1",
                    "mode": "Bunker",
                    "remote_signer_pubkey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "client_pubkey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "relays": ["wss://radroots.org"]
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": "radroots-sdk-listing-publish",
                "result": {
                    "deduplicated": false,
                    "job": {
                        "job_id": "job-workflow-listing",
                        "command": "bridge.listing.publish",
                        "status": "published",
                        "terminal": true,
                        "recovered_after_restart": false,
                        "signer_mode": "nip46_session:session-workflow-1",
                        "signer_session_id": "session-workflow-1",
                        "event_kind": 30402,
                        "event_id": "event-workflow-listing",
                        "event_addr": "30402:seller:listing-workflow-1",
                        "relay_count": 1,
                        "acknowledged_relay_count": 1
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": "radroots-sdk-order-request-publish",
                "result": {
                    "deduplicated": false,
                    "job": {
                        "job_id": "job-workflow-order",
                        "command": "bridge.order.request",
                        "status": "published",
                        "terminal": true,
                        "recovered_after_restart": false,
                        "signer_mode": "nip46_session:session-workflow-1",
                        "signer_session_id": "session-workflow-1",
                        "event_kind": KIND_ORDER_REQUEST,
                        "event_id": "event-workflow-order",
                        "event_addr": format!("{KIND_LISTING}:{}:AAAAAAAAAAAAAAAAAAAAAg", "a".repeat(64)),
                        "relay_count": 1,
                        "acknowledged_relay_count": 1
                    }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": "radroots-sdk-bridge-job-status",
                "result": sample_bridge_job_json_for(
                    "job-workflow-order",
                    "bridge.order.request",
                    KIND_ORDER_REQUEST,
                )
            }),
        ],
    )
    .await?;

    let client = radrootsd_test_client(server.endpoint())?;
    let handle = client
        .radrootsd()
        .signer_sessions()
        .connect_bunker(
            "bunker://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?relay=wss%3A%2F%2Fradroots.org&secret=shared-secret",
        )
        .await?;
    assert_eq!(handle.mode(), SdkRadrootsdSignerSessionMode::Bunker);

    let connect_request = request_rx.recv().await.expect("connect request");
    assert_eq!(connect_request["method"], "nip46.connect");

    let listing_receipt = client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &handle)
        .await?;
    let listing_request = request_rx.recv().await.expect("listing publish request");
    assert_eq!(listing_request["method"], "bridge.listing.publish");
    assert_eq!(
        listing_request["params"]["signer_session_id"],
        "session-workflow-1"
    );

    let order_receipt = client
        .order()
        .publish_order_request_via_radrootsd(
            &sample_order_request(),
            &listing_event_ptr_with_relays(Some("wss://radroots.org")),
            &handle,
        )
        .await?;
    let order_request = request_rx.recv().await.expect("order publish request");
    assert_eq!(order_request["method"], "bridge.order.request");
    assert_eq!(
        order_request["params"]["signer_session_id"],
        "session-workflow-1"
    );
    assert_eq!(order_request["params"]["order"]["order_id"], "order-1");
    assert_eq!(
        order_request["params"]["listing_event"]["id"],
        "a".repeat(64)
    );

    let order_job = match &order_receipt.transport_receipt {
        SdkTransportReceipt::Radrootsd(receipt) => receipt.job(),
        SdkTransportReceipt::RelayDirect(_) => None,
    }
    .expect("order publish receipt should expose a bridge job ref");

    let job_view = client.radrootsd().bridge().job(&order_job).await?;
    let job_request = request_rx.recv().await.expect("bridge job request");
    assert_eq!(job_request["method"], "bridge.job.status");
    assert_eq!(job_request["params"]["job_id"], "job-workflow-order");

    assert_eq!(listing_receipt.event_kind, Some(30402));
    assert_eq!(order_receipt.event_kind, Some(KIND_ORDER_REQUEST));
    assert_eq!(job_view.job().job_id(), "job-workflow-order");
    assert_eq!(job_view.command, "bridge.order.request");
    assert_eq!(job_view.status, SdkRadrootsdBridgeJobStatus::Published);

    Ok(())
}

#[tokio::test]
async fn radrootsd_bridge_status_returns_typed_status() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-bridge-status",
            "result": sample_bridge_status_json()
        }),
    )
    .await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let status = client.radrootsd().bridge().status().await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.status");
    assert!(status.enabled);
    assert!(status.ready);
    assert_eq!(
        status.delivery_policy,
        SdkRadrootsdBridgeDeliveryPolicy::Quorum
    );
    assert_eq!(status.delivery_quorum, Some(1));
    assert_eq!(status.available_nip46_signer_sessions, 2);
    assert!(
        status
            .methods
            .contains(&"bridge.listing.publish".to_owned())
    );

    Ok(())
}

#[tokio::test]
async fn radrootsd_bridge_job_status_accepts_typed_job_ref_from_publish_receipt() -> TestResult<()>
{
    let (publish_server, publish_request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-listing-publish",
            "result": {
                "deduplicated": false,
                "job": {
                    "job_id": "job-bridge-1",
                    "command": "bridge.listing.publish",
                    "status": "published",
                    "terminal": true,
                    "recovered_after_restart": false,
                    "signer_mode": "nip46_session:session-123",
                    "signer_session_id": "session-123",
                    "event_kind": 30402,
                    "event_id": "event-bridge-1",
                    "event_addr": "30402:seller:listing-bridge-1",
                    "relay_count": 1,
                    "acknowledged_relay_count": 1
                }
            }
        }),
    )
    .await?;
    let handle = connected_bunker_session_handle("session-123").await?;
    let publish_client = radrootsd_test_client(publish_server.endpoint())?;
    let publish_receipt = publish_client
        .listing()
        .publish_listing_via_radrootsd(&sample_listing(), &handle)
        .await?;
    let publish_request_json = publish_request_rx.await?;
    assert_eq!(publish_request_json["method"], "bridge.listing.publish");

    let job = match &publish_receipt.transport_receipt {
        SdkTransportReceipt::Radrootsd(receipt) => receipt.job(),
        SdkTransportReceipt::RelayDirect(_) => None,
    }
    .expect("publish receipt should expose a bridge job ref");

    let (job_server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-bridge-job-status",
            "result": sample_bridge_job_json("job-bridge-1")
        }),
    )
    .await?;
    let job_client = radrootsd_test_client(job_server.endpoint())?;
    let job_view = job_client.radrootsd().bridge().job(&job).await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.job.status");
    assert_eq!(request_json["params"]["job_id"], "job-bridge-1");
    assert_eq!(job_view.job().job_id(), "job-bridge-1");
    assert_eq!(job_view.status, SdkRadrootsdBridgeJobStatus::Published);
    assert_eq!(
        job_view.delivery_policy,
        SdkRadrootsdBridgeDeliveryPolicy::Quorum
    );
    assert_eq!(job_view.attempt_count, 1);
    assert_eq!(job_view.relay_results.len(), 2);
    assert_eq!(job_view.relay_results[0].relay_url, "wss://radroots.org");
    assert!(job_view.relay_results[0].acknowledged);

    Ok(())
}

#[tokio::test]
async fn radrootsd_bridge_job_list_returns_typed_views() -> TestResult<()> {
    let (server, request_rx) = JsonRpcServer::spawn(
        Some("Bearer sdk-secret"),
        json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-bridge-job-list",
            "result": [
                sample_bridge_job_json("job-bridge-1"),
                sample_bridge_job_json("job-bridge-2")
            ]
        }),
    )
    .await?;
    let client = radrootsd_test_client(server.endpoint())?;
    let jobs = client.radrootsd().bridge().jobs().await?;
    let request_json = request_rx.await?;

    assert_eq!(request_json["method"], "bridge.job.list");
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].job().job_id(), "job-bridge-1");
    assert_eq!(jobs[1].job().job_id(), "job-bridge-2");
    assert_eq!(jobs[0].status, SdkRadrootsdBridgeJobStatus::Published);

    Ok(())
}

#[tokio::test]
async fn radrootsd_bridge_status_rejects_relay_transport_mode() -> TestResult<()> {
    let client = RadrootsSdkClient::from_config(RadrootsSdkConfig::production())?;
    let error = client
        .radrootsd()
        .bridge()
        .status()
        .await
        .expect_err("unsupported transport");

    assert!(matches!(
        error,
        SdkRadrootsdBridgeError::UnsupportedTransport {
            transport: SdkTransportMode::RelayDirect,
            operation: "radrootsd.bridge.status",
        }
    ));

    Ok(())
}

#[test]
fn radrootsd_listing_request_from_event_rejects_listing_draft_kind() -> TestResult<()> {
    let draft = radroots_sdk::protocol::listing::build_draft(&sample_listing())?;
    let mut event = sdk_event("seller", 1_720_000_000, draft);
    event.kind = KIND_LISTING_DRAFT;

    assert!(matches!(
        SdkRadrootsdListingPublishRequest::from_event(&event, "session-123", None, None),
        Err(RadrootsListingParseError::InvalidKind(KIND_LISTING_DRAFT))
    ));

    Ok(())
}
