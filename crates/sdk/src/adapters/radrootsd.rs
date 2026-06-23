use core::fmt;
use core::time::Duration;

use crate::farm::RadrootsFarm;
use crate::listing;
use crate::listing::RadrootsListing;
use crate::profile::{RadrootsProfile, RadrootsProfileType};
use radroots_events::RadrootsNostrEvent;
use radroots_events::kinds::KIND_LISTING;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RadrootsdAuth {
    #[default]
    None,
    BearerToken(String),
}

impl fmt::Debug for RadrootsdAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::BearerToken(_) => f.write_str("BearerToken(<redacted>)"),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkRadrootsdSignerAuthority {
    pub provider_runtime_id: String,
    pub account_identity_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_signer_session_id: Option<String>,
}

impl fmt::Debug for SdkRadrootsdSignerAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdSignerAuthority");
        debug.field("provider_runtime_id", &self.provider_runtime_id);
        debug.field("account_identity_id", &self.account_identity_id);
        debug.field(
            "provider_signer_session_id",
            &self
                .provider_signer_session_id
                .as_ref()
                .map(|_| "<redacted>"),
        );
        debug.finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SdkRadrootsdSignerSessionMode {
    #[serde(alias = "bunker")]
    Bunker,
    #[serde(alias = "nostrconnect")]
    Nostrconnect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkRadrootsdSignerSessionRole {
    InboundLocalSigner,
    OutboundRemoteSigner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkRadrootsdBridgeDeliveryPolicy {
    Any,
    Quorum,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkRadrootsdBridgeJobStatus {
    Accepted,
    Published,
    Failed,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct SdkRadrootsdSignerSessionConnectRequest {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_authority: Option<SdkRadrootsdSignerAuthority>,
}

impl SdkRadrootsdSignerSessionConnectRequest {
    pub fn bunker(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client_secret_key: None,
            signer_authority: None,
        }
    }

    pub fn nostrconnect(url: impl Into<String>, client_secret_key: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client_secret_key: Some(client_secret_key.into()),
            signer_authority: None,
        }
    }

    pub fn with_signer_authority(mut self, signer_authority: SdkRadrootsdSignerAuthority) -> Self {
        self.signer_authority = Some(signer_authority);
        self
    }
}

impl fmt::Debug for SdkRadrootsdSignerSessionConnectRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdSignerSessionConnectRequest");
        debug.field("url", &self.url);
        debug.field(
            "client_secret_key",
            &self.client_secret_key.as_ref().map(|_| "<redacted>"),
        );
        debug.field("signer_authority", &self.signer_authority);
        debug.finish()
    }
}

#[derive(Clone, Serialize)]
pub struct SdkRadrootsdProfilePublishRequest {
    pub profile: RadrootsProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_type: Option<RadrootsProfileType>,
    pub signer_session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_authority: Option<SdkRadrootsdSignerAuthority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl fmt::Debug for SdkRadrootsdProfilePublishRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdProfilePublishRequest");
        debug.field("profile", &self.profile);
        debug.field("profile_type", &self.profile_type);
        debug.field("signer_session_id", &"<redacted>");
        debug.field("signer_authority", &self.signer_authority);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.finish()
    }
}

#[derive(Clone, Serialize)]
pub struct SdkRadrootsdFarmPublishRequest {
    pub farm: RadrootsFarm,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,
    pub signer_session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_authority: Option<SdkRadrootsdSignerAuthority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl fmt::Debug for SdkRadrootsdFarmPublishRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdFarmPublishRequest");
        debug.field("farm", &self.farm);
        debug.field("kind", &self.kind);
        debug.field("signer_session_id", &"<redacted>");
        debug.field("signer_authority", &self.signer_authority);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.finish()
    }
}

#[derive(Clone, Serialize)]
pub struct SdkRadrootsdListingPublishRequest {
    pub listing: RadrootsListing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,
    pub signer_session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_authority: Option<SdkRadrootsdSignerAuthority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl fmt::Debug for SdkRadrootsdListingPublishRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdListingPublishRequest");
        debug.field("listing", &self.listing);
        debug.field("kind", &self.kind);
        debug.field("signer_session_id", &"<redacted>");
        debug.field("signer_authority", &self.signer_authority);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.finish()
    }
}

