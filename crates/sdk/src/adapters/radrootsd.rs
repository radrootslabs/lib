use core::fmt;
use core::time::Duration;

use crate::RadrootsNostrEvent;
use crate::config::RadrootsdAuth;
use crate::listing;
use crate::listing::RadrootsListing;
use radroots_events::kinds::KIND_LISTING;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
    ) -> Result<Self, listing::RadrootsTradeListingParseError> {
        if event.kind != KIND_LISTING {
            return Err(listing::RadrootsTradeListingParseError::InvalidKind(
                event.kind,
            ));
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
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|err| RadrootsdError::Http(format!("build radrootsd client: {err}")))?;
    let mut request_builder = client
        .post(endpoint)
        .headers(auth_headers(auth)?)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "radroots-sdk-listing-publish",
            "method": "bridge.listing.publish",
            "params": request,
        }));

    request_builder = request_builder.header(CONTENT_TYPE, "application/json");

    let response = request_builder.send().await.map_err(|err| {
        RadrootsdError::Http(format!("send radrootsd listing publish request: {err}"))
    })?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| RadrootsdError::Http(format!("read radrootsd response body: {err}")))?;

    if !status.is_success() {
        return Err(RadrootsdError::Http(format!(
            "radrootsd returned http {}: {}",
            status.as_u16(),
            body
        )));
    }

    let envelope: JsonRpcEnvelope<SdkRadrootsdBridgePublishResponse> =
        serde_json::from_str(body.as_str()).map_err(|err| {
            RadrootsdError::MalformedResponse(format!(
                "decode radrootsd bridge.listing.publish response: {err}"
            ))
        })?;
    match (envelope.result, envelope.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => Err(RadrootsdError::JsonRpc(format!(
            "radrootsd bridge.listing.publish failed {}: {}",
            error.code, error.message
        ))),
        (Some(_), Some(error)) => Err(RadrootsdError::MalformedResponse(format!(
            "radrootsd bridge.listing.publish returned result and error: {} {}",
            error.code, error.message
        ))),
        (None, None) => Err(RadrootsdError::MalformedResponse(
            "radrootsd bridge.listing.publish returned neither result nor error".to_owned(),
        )),
    }
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
