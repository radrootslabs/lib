//! Bounded HTTP/1.1 administration server over an owned Unix listener.

use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, de, de::DeserializeOwned};
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::error::Error;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use http::{Method, Request, Response, StatusCode, Version};
use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

#[cfg(test)]
use super::test_support;
use super::{
    ADMIN_CONTRACT_VERSION, AdminCorrelationId, AdminError, AdminErrorCode, AdminErrorMessage,
    AdminFailureResponse, AdminMutationRequest, AdminTransportLimits, UnixAdminSocketBinding,
};
use crate::{CancellationToken, EntropySource, SystemEntropy};

/// Smallest configured response cap that can always carry a safe v1 failure envelope.
pub const ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES: u32 = 512;

pub const ADMIN_ROUTE_PATH_MAX_UTF8_BYTES: usize = 256;
pub const ADMIN_ROUTE_PARAMETER_NAME_MAX_UTF8_BYTES: usize = 64;
pub const ADMIN_ROUTE_PARAMETER_VALUE_MAX_UTF8_BYTES: usize = 128;
const HTTP_MINIMUM_MAX_BUFFER_SIZE: usize = 8 * 1024;
const JSON_CONTENT_TYPE: &str = "application/json";
const FALLBACK_CORRELATION_ID: &str = "correlation-unavailable";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AdminHttpMethod {
    Get,
    Post,
}

impl AdminHttpMethod {
    fn from_http(method: &Method) -> Option<Self> {
        if method == Method::GET {
            Some(Self::Get)
        } else if method == Method::POST {
            Some(Self::Post)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminRoutePathError {
    Empty,
    TooLong,
    WrongVersionPrefix,
    InvalidCharacter,
    EmptySegment,
    InvalidParameter,
    DuplicateParameter,
}

impl fmt::Display for AdminRoutePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin route path is not a canonical v1 route")
    }
}

impl Error for AdminRoutePathError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminRoutePath {
    canonical: String,
    segments: Vec<AdminRouteSegment>,
    literal_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AdminRouteSegment {
    Literal(String),
    Parameter(String),
}

impl AdminRoutePath {
    pub fn new(value: impl AsRef<str>) -> Result<Self, AdminRoutePathError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(AdminRoutePathError::Empty);
        }
        if value.len() > ADMIN_ROUTE_PATH_MAX_UTF8_BYTES {
            return Err(AdminRoutePathError::TooLong);
        }
        if !value.starts_with("/v1/") || value.ends_with('/') {
            return Err(AdminRoutePathError::WrongVersionPrefix);
        }
        let mut parameter_names = BTreeSet::new();
        let mut literal_count = 0;
        for segment in value[1..].split('/') {
            if segment.is_empty() {
                return Err(AdminRoutePathError::EmptySegment);
            }
            if let Some(parameter) = segment
                .strip_prefix('{')
                .and_then(|segment| segment.strip_suffix('}'))
            {
                if !valid_parameter_name(parameter) {
                    return Err(AdminRoutePathError::InvalidParameter);
                }
                if !parameter_names.insert(parameter) {
                    return Err(AdminRoutePathError::DuplicateParameter);
                }
            } else {
                if !segment
                    .bytes()
                    .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
                {
                    return Err(AdminRoutePathError::InvalidCharacter);
                }
                literal_count += 1;
            }
        }
        let segments = value[1..]
            .split('/')
            .map(|segment| {
                segment
                    .strip_prefix('{')
                    .and_then(|segment| segment.strip_suffix('}'))
                    .map_or_else(
                        || AdminRouteSegment::Literal(segment.to_owned()),
                        |parameter| AdminRouteSegment::Parameter(parameter.to_owned()),
                    )
            })
            .collect();
        Ok(Self {
            canonical: value.to_owned(),
            segments,
            literal_count,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.segments.len() == other.segments.len()
            && self
                .segments
                .iter()
                .zip(&other.segments)
                .all(|(left, right)| match (left, right) {
                    (AdminRouteSegment::Literal(left), AdminRouteSegment::Literal(right)) => {
                        left == right
                    }
                    _ => true,
                })
    }

    fn match_path(&self, path: &str) -> Option<BTreeMap<String, String>> {
        if !path.starts_with('/') || path.ends_with('/') || path.contains("//") {
            return None;
        }
        let raw_segments = path[1..].split('/').collect::<Vec<_>>();
        if raw_segments.len() != self.segments.len() {
            return None;
        }
        let mut parameters = BTreeMap::new();
        for (pattern, raw) in self.segments.iter().zip(raw_segments) {
            match pattern {
                AdminRouteSegment::Literal(literal) if literal != raw => return None,
                AdminRouteSegment::Literal(_) => {}
                AdminRouteSegment::Parameter(name) => {
                    parameters.insert(name.clone(), decode_route_parameter(raw)?);
                }
            }
        }
        Some(parameters)
    }
}

fn valid_parameter_name(value: &str) -> bool {
    value.len() <= ADMIN_ROUTE_PARAMETER_NAME_MAX_UTF8_BYTES
        && matches!(value.as_bytes().first(), Some(b'a'..=b'z'))
        && value
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_'))
}

fn decode_route_parameter(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let raw = raw.as_bytes();
    let mut decoded = Vec::with_capacity(raw.len().min(ADMIN_ROUTE_PARAMETER_VALUE_MAX_UTF8_BYTES));
    let mut index = 0;
    while index < raw.len() {
        let byte = if raw[index] == b'%' {
            let high = *raw.get(index + 1)?;
            let low = *raw.get(index + 2)?;
            index += 3;
            (hex_nibble(high)? << 4) | hex_nibble(low)?
        } else {
            let byte = raw[index];
            index += 1;
            byte
        };
        if byte == b'/' || byte == b'\\' || byte == 0 || byte.is_ascii_control() {
            return None;
        }
        if decoded.len() == ADMIN_ROUTE_PARAMETER_VALUE_MAX_UTF8_BYTES {
            return None;
        }
        decoded.push(byte);
    }
    String::from_utf8(decoded)
        .ok()
        .filter(|value| !value.is_empty())
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminRouteRegistrationError {
    InvalidPath(AdminRoutePathError),
    Duplicate,
    Ambiguous,
}

impl fmt::Display for AdminRouteRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin route registration is invalid")
    }
}

impl Error for AdminRouteRegistrationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminRequestDecodeError {
    Empty,
    Malformed,
}

impl fmt::Display for AdminRequestDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin request body is not valid JSON")
    }
}

impl Error for AdminRequestDecodeError {}

/// Validated request metadata and a bounded raw JSON body.
pub struct AdminRequest {
    method: AdminHttpMethod,
    path: AdminRoutePath,
    query: Option<String>,
    correlation_id: AdminCorrelationId,
    parameters: BTreeMap<String, String>,
    body: Bytes,
    response_body_limit: usize,
}

impl AdminRequest {
    #[must_use]
    pub const fn method(&self) -> AdminHttpMethod {
        self.method
    }