impl SdkRadrootsdListingPublishRequest {
    pub fn from_event(
        event: &RadrootsNostrEvent,
        signer_session_id: impl Into<String>,
        signer_authority: Option<SdkRadrootsdSignerAuthority>,
        idempotency_key: Option<String>,
    ) -> Result<Self, listing::RadrootsListingParseError> {
        if event.kind != KIND_LISTING {
            return Err(listing::RadrootsListingParseError::InvalidKind(event.kind));
        }
        Ok(Self {
            listing: listing::parse_event(event)?,
            kind: Some(event.kind),
            signer_session_id: signer_session_id.into(),
            signer_authority,
            idempotency_key,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SdkRadrootsdBridgePublishResponse {
    pub deduplicated: bool,
    pub job: SdkRadrootsdBridgeJob,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct SdkRadrootsdBridgeStatusResponse {
    pub enabled: bool,
    pub ready: bool,
    pub auth_mode: String,
    pub signer_mode: String,
    pub default_signer_mode: String,
    pub supported_signer_modes: Vec<String>,
    pub available_nip46_signer_sessions: usize,
    pub relay_count: usize,
    pub delivery_policy: SdkRadrootsdBridgeDeliveryPolicy,
    #[serde(default)]
    pub delivery_quorum: Option<usize>,
    pub publish_max_attempts: usize,
    pub publish_initial_backoff_millis: u64,
    pub publish_max_backoff_millis: u64,
    pub job_status_retention: usize,
    pub retained_jobs: usize,
    pub retained_idempotency_keys: usize,
    pub accepted_jobs: usize,
    pub published_jobs: usize,
    pub failed_jobs: usize,
    pub recovered_failed_jobs: usize,
    pub methods: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkRadrootsdBridgeRelayPublishResult {
    pub relay_url: String,
    pub acknowledged: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct SdkRadrootsdBridgeJob {
    pub job_id: String,
    pub command: String,
    pub status: String,
    pub terminal: bool,
    pub recovered_after_restart: bool,
    pub signer_mode: String,
    #[serde(default)]
    pub signer_session_id: Option<String>,
    pub event_kind: u32,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub event_addr: Option<String>,
    pub relay_count: usize,
    pub acknowledged_relay_count: usize,
}

impl fmt::Debug for SdkRadrootsdBridgeJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdBridgeJob");
        debug.field("job_id", &self.job_id);
        debug.field("command", &self.command);
        debug.field("status", &self.status);
        debug.field("terminal", &self.terminal);
        debug.field("recovered_after_restart", &self.recovered_after_restart);
        debug.field("signer_mode", &"<redacted>");
        debug.field(
            "signer_session_id",
            &self.signer_session_id.as_ref().map(|_| "<redacted>"),
        );
        debug.field("event_kind", &self.event_kind);
        debug.field("event_id", &self.event_id);
        debug.field("event_addr", &self.event_addr);
        debug.field("relay_count", &self.relay_count);
        debug.field("acknowledged_relay_count", &self.acknowledged_relay_count);
        debug.finish()
    }
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct SdkRadrootsdBridgeJobView {
    pub job_id: String,
    pub command: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    pub status: SdkRadrootsdBridgeJobStatus,
    pub terminal: bool,
    pub recovered_after_restart: bool,
    pub requested_at_unix: u64,
    #[serde(default)]
    pub completed_at_unix: Option<u64>,
    pub signer_mode: String,
    #[serde(default)]
    pub signer_session_id: Option<String>,
    pub event_kind: u32,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub event_addr: Option<String>,
    pub delivery_policy: SdkRadrootsdBridgeDeliveryPolicy,
    #[serde(default)]
    pub delivery_quorum: Option<usize>,
    pub relay_count: usize,
    pub acknowledged_relay_count: usize,
    pub required_acknowledged_relay_count: usize,
    pub attempt_count: usize,
    #[serde(default)]
    pub attempt_summaries: Vec<String>,
    #[serde(default)]
    pub relay_results: Vec<SdkRadrootsdBridgeRelayPublishResult>,
    pub relay_outcome_summary: String,
}

impl fmt::Debug for SdkRadrootsdBridgeJobView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdBridgeJobView");
        debug.field("job_id", &self.job_id);
        debug.field("command", &self.command);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.field("status", &self.status);
        debug.field("terminal", &self.terminal);
        debug.field("recovered_after_restart", &self.recovered_after_restart);
        debug.field("requested_at_unix", &self.requested_at_unix);
        debug.field("completed_at_unix", &self.completed_at_unix);
        debug.field("signer_mode", &self.signer_mode.as_str());
        debug.field(
            "signer_session_id",
            &self.signer_session_id.as_ref().map(|_| "<redacted>"),
        );
        debug.field("event_kind", &self.event_kind);
        debug.field("event_id", &self.event_id);
        debug.field("event_addr", &self.event_addr);
        debug.field("delivery_policy", &self.delivery_policy);
        debug.field("delivery_quorum", &self.delivery_quorum);
        debug.field("relay_count", &self.relay_count);
        debug.field("acknowledged_relay_count", &self.acknowledged_relay_count);
        debug.field(
            "required_acknowledged_relay_count",
            &self.required_acknowledged_relay_count,
        );
        debug.field("attempt_count", &self.attempt_count);
        debug.field("attempt_summaries", &self.attempt_summaries);
        debug.field("relay_results", &self.relay_results);
        debug.field("relay_outcome_summary", &self.relay_outcome_summary);
        debug.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsdError {
    InvalidAuthHeader(String),
    Http(String),
    JsonRpc(String),
    MalformedResponse(String),
}

impl core::fmt::Display for RadrootsdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidAuthHeader(value) => {
                write!(f, "invalid radrootsd bearer token header: {value}")
            }
            Self::Http(value) => write!(f, "{value}"),
            Self::JsonRpc(value) => write!(f, "{value}"),
            Self::MalformedResponse(value) => write!(f, "{value}"),
        }
    }
}

impl std::error::Error for RadrootsdError {}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

pub async fn publish_listing(
    endpoint: &str,
    auth: &RadrootsdAuth,
    request: &SdkRadrootsdListingPublishRequest,
    timeout: Duration,
) -> Result<SdkRadrootsdBridgePublishResponse, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-listing-publish",
        "bridge.listing.publish",
        request,
        timeout,
    )
    .await
}

fn auth_headers(auth: &RadrootsdAuth) -> Result<HeaderMap, RadrootsdError> {
    let mut headers = HeaderMap::new();
    match auth {
        RadrootsdAuth::None => Ok(headers),
        RadrootsdAuth::BearerToken(token) => {
            let value = HeaderValue::from_str(format!("Bearer {token}").as_str())
                .map_err(|err| RadrootsdError::InvalidAuthHeader(err.to_string()))?;
            headers.insert(AUTHORIZATION, value);
            Ok(headers)
        }
    }
}

pub fn bridge_listing_publish_request_json(
    request: &SdkRadrootsdListingPublishRequest,
) -> Result<Value, RadrootsdError> {
    serde_json::to_value(request).map_err(|err| {
        RadrootsdError::MalformedResponse(format!(
            "serialize radrootsd listing publish request: {err}"
        ))
    })
}

fn http_status_error(status: reqwest::StatusCode, body: &str) -> RadrootsdError {
    RadrootsdError::Http(format!(
        "radrootsd returned http {}: {}",
        status.as_u16(),
        body
    ))
}

fn decode_jsonrpc_response<R>(method: &str, body: &str) -> Result<R, RadrootsdError>
where
    R: DeserializeOwned,
{
    let envelope: JsonRpcEnvelope<R> = serde_json::from_str(body).map_err(|err| {
        RadrootsdError::MalformedResponse(format!("decode radrootsd {method} response: {err}"))
    })?;
    match (envelope.result, envelope.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => Err(RadrootsdError::JsonRpc(format!(
            "radrootsd {method} failed {}: {}",
            error.code, error.message
        ))),
        (Some(_), Some(error)) => Err(RadrootsdError::MalformedResponse(format!(
            "radrootsd {method} returned result and error: {} {}",
            error.code, error.message
        ))),
        (None, None) => Err(RadrootsdError::MalformedResponse(format!(
            "radrootsd {method} returned neither result nor error"
        ))),
    }
}

async fn jsonrpc_call<P, R>(
    endpoint: &str,
    auth: &RadrootsdAuth,
    request_id: &str,
    method: &str,
    params: &P,
    timeout: Duration,
) -> Result<R, RadrootsdError>
where
    P: Serialize + ?Sized,
    R: DeserializeOwned,
{
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|err| RadrootsdError::Http(format!("build radrootsd client: {err}")))?;
    let mut request_builder = client
        .post(endpoint)
        .headers(auth_headers(auth)?)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        }));

    request_builder = request_builder.header(CONTENT_TYPE, "application/json");

    let response = request_builder
        .send()
        .await
        .map_err(|err| RadrootsdError::Http(format!("send radrootsd {method} request: {err}")))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| RadrootsdError::Http(format!("read radrootsd response body: {err}")))?;

    if !status.is_success() {
        return Err(http_status_error(status, body.as_str()));
    }

    decode_jsonrpc_response(method, body.as_str())
}

