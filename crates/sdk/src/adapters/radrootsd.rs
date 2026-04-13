use core::fmt;
use core::time::Duration;

use crate::RadrootsNostrEvent;
use crate::config::RadrootsdAuth;
use crate::listing;
use crate::listing::RadrootsListing;
use radroots_events::kinds::KIND_LISTING;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct SdkRadrootsdSignerSessionConnectResponse {
    pub session_id: String,
    pub mode: SdkRadrootsdSignerSessionMode,
    pub remote_signer_pubkey: String,
    pub client_pubkey: String,
    pub relays: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct SdkRadrootsdSignerSessionViewResponse {
    pub session_id: String,
    pub role: SdkRadrootsdSignerSessionRole,
    pub client_pubkey: String,
    pub signer_pubkey: String,
    #[serde(default)]
    pub user_pubkey: Option<String>,
    pub relays: Vec<String>,
    pub permissions: Vec<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    pub auth_required: bool,
    pub authorized: bool,
    #[serde(default)]
    pub auth_url: Option<String>,
    #[serde(default)]
    pub expires_in_secs: Option<u64>,
    #[serde(default)]
    pub signer_authority: Option<SdkRadrootsdSignerAuthority>,
}

impl fmt::Debug for SdkRadrootsdSignerSessionViewResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdSignerSessionViewResponse");
        debug.field("session_id", &"<redacted>");
        debug.field("role", &self.role);
        debug.field("client_pubkey", &self.client_pubkey);
        debug.field("signer_pubkey", &self.signer_pubkey);
        debug.field("user_pubkey", &self.user_pubkey);
        debug.field("relays", &self.relays);
        debug.field("permissions", &self.permissions);
        debug.field("name", &self.name);
        debug.field("url", &self.url);
        debug.field("image", &self.image);
        debug.field("auth_required", &self.auth_required);
        debug.field("authorized", &self.authorized);
        debug.field("auth_url", &self.auth_url);
        debug.field("expires_in_secs", &self.expires_in_secs);
        debug.field("signer_authority", &self.signer_authority);
        debug.finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct SdkRadrootsdSignerSessionAuthorizeResponse {
    pub authorized: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct SdkRadrootsdSignerSessionRequireAuthResponse {
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct SdkRadrootsdSignerSessionCloseResponse {
    pub closed: bool,
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

#[derive(Debug, Serialize)]
struct SdkRadrootsdSignerSessionParams<'a> {
    session_id: &'a str,
}

#[derive(Debug, Serialize)]
struct SdkRadrootsdSignerSessionRequireAuthParams<'a> {
    session_id: &'a str,
    auth_url: &'a str,
}

#[derive(Debug, Serialize)]
struct SdkRadrootsdBridgeJobParams<'a> {
    job_id: &'a str,
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

pub(crate) async fn connect_signer_session(
    endpoint: &str,
    auth: &RadrootsdAuth,
    request: &SdkRadrootsdSignerSessionConnectRequest,
    timeout: Duration,
) -> Result<SdkRadrootsdSignerSessionConnectResponse, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-nip46-connect",
        "nip46.connect",
        request,
        timeout,
    )
    .await
}

pub(crate) async fn signer_session_status(
    endpoint: &str,
    auth: &RadrootsdAuth,
    session_id: &str,
    timeout: Duration,
) -> Result<SdkRadrootsdSignerSessionViewResponse, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-nip46-session-status",
        "nip46.session.status",
        &SdkRadrootsdSignerSessionParams { session_id },
        timeout,
    )
    .await
}

pub(crate) async fn list_signer_sessions(
    endpoint: &str,
    auth: &RadrootsdAuth,
    timeout: Duration,
) -> Result<Vec<SdkRadrootsdSignerSessionViewResponse>, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-nip46-session-list",
        "nip46.session.list",
        &json!({}),
        timeout,
    )
    .await
}

pub(crate) async fn authorize_signer_session(
    endpoint: &str,
    auth: &RadrootsdAuth,
    session_id: &str,
    timeout: Duration,
) -> Result<SdkRadrootsdSignerSessionAuthorizeResponse, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-nip46-session-authorize",
        "nip46.session.authorize",
        &SdkRadrootsdSignerSessionParams { session_id },
        timeout,
    )
    .await
}

pub(crate) async fn require_signer_session_auth(
    endpoint: &str,
    auth: &RadrootsdAuth,
    session_id: &str,
    auth_url: &str,
    timeout: Duration,
) -> Result<SdkRadrootsdSignerSessionRequireAuthResponse, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-nip46-session-require-auth",
        "nip46.session.require_auth",
        &SdkRadrootsdSignerSessionRequireAuthParams {
            session_id,
            auth_url,
        },
        timeout,
    )
    .await
}

pub(crate) async fn close_signer_session(
    endpoint: &str,
    auth: &RadrootsdAuth,
    session_id: &str,
    timeout: Duration,
) -> Result<SdkRadrootsdSignerSessionCloseResponse, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-nip46-session-close",
        "nip46.session.close",
        &SdkRadrootsdSignerSessionParams { session_id },
        timeout,
    )
    .await
}

pub(crate) async fn bridge_status(
    endpoint: &str,
    auth: &RadrootsdAuth,
    timeout: Duration,
) -> Result<SdkRadrootsdBridgeStatusResponse, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-bridge-status",
        "bridge.status",
        &json!({}),
        timeout,
    )
    .await
}

pub(crate) async fn bridge_job_status(
    endpoint: &str,
    auth: &RadrootsdAuth,
    job_id: &str,
    timeout: Duration,
) -> Result<SdkRadrootsdBridgeJobView, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-bridge-job-status",
        "bridge.job.status",
        &SdkRadrootsdBridgeJobParams { job_id },
        timeout,
    )
    .await
}

pub(crate) async fn list_bridge_jobs(
    endpoint: &str,
    auth: &RadrootsdAuth,
    timeout: Duration,
) -> Result<Vec<SdkRadrootsdBridgeJobView>, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        "radroots-sdk-bridge-job-list",
        "bridge.job.list",
        &json!({}),
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
        return Err(RadrootsdError::Http(format!(
            "radrootsd returned http {}: {}",
            status.as_u16(),
            body
        )));
    }

    let envelope: JsonRpcEnvelope<R> = serde_json::from_str(body.as_str()).map_err(|err| {
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