    #[must_use]
    pub const fn path(&self) -> &AdminRoutePath {
        &self.path
    }

    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    #[must_use]
    pub const fn correlation_id(&self) -> &AdminCorrelationId {
        &self.correlation_id
    }

    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<&str> {
        self.parameters.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn decode_json<T>(&self) -> Result<T, AdminRequestDecodeError>
    where
        T: DeserializeOwned,
    {
        if self.body.is_empty() {
            return Err(AdminRequestDecodeError::Empty);
        }
        serde_json::from_slice(&self.body).map_err(|_| AdminRequestDecodeError::Malformed)
    }

    pub fn success<T>(&self, result: &T) -> Result<AdminRouteOutcome, AdminRouteOutcomeError>
    where
        T: Serialize,
    {
        let encoded =
            encode_bounded(result, self.response_body_limit).map_err(|error| match error {
                BoundedEncodingError::Limit => AdminRouteOutcomeError::ResponseLimit,
                BoundedEncodingError::Encoding => AdminRouteOutcomeError::Encoding,
            })?;
        let _: StrictJsonPayload =
            serde_json::from_slice(&encoded).map_err(|_| AdminRouteOutcomeError::InvalidPayload)?;
        let encoded =
            String::from_utf8(encoded).expect("serde_json output must always be valid UTF-8");
        let result =
            RawValue::from_string(encoded).map_err(|_| AdminRouteOutcomeError::Encoding)?;
        Ok(AdminRouteOutcome(AdminRouteOutcomeKind::Success(result)))
    }
}

impl fmt::Debug for AdminRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminRequest")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query", &self.query.as_ref().map(|_| "[redacted]"))
            .field("correlation_id", &self.correlation_id)
            .field(
                "parameter_names",
                &self.parameters.keys().collect::<Vec<_>>(),
            )
            .field("body", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminRouteOutcomeError {
    Encoding,
    InvalidPayload,
    ResponseLimit,
}

impl fmt::Display for AdminRouteOutcomeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Encoding => "admin route result could not be encoded",
            Self::InvalidPayload => "admin route result violates the JSON payload contract",
            Self::ResponseLimit => "admin route result exceeds the response limit",
        })
    }
}

impl Error for AdminRouteOutcomeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminRouteFailureStatus {
    BadRequest,
    NotFound,
    Conflict,
    Unavailable,
    Internal,
}

impl AdminRouteFailureStatus {
    const fn http_status(self) -> StatusCode {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminRouteFailure {
    status: AdminRouteFailureStatus,
    error: AdminError,
}

impl AdminRouteFailure {
    #[must_use]
    pub const fn new(status: AdminRouteFailureStatus, error: AdminError) -> Self {
        Self { status, error }
    }

    #[must_use]
    pub const fn status(&self) -> AdminRouteFailureStatus {
        self.status
    }

    #[must_use]
    pub const fn error(&self) -> &AdminError {
        &self.error
    }
}

enum AdminRouteOutcomeKind {
    Success(Box<RawValue>),
    Failure(AdminRouteFailure),
}

pub struct AdminRouteOutcome(AdminRouteOutcomeKind);

impl AdminRouteOutcome {
    #[must_use]
    pub const fn failure(failure: AdminRouteFailure) -> Self {
        Self(AdminRouteOutcomeKind::Failure(failure))
    }
}

impl fmt::Debug for AdminRouteOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            AdminRouteOutcomeKind::Success(_) => {
                formatter.write_str("AdminRouteOutcome::Success(<redacted>)")
            }
            AdminRouteOutcomeKind::Failure(failure) => {
                formatter.debug_tuple("Failure").field(failure).finish()
            }
        }
    }
}

#[derive(Serialize)]
struct ServerSuccessEnvelope<'a> {
    contract_version: u32,
    ok: bool,
    correlation_id: &'a AdminCorrelationId,
    result: &'a RawValue,
}

#[derive(Clone, Copy)]
struct StrictJsonPayload;

impl Serialize for StrictJsonPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for StrictJsonPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictJsonVisitor)
    }
}

struct StrictJsonVisitor;

impl<'de> de::Visitor<'de> for StrictJsonVisitor {
    type Value = StrictJsonPayload;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys or null values")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(StrictJsonPayload)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(StrictJsonPayload)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(StrictJsonPayload)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.is_finite() {
            Ok(StrictJsonPayload)
        } else {
            Err(E::custom("non-finite JSON number"))
        }
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(StrictJsonPayload)
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(StrictJsonPayload)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom("JSON null is forbidden"))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom("JSON null is forbidden"))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        while sequence.next_element::<StrictJsonPayload>()?.is_some() {}
        Ok(StrictJsonPayload)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            map.next_value::<StrictJsonPayload>()?;
        }
        Ok(StrictJsonPayload)
    }
}

type AdminRouteFuture = Pin<Box<dyn Future<Output = AdminRouteOutcome> + Send + 'static>>;

trait AdminRouteHandler: Send + Sync {
    fn handle(&self, request: AdminRequest) -> AdminRouteFuture;
}

struct FunctionRouteHandler<F>(F);

// This impl only boxes and forwards a service-owned handler future. End-to-end dispatch remains
// covered by the server tests; generic closure instantiations add no host policy branches.
#[cfg_attr(coverage_nightly, coverage(off))]
impl<F, Fut> AdminRouteHandler for FunctionRouteHandler<F>
where
    F: Fn(AdminRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = AdminRouteOutcome> + Send + 'static,
{
    fn handle(&self, request: AdminRequest) -> AdminRouteFuture {
        Box::pin((self.0)(request))
    }
}

struct RegisteredRoute {
    method: AdminHttpMethod,
    path: AdminRoutePath,
    handler: Arc<dyn AdminRouteHandler>,
}

/// Method/path registry supplied by the consuming service.
#[derive(Default)]
pub struct AdminRouter {
    routes: Vec<RegisteredRoute>,
}

impl AdminRouter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route<F, Fut>(
        &mut self,
        method: AdminHttpMethod,
        path: impl AsRef<str>,
        handler: F,
    ) -> Result<(), AdminRouteRegistrationError>
    where
        F: Fn(AdminRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AdminRouteOutcome> + Send + 'static,
    {
        let path = AdminRoutePath::new(path).map_err(AdminRouteRegistrationError::InvalidPath)?;
        if self
            .routes
            .iter()
            .any(|route| route.method == method && route.path == path)
        {
            return Err(AdminRouteRegistrationError::Duplicate);
        }
        if self.routes.iter().any(|route| {
            route.method == method
                && route.path.literal_count == path.literal_count
                && route.path.overlaps(&path)
        }) {
            return Err(AdminRouteRegistrationError::Ambiguous);
        }
        self.routes.push(RegisteredRoute {
            method,
            path,
            handler: Arc::new(FunctionRouteHandler(handler)),
        });
        Ok(())
    }
}

impl fmt::Debug for AdminRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminRouter")
            .field("route_count", &self.routes.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminServerConfigError {
    NoRoutes,
    ResponseLimitTooSmall,
}

impl fmt::Display for AdminServerConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin server configuration is invalid")
    }
}

impl Error for AdminServerConfigError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminServerError {
    ListenerClone { kind: io::ErrorKind },
    ListenerRegistration { kind: io::ErrorKind },
    Accept { kind: io::ErrorKind },
    ConnectionTaskPanicked,
}

impl fmt::Display for AdminServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded admin server failed")
    }
}

impl Error for AdminServerError {}

struct AdminServerState {
    routes: Vec<RegisteredRoute>,
    limits: AdminTransportLimits,
    entropy: Arc<dyn EntropySource>,
}

/// One bounded HTTP/1.1 server that can only consume a Unix admin binding.
pub struct AdminServer {
    state: Arc<AdminServerState>,
}

impl fmt::Debug for AdminServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminServer")
            .field("route_count", &self.state.routes.len())
            .field("limits", &self.state.limits)
            .field("entropy", &"[injected]")
            .finish()
    }
}