#[cfg(test)]
mod tests {
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
        let headers =
            auth_headers(&RadrootsdAuth::BearerToken("sdk-token".into())).expect("headers");

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
        let error =
            auth_headers(&RadrootsdAuth::BearerToken("bad\ntoken".into())).expect_err("error");

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
    fn debug_output_redacts_auth_and_signer_secrets() {
        let auth = RadrootsdAuth::BearerToken("token-secret".into());
        let connect =
            SdkRadrootsdSignerSessionConnectRequest::nostrconnect("nostrconnect://session", "nsec")
                .with_signer_authority(sample_authority());
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

        let rendered = format!("{auth:?} {connect:?} {listing_request:?} {job:?}");

        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("token-secret"));
        assert!(!rendered.contains("nsec"));
        assert!(!rendered.contains("provider-session-secret"));
        assert!(!rendered.contains("signer-session-secret"));
        assert!(!rendered.contains("signer_mode: \"bunker\""));
    }

    #[test]
    fn http_status_error_reports_status_and_body() {
        let error = http_status_error(reqwest::StatusCode::UNAUTHORIZED, "missing token");

        assert_message(error, "radrootsd returned http 401: missing token");
    }

    #[test]
    fn decode_jsonrpc_response_returns_result() {
        let response: SdkRadrootsdBridgePublishResponse = decode_jsonrpc_response(
            "bridge.listing.publish",
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
            r#"{
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
            r#"{ "result": "#,
        )
        .expect_err("error");

        assert!(matches!(error, RadrootsdError::MalformedResponse(_)));
        assert_message(error, "decode radrootsd bridge.listing.publish response");
    }
}
