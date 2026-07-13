use core::fmt;
use core::time::Duration;

use radroots_event::draft::RadrootsSignedEvent;
use radroots_transport_publish_protocol::{
    METHOD_EVENT, SignedEventWire, TransportPublishDeliveryPolicy, TransportPublishEventRequest,
    TransportPublishEventResponse, TransportPublishProtocolError, TransportPublishTargetPolicy,
};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

pub const SDK_RADROOTSD_PROXY_REQUEST_ID: &str = "radroots-sdk-transport-publish-event";
pub const SDK_RADROOTSD_PROXY_MAX_TARGETS: usize = 20;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsdProxyConfig {
    pub endpoint: String,
    pub auth: RadrootsdAuth,
    pub timeout: Duration,
    pub request_timeout_ms: Option<u64>,
}

impl RadrootsdProxyConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            auth: RadrootsdAuth::None,
            timeout: Duration::from_secs(10),
            request_timeout_ms: None,
        }
    }

    pub fn with_auth(mut self, auth: RadrootsdAuth) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_request_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.request_timeout_ms = Some(timeout_ms);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsdProxyPublishAdapter {
    config: RadrootsdProxyConfig,
}

impl RadrootsdProxyPublishAdapter {
    pub fn new(config: RadrootsdProxyConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &RadrootsdProxyConfig {
        &self.config
    }

    pub async fn publish_signed_event(
        &self,
        request: RadrootsdProxyPublishRequest,
    ) -> Result<TransportPublishEventResponse, RadrootsdError> {
        let request = request.into_protocol_request();
        request
            .validate(SDK_RADROOTSD_PROXY_MAX_TARGETS)
            .map_err(RadrootsdError::from_protocol)?;
        let response = publish_event(
            self.config.endpoint.as_str(),
            &self.config.auth,
            &request,
            self.config.timeout,
        )
        .await?;
        validate_transport_publish_response_for_request(&request, &response)?;
        Ok(response)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsdProxyPublishRequest {
    pub signed_event: RadrootsSignedEvent,
    pub target_policy: TransportPublishTargetPolicy,
    pub delivery_policy: TransportPublishDeliveryPolicy,
    pub idempotency_key: Option<String>,
    pub timeout_ms: Option<u64>,
}

impl RadrootsdProxyPublishRequest {
    fn into_protocol_request(self) -> TransportPublishEventRequest {
        TransportPublishEventRequest {
            event: signed_event_wire(&self.signed_event),
            target_policy: self.target_policy,
            delivery_policy: self.delivery_policy,
            idempotency_key: self.idempotency_key,
            timeout_ms: self.timeout_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadrootsdError {
    InvalidAuthHeader(String),
    InvalidRequest(String),
    Http(String),
    JsonRpc { code: i64, message: String },
    MalformedResponse(String),
}

impl RadrootsdError {
    fn from_protocol(error: TransportPublishProtocolError) -> Self {
        Self::InvalidRequest(error.to_string())
    }
}

impl fmt::Display for RadrootsdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAuthHeader(value) => {
                write!(f, "invalid radrootsd bearer token header: {value}")
            }
            Self::InvalidRequest(value) => f.write_str(value),
            Self::Http(value) => f.write_str(value),
            Self::MalformedResponse(value) => f.write_str(value),
            Self::JsonRpc { code, message } => {
                write!(f, "radrootsd jsonrpc failed {code}: {message}")
            }
        }
    }
}

impl std::error::Error for RadrootsdError {}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope<T> {
    jsonrpc: Option<String>,
    id: Option<Value>,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

pub async fn publish_event(
    endpoint: &str,
    auth: &RadrootsdAuth,
    request: &TransportPublishEventRequest,
    timeout: Duration,
) -> Result<TransportPublishEventResponse, RadrootsdError> {
    jsonrpc_call(
        endpoint,
        auth,
        SDK_RADROOTSD_PROXY_REQUEST_ID,
        METHOD_EVENT,
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
            let header = format!("Bearer {token}");
            let value = HeaderValue::from_str(header.as_str())
                .map_err(|err| RadrootsdError::InvalidAuthHeader(err.to_string()))?;
            headers.insert(AUTHORIZATION, value);
            Ok(headers)
        }
    }
}

pub fn publish_event_request_json(
    request: &TransportPublishEventRequest,
) -> Result<Value, RadrootsdError> {
    Ok(serde_json::to_value(request).expect("radrootsd transport publish request serializes"))
}

fn http_status_error(status: reqwest::StatusCode, body: &str) -> RadrootsdError {
    let body_summary = if body.is_empty() {
        "response body empty".to_owned()
    } else {
        format!("response body omitted ({} bytes)", body.len())
    };
    RadrootsdError::Http(format!(
        "radrootsd returned http {}: {}",
        status.as_u16(),
        body_summary
    ))
}

fn decode_jsonrpc_response<R>(
    method: &str,
    expected_id: &str,
    body: &str,
) -> Result<R, RadrootsdError>
where
    R: DeserializeOwned,
{
    let envelope: JsonRpcEnvelope<R> = serde_json::from_str(body).map_err(|err| {
        RadrootsdError::MalformedResponse(format!("decode radrootsd {method} response: {err}"))
    })?;
    if envelope.jsonrpc.as_deref() != Some("2.0") {
        return Err(RadrootsdError::MalformedResponse(format!(
            "radrootsd {method} returned invalid jsonrpc version"
        )));
    }
    let expected_id_value = Value::String(expected_id.to_owned());
    if envelope.id.as_ref() != Some(&expected_id_value) {
        return Err(RadrootsdError::MalformedResponse(format!(
            "radrootsd {method} returned mismatched jsonrpc id"
        )));
    }
    match (envelope.result, envelope.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => Err(RadrootsdError::JsonRpc {
            code: error.code,
            message: error.message,
        }),
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

    decode_jsonrpc_response(method, request_id, body.as_str())
}

fn signed_event_wire(event: &RadrootsSignedEvent) -> SignedEventWire {
    SignedEventWire {
        id: event.id.clone(),
        pubkey: event.pubkey.clone(),
        created_at: event.created_at as u64,
        kind: event.kind,
        tags: event.tags.clone(),
        content: event.content.clone(),
        sig: event.sig.clone(),
    }
}

fn validate_transport_publish_response_for_request(
    request: &TransportPublishEventRequest,
    response: &TransportPublishEventResponse,
) -> Result<(), RadrootsdError> {
    response.job.validate().map_err(|error| {
        RadrootsdError::MalformedResponse(format!(
            "radrootsd transport publish response invalid: {error}"
        ))
    })?;
    if response.job.event_id != request.event.id {
        return Err(response_mismatch("event_id"));
    }
    if response.job.pubkey != request.event.pubkey {
        return Err(response_mismatch("pubkey"));
    }
    if response.job.event_kind != request.event.kind {
        return Err(response_mismatch("event_kind"));
    }
    if response.job.delivery_policy != request.delivery_policy {
        return Err(response_mismatch("delivery_policy"));
    }
    if response.job.target_policy != request.target_policy {
        return Err(response_mismatch("target_policy"));
    }
    Ok(())
}

fn response_mismatch(field: &str) -> RadrootsdError {
    RadrootsdError::MalformedResponse(format!(
        "radrootsd transport publish response {field} does not match request"
    ))
}

#[cfg(test)]
#[path = "../../tests/unit/adapters_radrootsd_tests.rs"]
mod tests;