impl AdminServer {
    pub fn new<E>(
        router: AdminRouter,
        limits: AdminTransportLimits,
        entropy: E,
    ) -> Result<Self, AdminServerConfigError>
    where
        E: EntropySource + 'static,
    {
        if router.routes.is_empty() {
            return Err(AdminServerConfigError::NoRoutes);
        }
        if limits.response_body_utf8_bytes() < ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES {
            return Err(AdminServerConfigError::ResponseLimitTooSmall);
        }
        Ok(Self {
            state: Arc::new(AdminServerState {
                routes: router.routes,
                limits,
                entropy: Arc::new(entropy),
            }),
        })
    }

    pub fn with_system_entropy(
        router: AdminRouter,
        limits: AdminTransportLimits,
    ) -> Result<Self, AdminServerConfigError> {
        Self::new(router, limits, SystemEntropy)
    }

    /// Serves until cancellation, then stops admission and drains every bounded connection task.
    pub async fn serve(
        self,
        binding: UnixAdminSocketBinding,
        cancellation: CancellationToken,
    ) -> Result<(), AdminServerError> {
        let listener = binding
            .listener()
            .try_clone()
            .map_err(|error| AdminServerError::ListenerClone { kind: error.kind() })?;
        listener
            .set_nonblocking(true)
            .map_err(|error| AdminServerError::ListenerRegistration { kind: error.kind() })?;
        let listener = tokio::net::UnixListener::from_std(listener)
            .map_err(|error| AdminServerError::ListenerRegistration { kind: error.kind() })?;
        let peer_authorizer = binding.peer_authorizer();
        let permits = Arc::new(Semaphore::new(
            self.state.limits.concurrent_connections() as usize
        ));
        let mut tasks = JoinSet::new();

        let mut result = loop {
            tokio::select! {
                biased;
                () = cancellation.cancelled() => break Ok(()),
                joined = tasks.join_next(), if !tasks.is_empty() => {
                    if joined.is_some_and(|result| result.is_err()) {
                        cancellation.cancel();
                        break Err(AdminServerError::ConnectionTaskPanicked);
                    }
                }
                accepted = listener.accept() => {
                    let (stream, _) = match accepted {
                        Ok(accepted) => accepted,
                        Err(error) => {
                            cancellation.cancel();
                            break Err(AdminServerError::Accept { kind: error.kind() });
                        }
                    };
                    if peer_authorizer.authorize(&stream).is_err() {
                        drop(stream);
                        continue;
                    }
                    let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                        drop(stream);
                        continue;
                    };
                    let state = Arc::clone(&self.state);
                    let connection_cancellation = cancellation.clone();
                    tasks.spawn(async move {
                        let _permit = permit;
                        serve_connection(stream, state, connection_cancellation).await;
                    });
                }
            }
        };

        while let Some(joined) = tasks.join_next().await {
            if joined.is_err() {
                result = Err(AdminServerError::ConnectionTaskPanicked);
            }
        }
        drop(binding);
        result
    }
}

async fn serve_connection(
    stream: tokio::net::UnixStream,
    state: Arc<AdminServerState>,
    cancellation: CancellationToken,
) {
    let service_state = Arc::clone(&state);
    let service = service_fn(move |request| {
        let state = Arc::clone(&service_state);
        async move { Ok::<_, Infallible>(serve_request(request, state).await) }
    });

    let mut builder = http1::Builder::new();
    builder
        .keep_alive(false)
        .auto_date_header(false)
        .max_headers(state.limits.header_count() as usize)
        .max_buf_size((state.limits.header_bytes() as usize).max(HTTP_MINIMUM_MAX_BUFFER_SIZE))
        .header_read_timeout(state.limits.idle_timeout())
        .timer(TokioTimer::new());

    let mut connection = Box::pin(builder.serve_connection(TokioIo::new(stream), service));
    let connection_deadline = state
        .limits
        .request_deadline()
        .checked_add(state.limits.idle_timeout())
        .unwrap_or(state.limits.idle_timeout());
    tokio::select! {
        biased;
        () = cancellation.cancelled() => {
            connection.as_mut().graceful_shutdown();
            let _ = tokio::time::timeout(connection_deadline, connection).await;
        }
        _ = tokio::time::timeout(connection_deadline, &mut connection) => {}
    }
}

async fn serve_request(
    request: Request<Incoming>,
    state: Arc<AdminServerState>,
) -> Response<Full<Bytes>> {
    let (initial_correlation, deferred_entropy_failure) =
        match generated_correlation(&*state.entropy) {
            Ok(correlation) => (correlation, None),
            Err(error) => (fallback_correlation(), Some(error)),
        };
    let correlation = Arc::new(CorrelationSlot::new(initial_correlation));
    let deadline = state.limits.request_deadline();
    match tokio::time::timeout(
        deadline,
        process_request(
            request,
            Arc::clone(&state),
            Arc::clone(&correlation),
            deferred_entropy_failure,
        ),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => failure_response(
            StatusCode::GATEWAY_TIMEOUT,
            correlation.current(),
            known_error("request_timeout", "admin request deadline elapsed"),
            state.limits,
        ),
    }
}

async fn process_request(
    request: Request<Incoming>,
    state: Arc<AdminServerState>,
    correlation: Arc<CorrelationSlot>,
    deferred_entropy_failure: Option<AdminError>,
) -> Response<Full<Bytes>> {
    let correlation_id = correlation.current();
    if request.version() != Version::HTTP_11 {
        return failure_response(
            StatusCode::HTTP_VERSION_NOT_SUPPORTED,
            correlation_id,
            known_error(
                "http_version_unsupported",
                "admin transport requires HTTP/1.1",
            ),
            state.limits,
        );
    }
    if header_bytes(request.headers()) > u64::from(state.limits.header_bytes())
        || request.headers().len() > state.limits.header_count() as usize
    {
        return failure_response(
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            correlation_id,
            known_error(
                "request_headers_too_large",
                "admin request headers exceed the limit",
            ),
            state.limits,
        );
    }
    if query_item_count(request.uri().query()) > state.limits.query_items() as usize {
        return failure_response(
            StatusCode::BAD_REQUEST,
            correlation_id,
            known_error("query_limit_exceeded", "admin query exceeds the item limit"),
            state.limits,
        );
    }

    if unknown_major_version(request.uri().path()) {
        let response = AdminFailureResponse::unsupported_contract_version(correlation_id.clone());
        return bounded_json_response(
            StatusCode::BAD_REQUEST,
            &response,
            correlation_id,
            state.limits,
        );
    }
    let Some(method) = AdminHttpMethod::from_http(request.method()) else {
        return failure_response(
            StatusCode::NOT_FOUND,
            correlation_id,
            known_error("route_not_found", "admin route was not found"),
            state.limits,
        );
    };
    let route = state
        .routes
        .iter()
        .filter(|route| route.method == method)
        .filter_map(|route| {
            route
                .path
                .match_path(request.uri().path())
                .map(|parameters| {
                    (
                        route.path.literal_count,
                        route.path.clone(),
                        parameters,
                        Arc::clone(&route.handler),
                    )
                })
        })
        .max_by_key(|(literal_count, _, _, _)| *literal_count);
    let Some((_, path, parameters, handler)) = route else {
        return failure_response(
            StatusCode::NOT_FOUND,
            correlation_id,
            known_error("route_not_found", "admin route was not found"),
            state.limits,
        );
    };

    if method == AdminHttpMethod::Post && !is_json_content_type(request.headers()) {
        return failure_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            correlation_id,
            known_error(
                "content_type_required",
                "admin mutation requires JSON content type",
            ),
            state.limits,
        );
    }
    let query = request.uri().query().map(str::to_owned);
    let body = match Limited::new(
        request.into_body(),
        state.limits.request_body_utf8_bytes() as usize,
    )
    .collect()
    .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(error) if error.downcast_ref::<LengthLimitError>().is_some() => {
            return failure_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                correlation_id,
                known_error(
                    "request_body_too_large",
                    "admin request body exceeds the limit",
                ),
                state.limits,
            );
        }
        Err(_) => {
            return failure_response(
                StatusCode::BAD_REQUEST,
                correlation_id,
                known_error(
                    "request_body_unavailable",
                    "admin request body is unavailable",
                ),
                state.limits,
            );
        }
    };
    if method == AdminHttpMethod::Get && !body.is_empty() {
        return failure_response(
            StatusCode::BAD_REQUEST,
            correlation_id,
            known_error("malformed_json", "admin request body is not valid JSON"),
            state.limits,
        );
    }

    if method == AdminHttpMethod::Post {
        let envelope =
            match serde_json::from_slice::<AdminMutationRequest<StrictJsonPayload>>(&body) {
                Ok(envelope) => envelope,
                Err(_) => {
                    return failure_response(
                        StatusCode::BAD_REQUEST,
                        correlation.current(),
                        known_error("malformed_json", "admin request body is not valid JSON"),
                        state.limits,
                    );
                }
            };
        if let Some(caller_correlation) = envelope.correlation_id().cloned() {
            correlation.replace(caller_correlation);
        } else if let Some(error) = deferred_entropy_failure {
            return failure_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                correlation.current(),
                error,
                state.limits,
            );
        }
        if envelope.validate_contract_version().is_err() {
            let correlation_id = correlation.current();
            let response =
                AdminFailureResponse::unsupported_contract_version(correlation_id.clone());
            return bounded_json_response(
                StatusCode::BAD_REQUEST,
                &response,
                correlation_id,
                state.limits,
            );
        }
    } else if let Some(error) = deferred_entropy_failure {
        return failure_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            correlation.current(),
            error,
            state.limits,
        );
    }
    let correlation_id = correlation.current();

    let request = AdminRequest {
        method,
        path,
        query,
        correlation_id: correlation_id.clone(),
        parameters,
        body,
        response_body_limit: state.limits.response_body_utf8_bytes() as usize,
    };
    match handler.handle(request).await.0 {
        AdminRouteOutcomeKind::Success(result) => {
            let envelope = ServerSuccessEnvelope {
                contract_version: ADMIN_CONTRACT_VERSION,
                ok: true,
                correlation_id: &correlation_id,
                result: &result,
            };
            bounded_json_response(
                StatusCode::OK,
                &envelope,
                correlation_id.clone(),
                state.limits,
            )
        }
        AdminRouteOutcomeKind::Failure(failure) => failure_response(
            failure.status.http_status(),
            correlation_id,
            failure.error,
            state.limits,
        ),
    }
}

fn generated_correlation(entropy: &dyn EntropySource) -> Result<AdminCorrelationId, AdminError> {
    let mut bytes = [0_u8; 16];
    if entropy.fill_bytes(&mut bytes).is_err() {
        return Err(known_error(
            "correlation_unavailable",
            "admin correlation identity is unavailable",
        ));
    }
    let value = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(AdminCorrelationId::new(value).expect("hex correlation must be valid"))
}

fn fallback_correlation() -> AdminCorrelationId {
    AdminCorrelationId::new(FALLBACK_CORRELATION_ID)
        .expect("static correlation fallback must be valid")
}

fn unknown_major_version(path: &str) -> bool {
    path.strip_prefix('/')
        .and_then(|path| path.split('/').next())
        .is_some_and(|segment| {
            segment != "v1"
                && segment.strip_prefix('v').is_some_and(|version| {
                    !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
}

struct CorrelationSlot(Mutex<AdminCorrelationId>);

impl CorrelationSlot {
    fn new(correlation_id: AdminCorrelationId) -> Self {
        Self(Mutex::new(correlation_id))
    }

    fn current(&self) -> AdminCorrelationId {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn replace(&self, correlation_id: AdminCorrelationId) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = correlation_id;
    }
}

fn header_bytes(headers: &HeaderMap) -> u64 {
    headers.iter().fold(0_u64, |total, (name, value)| {
        total
            .saturating_add(name.as_str().len() as u64)
            .saturating_add(value.as_bytes().len() as u64)
    })
}

fn query_item_count(query: Option<&str>) -> usize {
    match query {
        None | Some("") => 0,
        Some(query) => query.split('&').count(),
    }
}

fn is_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value == JSON_CONTENT_TYPE || value == "application/json; charset=utf-8"
        })
}

fn failure_response(
    status: StatusCode,
    correlation_id: AdminCorrelationId,
    error: AdminError,
    limits: AdminTransportLimits,
) -> Response<Full<Bytes>> {
    let envelope = AdminFailureResponse::new(correlation_id.clone(), error);
    bounded_json_response(status, &envelope, correlation_id, limits)
}

fn bounded_json_response<T>(
    status: StatusCode,
    value: &T,
    correlation_id: AdminCorrelationId,
    limits: AdminTransportLimits,
) -> Response<Full<Bytes>>
where
    T: Serialize,
{
    let limit = limits.response_body_utf8_bytes() as usize;
    if let Ok(encoded) = encode_bounded(value, limit) {
        return json_response(status, encoded);
    }
    let fallback = AdminFailureResponse::new(
        correlation_id,
        known_error(
            "response_body_too_large",
            "admin response body exceeds the limit",
        ),
    );
    match encode_bounded(&fallback, limit) {
        Ok(encoded) => json_response(StatusCode::INTERNAL_SERVER_ERROR, encoded),
        Err(_) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            br#"{"contract_version":1,"ok":false,"correlation_id":"correlation-unavailable","error":{"code":"response_body_too_large","message":"admin response body exceeds the limit"}}"#.to_vec(),
        ),
    }
}

enum BoundedEncodingError {
    Limit,
    Encoding,
}

struct CappedWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl CappedWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(4096)),
            limit,
            exceeded: false,
        }
    }
}

impl io::Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("bounded JSON output limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode_bounded<T>(value: &T, limit: usize) -> Result<Vec<u8>, BoundedEncodingError>
where
    T: Serialize,
{
    let mut writer = CappedWriter::new(limit);
    match serde_json::to_writer(&mut writer, value) {
        Ok(()) => Ok(writer.bytes),
        Err(_) if writer.exceeded => Err(BoundedEncodingError::Limit),
        Err(_) => Err(BoundedEncodingError::Encoding),
    }
}

fn json_response(status: StatusCode, body: Vec<u8>) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(JSON_CONTENT_TYPE));
    response
}

fn known_error(code: &'static str, message: &'static str) -> AdminError {
    AdminError::new(
        AdminErrorCode::new(code).expect("static admin error code must be valid"),
        AdminErrorMessage::new(message).expect("static admin error message must be valid"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Notify;

    #[derive(Clone, Copy)]
    struct FixedEntropy(u8);

    impl EntropySource for FixedEntropy {
        fn fill_bytes(&self, destination: &mut [u8]) -> Result<(), crate::EntropyError> {
            destination.fill(self.0);
            Ok(())
        }
    }

    struct FailingEntropy;

    impl EntropySource for FailingEntropy {
        fn fill_bytes(&self, _destination: &mut [u8]) -> Result<(), crate::EntropyError> {
            Err(crate::EntropyError::Unavailable)
        }
    }

    struct CountingFailingEntropy(Arc<AtomicUsize>);

    impl EntropySource for CountingFailingEntropy {
        fn fill_bytes(&self, _destination: &mut [u8]) -> Result<(), crate::EntropyError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(crate::EntropyError::Unavailable)
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct EchoRequest {
        value: String,
    }

    fn echo_router() -> AdminRouter {
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Post, "/v1/echo", |request| async move {
                match request.decode_json::<AdminMutationRequest<EchoRequest>>() {
                    Ok(envelope) => {
                        let request_body = envelope.into_request();
                        request
                            .success(&serde_json::json!({"value": request_body.value}))
                            .expect("echo result should encode")
                    }
                    Err(_) => AdminRouteOutcome::failure(AdminRouteFailure::new(
                        AdminRouteFailureStatus::BadRequest,
                        known_error("malformed_echo", "echo request is malformed"),
                    )),
                }
            })
            .expect("echo route");
        router
    }

    fn limits_with(
        request_body: u32,
        concurrency: u32,
        deadline: std::time::Duration,
    ) -> AdminTransportLimits {
        let mut values = AdminTransportLimits::DEFAULT.values();
        values.request_body_utf8_bytes = request_body;
        values.concurrent_connections = concurrency;
        values.request_deadline = deadline;
        AdminTransportLimits::new(values).expect("test limits")
    }

    async fn binding(
        directory: &tempfile::TempDir,
    ) -> (std::path::PathBuf, UnixAdminSocketBinding) {
        let socket = directory.path().join("admin.sock");
        let authority = super::super::UnixAdminSocketWriterAuthority::acquire(directory.path())
            .expect("writer authority");
        let binding = UnixAdminSocketBinding::bind(authority, &socket)
            .await
            .expect("socket binding");
        (socket, binding)
    }

    async fn exchange(socket: &std::path::Path, request: &[u8]) -> Vec<u8> {
        let mut stream = tokio::net::UnixStream::connect(socket)
            .await
            .expect("connect admin server");
        stream.write_all(request).await.expect("write request");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("read response");
        response
    }

    async fn exchange_allowing_reset(socket: &std::path::Path, request: &[u8]) -> Vec<u8> {
        let mut stream = tokio::net::UnixStream::connect(socket)
            .await
            .expect("connect admin server");
        stream.write_all(request).await.expect("write request");
        let mut response = Vec::new();
        match stream.read_to_end(&mut response).await {
            Ok(_) => response,
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => response,
            Err(error) => panic!("read response: {error}"),
        }
    }

    #[tokio::test]
    async fn serves_valid_json_with_exact_caller_correlation_and_no_web_headers() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let server = AdminServer::new(
            echo_router(),
            AdminTransportLimits::DEFAULT,
            FixedEntropy(7),
        )
        .expect("admin server");
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(server.serve(binding, task_cancellation));

        let body = r#"{"contract_version":1,"operation_id":"echo-01","correlation_id":"caller-01","request":{"value":"ok"}}"#;
        let request = format!(
            "POST /v1/echo HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let response = exchange(&socket, request.as_bytes()).await;
        let text = String::from_utf8(response).expect("UTF-8 response");
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"), "{text}");
        assert!(text.contains("content-type: application/json"));
        assert!(!text.to_ascii_lowercase().contains("access-control-allow"));
        assert!(text.ends_with(
            r#"{"contract_version":1,"ok":true,"correlation_id":"caller-01","result":{"value":"ok"}}"#
        ));

        let unsupported =
            exchange(&socket, b"GET /v2/status HTTP/1.1\r\nHost: local\r\n\r\n").await;
        let unsupported = String::from_utf8(unsupported).expect("unsupported response");
        assert!(unsupported.starts_with("HTTP/1.1 400 Bad Request\r\n"));
        assert!(unsupported.contains("unsupported_contract_version"));
        assert!(unsupported.contains("07070707070707070707070707070707"));

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn rejects_oversized_and_malformed_json_before_the_handler() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Post, "/v1/test", move |request| {
                let calls = Arc::clone(&handler_calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    request
                        .success(&serde_json::json!({"ok": true}))
                        .expect("test result")
                }
            })
            .expect("test route");
        let server = AdminServer::new(
            router,
            limits_with(8, 4, std::time::Duration::from_secs(1)),
            FixedEntropy(8),
        )
        .expect("admin server");
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(binding, cancellation.clone()));

        let oversized = exchange(
            &socket,
            b"POST /v1/test HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: 9\r\n\r\n123456789",
        )
        .await;
        assert!(
            String::from_utf8(oversized)
                .expect("response")
                .contains("request_body_too_large")
        );
        let malformed = exchange(
            &socket,
            b"POST /v1/test HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: 1\r\n\r\n{",
        )
        .await;
        assert!(
            String::from_utf8(malformed)
                .expect("response")
                .contains("malformed_json")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
    }

    #[tokio::test]
    async fn rejects_invalid_mutation_envelopes_duplicates_and_nested_null_before_dispatch() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Post, "/v1/test", move |request| {
                let calls = Arc::clone(&handler_calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    request.success(&true).expect("test result")
                }
            })
            .expect("test route");
        let server = AdminServer::new(router, AdminTransportLimits::DEFAULT, FixedEntropy(0x21))
            .expect("admin server");
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(binding, cancellation.clone()));

        for body in [
            r#"{"operation_id":"missing-version","request":{"value":true}}"#,
            r#"{"contract_version":1,"operation_id":"missing-request"}"#,
            r#"{"contract_version":1,"operation_id":"duplicate-correlation","correlation_id":"first","correlation_id":"second","request":{"value":true}}"#,
            r#"{"contract_version":1,"operation_id":"nested-duplicate","request":{"value":true,"value":false}}"#,
            r#"{"contract_version":1,"operation_id":"nested-null","request":{"nested":[true,null]}}"#,
        ] {
            let request = format!(
                "POST /v1/test HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let response = String::from_utf8(exchange(&socket, request.as_bytes()).await)
                .expect("invalid envelope response");
            assert!(response.contains("malformed_json"), "{response}");
        }

        let body = r#"{"contract_version":2,"operation_id":"future-version","correlation_id":"caller-v2","request":{"value":true}}"#;
        let request = format!(
            "POST /v1/test HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let response = String::from_utf8(exchange(&socket, request.as_bytes()).await)
            .expect("version response");
        assert!(response.contains("unsupported_contract_version"));
        assert!(response.contains("caller-v2"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
    }

    #[tokio::test]
    async fn parameterized_routes_percent_decode_bounded_values_without_service_authority() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let mut router = AdminRouter::new();
        router
            .route(
                AdminHttpMethod::Post,
                "/v1/connections/{connection_id}/approve",
                move |request| {
                    let calls = Arc::clone(&handler_calls);
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        let parameter = request
                            .parameter("connection_id")
                            .expect("decoded route parameter")
                            .to_owned();
                        request
                            .success(&serde_json::json!({"connection_id": parameter}))
                            .expect("parameter response")
                    }
                },
            )
            .expect("parameterized route");
        let server = AdminServer::new(router, AdminTransportLimits::DEFAULT, FixedEntropy(0x22))
            .expect("admin server");
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(binding, cancellation.clone()));
        let body =
            r#"{"contract_version":1,"operation_id":"approve-01","request":{"approve":true}}"#;

        let request = format!(
            "POST /v1/connections/farm%2D01/approve HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let response = String::from_utf8(exchange(&socket, request.as_bytes()).await)
            .expect("parameter response");
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains(r#""connection_id":"farm-01""#));

        for path in [
            "/v1/connections/farm%2/approve",
            "/v1/connections/farm%2F01/approve",
            &format!(
                "/v1/connections/{}/approve",
                "x".repeat(ADMIN_ROUTE_PARAMETER_VALUE_MAX_UTF8_BYTES + 1)
            ),
        ] {
            let request = format!(
                "POST {path} HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let response = String::from_utf8(exchange(&socket, request.as_bytes()).await)
                .expect("invalid parameter response");
            assert!(response.contains("route_not_found"), "{response}");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
    }

    #[tokio::test]
    async fn caller_correlation_precedes_entropy_and_survives_timeout_handoff() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let entropy_calls = Arc::new(AtomicUsize::new(0));
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Post, "/v1/slow", |request| async move {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                request.success(&true).expect("slow result")
            })
            .expect("slow route");
        let server = AdminServer::new(
            router,
            limits_with(1024, 4, std::time::Duration::from_millis(20)),
            CountingFailingEntropy(Arc::clone(&entropy_calls)),
        )
        .expect("admin server");
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(binding, cancellation.clone()));

        let body = r#"{"contract_version":1,"operation_id":"slow-01","correlation_id":"caller-timeout","request":{"wait":true}}"#;
        let request = format!(
            "POST /v1/slow HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let response = String::from_utf8(exchange(&socket, request.as_bytes()).await)
            .expect("timeout response");
        assert!(response.starts_with("HTTP/1.1 504 Gateway Timeout"));
        assert!(response.contains("caller-timeout"));
        assert_eq!(entropy_calls.load(Ordering::SeqCst), 1);

        let body = r#"{"contract_version":1,"operation_id":"slow-02","request":{"wait":true}}"#;
        let request = format!(
            "POST /v1/slow HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let response = String::from_utf8(exchange(&socket, request.as_bytes()).await)
            .expect("entropy response");
        assert!(response.contains("correlation_unavailable"));
        assert_eq!(entropy_calls.load(Ordering::SeqCst), 2);

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
    }

    #[tokio::test]
    async fn rejects_http_1_0_before_dispatch() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Get, "/v1/status", move |request| {
                let calls = Arc::clone(&handler_calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    request.success(&true).expect("status result")
                }
            })
            .expect("status route");
        let server = AdminServer::new(router, AdminTransportLimits::DEFAULT, FixedEntropy(0x23))
            .expect("admin server");
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(binding, cancellation.clone()));

        let response = String::from_utf8(
            exchange(&socket, b"GET /v1/status HTTP/1.0\r\nHost: local\r\n\r\n").await,
        )
        .expect("HTTP/1.0 response");
        assert!(response.contains("505 HTTP Version Not Supported"));
        assert!(response.contains("http_version_unsupported"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
    }

    #[tokio::test]
    async fn request_deadline_returns_a_safe_timeout_and_cancels_the_handler_future() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Get, "/v1/slow", |request| async move {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                request
                    .success(&serde_json::json!({"late": true}))
                    .expect("late result")
            })
            .expect("slow route");
        let server = AdminServer::new(
            router,
            limits_with(1024, 4, std::time::Duration::from_millis(20)),
            FixedEntropy(9),
        )
        .expect("admin server");
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(binding, cancellation.clone()));

        let response = exchange(&socket, b"GET /v1/slow HTTP/1.1\r\nHost: local\r\n\r\n").await;
        let text = String::from_utf8(response).expect("response");
        assert!(text.starts_with("HTTP/1.1 504 Gateway Timeout\r\n"));
        assert!(text.contains("request_timeout"));

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
    }

    #[tokio::test]
    async fn enforces_header_query_response_and_body_correlation_boundaries() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Get, "/v1/large", move |request| {
                let calls = Arc::clone(&handler_calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    match request.success(&"x".repeat(1024)) {
                        Ok(outcome) => outcome,
                        Err(AdminRouteOutcomeError::ResponseLimit) => {
                            AdminRouteOutcome::failure(AdminRouteFailure::new(
                                AdminRouteFailureStatus::Internal,
                                known_error(
                                    "response_body_too_large",
                                    "admin response body exceeds the limit",
                                ),
                            ))
                        }
                        Err(_) => panic!("large result should only exceed the cap"),
                    }
                }
            })
            .expect("large route");
        router
            .route(AdminHttpMethod::Post, "/v1/input", |request| async move {
                request.success(&true).expect("input result")
            })
            .expect("input route");
        let mut values = AdminTransportLimits::DEFAULT.values();
        values.header_bytes = 64;
        values.query_items = 1;
        values.response_body_utf8_bytes = ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES;
        let limits = AdminTransportLimits::new(values).expect("boundary limits");
        let server = AdminServer::new(router, limits, FixedEntropy(11)).expect("admin server");
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(binding, cancellation.clone()));

        let headers = exchange(
            &socket,
            b"GET /v1/large HTTP/1.1\r\nHost: local\r\nX-Large: 0123456789012345678901234567890123456789012345678901234567890123456789\r\n\r\n",
        )
        .await;
        assert!(
            String::from_utf8(headers)
                .expect("header response")
                .contains("request_headers_too_large")
        );

        let query = exchange(
            &socket,
            b"GET /v1/large?a=1&b=2 HTTP/1.1\r\nHost: local\r\n\r\n",
        )
        .await;
        assert!(
            String::from_utf8(query)
                .expect("query response")
                .contains("query_limit_exceeded")
        );

        let response = exchange(&socket, b"GET /v1/large HTTP/1.1\r\nHost: local\r\n\r\n").await;
        let response = String::from_utf8(response).expect("bounded response");
        assert!(response.contains("response_body_too_large"));
        assert!(response.len() < ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES as usize + 256);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let body = r#"{"contract_version":1,"operation_id":"input-01","correlation_id":null,"request":{"value":true}}"#;
        let invalid_correlation = format!(
            "POST /v1/input HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let invalid_correlation = exchange(&socket, invalid_correlation.as_bytes()).await;
        assert!(
            String::from_utf8(invalid_correlation)
                .expect("correlation response")
                .contains("malformed_json")
        );

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
    }

    #[tokio::test]
    async fn connection_admission_never_exceeds_the_configured_limit() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let handler_entered = Arc::clone(&entered);
        let handler_release = Arc::clone(&release);
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Get, "/v1/block", move |request| {
                let entered = Arc::clone(&handler_entered);
                let release = Arc::clone(&handler_release);
                async move {
                    entered.notify_one();
                    release.notified().await;
                    request
                        .success(&serde_json::json!({"done": true}))
                        .expect("blocking result")
                }
            })
            .expect("blocking route");
        let server = AdminServer::new(
            router,
            limits_with(1024, 1, std::time::Duration::from_secs(1)),
            FixedEntropy(10),
        )
        .expect("admin server");
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(binding, cancellation.clone()));

        let first_socket = socket.clone();
        let first = tokio::spawn(async move {
            exchange(
                &first_socket,
                b"GET /v1/block HTTP/1.1\r\nHost: local\r\n\r\n",
            )
            .await
        });
        entered.notified().await;
        let second = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            exchange_allowing_reset(&socket, b"GET /v1/block HTTP/1.1\r\nHost: local\r\n\r\n"),
        )
        .await
        .expect("second connection must close without waiting");
        assert!(
            second.is_empty(),
            "excess connection must receive no response"
        );
        release.notify_one();
        assert!(
            String::from_utf8(first.await.expect("first task"))
                .expect("first response")
                .starts_with("HTTP/1.1 200 OK")
        );

        cancellation.cancel();
        task.await.expect("server task").expect("server shutdown");
    }

    #[tokio::test]
    async fn graceful_cancellation_stops_admission_and_drains_an_active_request() {
        let directory = super::test_support::short_tempdir();
        let (socket, binding) = binding(&directory).await;
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let handler_entered = Arc::clone(&entered);
        let handler_release = Arc::clone(&release);
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Get, "/v1/drain", move |request| {
                let entered = Arc::clone(&handler_entered);
                let release = Arc::clone(&handler_release);
                async move {
                    entered.notify_one();
                    release.notified().await;
                    request.success(&true).expect("drained result")
                }
            })
            .expect("drain route");
        let server = AdminServer::new(
            router,
            limits_with(1024, 1, std::time::Duration::from_secs(1)),
            FixedEntropy(12),
        )
        .expect("admin server");
        let cancellation = CancellationToken::new();
        let mut task = tokio::spawn(server.serve(binding, cancellation.clone()));
        let client_socket = socket.clone();
        let client = tokio::spawn(async move {
            exchange(
                &client_socket,
                b"GET /v1/drain HTTP/1.1\r\nHost: local\r\n\r\n",
            )
            .await
        });

        entered.notified().await;
        cancellation.cancel();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut task)
                .await
                .is_err(),
            "graceful cancellation must retain active request work"
        );
        release.notify_one();
        assert!(
            String::from_utf8(client.await.expect("client task"))
                .expect("drained response")
                .starts_with("HTTP/1.1 200 OK")
        );
        task.await.expect("server task").expect("server shutdown");
        assert!(!socket.exists());
    }

    #[test]
    fn route_and_server_configuration_fail_closed() {
        assert!(matches!(
            AdminServer::new(
                AdminRouter::new(),
                AdminTransportLimits::DEFAULT,
                FixedEntropy(1),
            ),
            Err(AdminServerConfigError::NoRoutes)
        ));
        let mut router = AdminRouter::new();
        assert!(matches!(
            router.route(AdminHttpMethod::Get, "/v2/status", |request| async move {
                request.success(&true).expect("bool")
            }),
            Err(AdminRouteRegistrationError::InvalidPath(
                AdminRoutePathError::WrongVersionPrefix
            ))
        ));
        router
            .route(AdminHttpMethod::Get, "/v1/status", |request| async move {
                request.success(&true).expect("bool")
            })
            .expect("first route");
        assert_eq!(
            router
                .route(AdminHttpMethod::Get, "/v1/status", |request| async move {
                    request.success(&true).expect("bool")
                })
                .expect_err("duplicate route"),
            AdminRouteRegistrationError::Duplicate
        );

        let mut values = AdminTransportLimits::DEFAULT.values();
        values.response_body_utf8_bytes = ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES - 1;
        let limits = AdminTransportLimits::new(values).expect("small response limit");
        assert_eq!(
            AdminServer::new(router, limits, FixedEntropy(1)).expect_err("small response cap"),
            AdminServerConfigError::ResponseLimitTooSmall
        );
        assert_eq!(ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES, 512);
        let longest_correlation = AdminCorrelationId::new("x".repeat(128)).expect("correlation");
        let fallback = AdminFailureResponse::new(
            longest_correlation,
            known_error(
                "response_body_too_large",
                "admin response body exceeds the limit",
            ),
        );
        assert!(encode_bounded(&fallback, ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES as usize).is_ok());

        let mut parameterized = AdminRouter::new();
        parameterized
            .route(
                AdminHttpMethod::Get,
                "/v1/items/{item_id}",
                |request| async move { request.success(&true).expect("parameterized result") },
            )
            .expect("parameterized route");
        parameterized
            .route(
                AdminHttpMethod::Get,
                "/v1/items/status",
                |request| async move { request.success(&true).expect("static result") },
            )
            .expect("more-specific static route");
        assert_eq!(
            parameterized
                .route(
                    AdminHttpMethod::Get,
                    "/v1/items/{other_id}",
                    |request| async move { request.success(&true).expect("ambiguous result") }
                )
                .expect_err("ambiguous route"),
            AdminRouteRegistrationError::Ambiguous
        );
        assert!(matches!(
            AdminRoutePath::new("/v1/items/{Bad}"),
            Err(AdminRoutePathError::InvalidParameter)
        ));
    }

    #[test]
    fn capped_encoding_and_result_validation_fail_before_unbounded_response_allocation() {
        let oversized = "x".repeat(ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES as usize + 1);
        assert!(matches!(
            encode_bounded(&oversized, ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES as usize),
            Err(BoundedEncodingError::Limit)
        ));

        let mut writer = CappedWriter::new(8);
        assert!(io::Write::write_all(&mut writer, b"123456789").is_err());
        assert!(writer.exceeded);
        assert!(writer.bytes.len() <= 8);

        let request = AdminRequest {
            method: AdminHttpMethod::Get,
            path: AdminRoutePath::new("/v1/test").expect("path"),
            query: None,
            correlation_id: AdminCorrelationId::new("safe-id").expect("correlation"),
            parameters: BTreeMap::new(),
            body: Bytes::new(),
            response_body_limit: ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES as usize,
        };
        assert!(matches!(
            request.success(&Option::<bool>::None),
            Err(AdminRouteOutcomeError::InvalidPayload)
        ));

        let malformed = AdminRequest {
            body: Bytes::from_static(b"secret malformed JSON"),
            ..request
        }
        .decode_json::<bool>()
        .expect_err("malformed request must fail");
        assert_eq!(malformed, AdminRequestDecodeError::Malformed);
        assert!(std::error::Error::source(&malformed).is_none());
        assert!(!format!("{malformed:?}").contains("secret"));

        let request = AdminRequest {
            method: AdminHttpMethod::Get,
            path: AdminRoutePath::new("/v1/test").expect("path"),
            query: None,
            correlation_id: AdminCorrelationId::new("safe-id").expect("correlation"),
            parameters: BTreeMap::new(),
            body: Bytes::new(),
            response_body_limit: ADMIN_MIN_RESPONSE_BODY_UTF8_BYTES as usize,
        };
        let unsupported_json_map_key = BTreeMap::from([((1_u8, 2_u8), true)]);
        let encoding = request
            .success(&unsupported_json_map_key)
            .expect_err("tuple map key must not encode as JSON");
        assert_eq!(encoding, AdminRouteOutcomeError::Encoding);
        assert!(std::error::Error::source(&encoding).is_none());
    }

    #[test]
    fn generated_correlation_is_exact_and_entropy_failure_is_safe() {
        let correlation = generated_correlation(&FixedEntropy(0xab)).expect("fixed entropy");
        assert_eq!(correlation.as_str(), "abababababababababababababababab");

        let failure = generated_correlation(&FailingEntropy).expect_err("entropy failure");
        assert_eq!(failure.code().as_str(), "correlation_unavailable");
        assert_eq!(fallback_correlation().as_str(), FALLBACK_CORRELATION_ID);
    }

    #[test]
    fn package_source_has_no_forbidden_listener_or_web_authority() {
        let source = include_str!("server.rs");
        for forbidden in [
            concat!("Tcp", "Listener"),
            concat!("Access-Control", "-Allow"),
            concat!("co", "rs"),
            concat!("browser", "_auth"),
        ] {
            assert!(
                !source.contains(forbidden),
                "forbidden server authority: {forbidden}"
            );
        }
    }

    #[test]
    fn request_and_outcome_debug_redact_body_query_and_result() {
        let request = AdminRequest {
            method: AdminHttpMethod::Post,
            path: AdminRoutePath::new("/v1/test").expect("path"),
            query: Some("secret=query".to_owned()),
            correlation_id: AdminCorrelationId::new("safe-id").expect("correlation"),
            parameters: BTreeMap::from([("item_id".to_owned(), "secret-item".to_owned())]),
            body: Bytes::from_static(b"{\"secret\":true}"),
            response_body_limit: 1024,
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("secret=query"));
        assert!(!debug.contains("secret\":true"));
        assert!(!debug.contains("secret-item"));
        let outcome = request
            .success(&serde_json::json!({"secret": true}))
            .expect("outcome");
        assert!(!format!("{outcome:?}").contains("secret"));
    }

    #[test]
    fn strict_json_routes_accessors_and_safe_errors_cover_the_full_value_surface() {
        assert!(valid_parameter_name("item_1"));
        for rejected in [
            "",
            "1item",
            "Item",
            "item-name",
            &"x".repeat(ADMIN_ROUTE_PARAMETER_NAME_MAX_UTF8_BYTES + 1),
        ] {
            assert!(!valid_parameter_name(rejected));
        }
        assert_eq!(decode_route_parameter(""), None);
        assert_eq!(decode_route_parameter("plain"), Some("plain".to_owned()));

        for path in ["/", "/v", "/v1", "/version2", "/v2beta"] {
            assert!(!unknown_major_version(path), "{path}");
        }
        for path in ["/v0", "/v2", "/v99/status"] {
            assert!(unknown_major_version(path), "{path}");
        }
        assert_eq!(query_item_count(None), 0);
        assert_eq!(query_item_count(Some("")), 0);
        assert_eq!(query_item_count(Some("one")), 1);
        assert_eq!(query_item_count(Some("one&two")), 2);

        let mut headers = HeaderMap::new();
        assert!(!is_json_content_type(&headers));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static(JSON_CONTENT_TYPE));
        assert!(is_json_content_type(&headers));
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        assert!(is_json_content_type(&headers));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        assert!(!is_json_content_type(&headers));

        for document in [
            "true",
            "-7",
            "9",
            "1.5",
            r#""text""#,
            "[true,2]",
            r#"{"value":3}"#,
        ] {
            serde_json::from_str::<StrictJsonPayload>(document).unwrap();
        }
        for rejected in ["null", "[1,null]", r#"{"same":1,"same":2}"#] {
            assert!(serde_json::from_str::<StrictJsonPayload>(rejected).is_err());
        }

        let route = AdminRoutePath::new("/v1/items/{item_id}").unwrap();
        assert_eq!(route.as_str(), "/v1/items/{item_id}");
        assert!(route.overlaps(&AdminRoutePath::new("/v1/items/static").unwrap()));
        assert!(!route.overlaps(&AdminRoutePath::new("/v1/other/static").unwrap()));
        assert!(!route.overlaps(&AdminRoutePath::new("/v1/items/static/more").unwrap()));
        assert_eq!(
            route.match_path("/v1/items/value%2D1").unwrap(),
            BTreeMap::from([("item_id".to_owned(), "value-1".to_owned())])
        );
        for rejected in [
            "v1/items/value",
            "/v1/items/value/",
            "/v1//items/value",
            "/v1/items",
            "/v1/other/value",
            "/v1/items/%",
            "/v1/items/%GG",
            "/v1/items/%2F",
            "/v1/items/%5c",
            "/v1/items/%00",
            "/v1/items/%0A",
            "/v1/items/%FF",
        ] {
            assert!(route.match_path(rejected).is_none(), "{rejected}");
        }
        assert!(
            route
                .match_path(&format!(
                    "/v1/items/{}",
                    "x".repeat(ADMIN_ROUTE_PARAMETER_VALUE_MAX_UTF8_BYTES + 1)
                ))
                .is_none()
        );

        let invalid_paths = [
            ("", AdminRoutePathError::Empty),
            ("/v2/items", AdminRoutePathError::WrongVersionPrefix),
            ("/v1/items/", AdminRoutePathError::WrongVersionPrefix),
            ("/v1//items", AdminRoutePathError::EmptySegment),
            ("/v1/Items", AdminRoutePathError::InvalidCharacter),
            ("/v1/{Bad}", AdminRoutePathError::InvalidParameter),
            ("/v1/{item}/{item}", AdminRoutePathError::DuplicateParameter),
        ];
        for (path, expected) in invalid_paths {
            assert_eq!(AdminRoutePath::new(path).unwrap_err(), expected);
        }
        let exact_maximum = format!(
            "/v1/{}",
            "x".repeat(ADMIN_ROUTE_PATH_MAX_UTF8_BYTES - "/v1/".len())
        );
        assert_eq!(
            AdminRoutePath::new(&exact_maximum)
                .expect("exact maximum route")
                .as_str(),
            exact_maximum
        );
        assert_eq!(
            AdminRoutePath::new(format!(
                "/v1/{}",
                "x".repeat(ADMIN_ROUTE_PATH_MAX_UTF8_BYTES)
            ))
            .unwrap_err(),
            AdminRoutePathError::TooLong
        );
        assert_eq!(
            AdminRoutePath::new(format!("/v1/{}", "x".repeat(4 * 1024 * 1024))),
            Err(AdminRoutePathError::TooLong)
        );

        assert_eq!(
            AdminHttpMethod::from_http(&Method::GET),
            Some(AdminHttpMethod::Get)
        );
        assert_eq!(
            AdminHttpMethod::from_http(&Method::POST),
            Some(AdminHttpMethod::Post)
        );
        assert_eq!(AdminHttpMethod::from_http(&Method::DELETE), None);

        let request = AdminRequest {
            method: AdminHttpMethod::Post,
            path: route,
            query: Some("page=1".to_owned()),
            correlation_id: AdminCorrelationId::new("correlation-1").unwrap(),
            parameters: BTreeMap::from([("item_id".to_owned(), "item-1".to_owned())]),
            body: Bytes::from_static(b"true"),
            response_body_limit: 1024,
        };
        assert_eq!(request.method(), AdminHttpMethod::Post);
        assert_eq!(request.path().as_str(), "/v1/items/{item_id}");
        assert_eq!(request.query(), Some("page=1"));
        assert_eq!(request.correlation_id().as_str(), "correlation-1");
        assert_eq!(request.parameter("item_id"), Some("item-1"));
        assert_eq!(request.parameter("missing"), None);
        assert_eq!(request.body(), b"true");
        assert!(request.decode_json::<bool>().unwrap());
        let empty = AdminRequest {
            body: Bytes::new(),
            ..request
        };
        assert_eq!(
            empty.decode_json::<bool>().unwrap_err(),
            AdminRequestDecodeError::Empty
        );

        let error = known_error("stable_error", "stable message");
        for status in [
            AdminRouteFailureStatus::BadRequest,
            AdminRouteFailureStatus::NotFound,
            AdminRouteFailureStatus::Conflict,
            AdminRouteFailureStatus::Unavailable,
            AdminRouteFailureStatus::Internal,
        ] {
            assert!(
                status.http_status().is_client_error() || status.http_status().is_server_error()
            );
            let failure = AdminRouteFailure::new(status, error.clone());
            assert_eq!(failure.status(), status);
            assert_eq!(failure.error(), &error);
            assert!(format!("{:?}", AdminRouteOutcome::failure(failure)).contains("Failure"));
        }
        for rendered in [
            AdminRoutePathError::Empty.to_string(),
            AdminRouteRegistrationError::Duplicate.to_string(),
            AdminRequestDecodeError::Malformed.to_string(),
            AdminRouteOutcomeError::Encoding.to_string(),
            AdminRouteOutcomeError::InvalidPayload.to_string(),
            AdminRouteOutcomeError::ResponseLimit.to_string(),
        ] {
            assert!(!rendered.is_empty());
        }

        use std::io::Write as _;
        let mut writer = CappedWriter::new(4);
        writer.flush().unwrap();
        assert_eq!(writer.write(b"four").unwrap(), 4);
        assert!(writer.write(b"x").is_err());
    }
}
