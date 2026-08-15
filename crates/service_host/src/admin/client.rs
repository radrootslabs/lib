//! Bounded HTTP/1.1 administration client over Unix sockets.

use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, de, de::DeserializeOwned};
use std::collections::BTreeSet;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderMap, HeaderValue};
use http::{Method, Request, StatusCode, Uri, Version};
use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::task::JoinHandle;

use super::{
    ADMIN_CONTRACT_VERSION, ADMIN_ROUTE_PATH_MAX_UTF8_BYTES, AdminCorrelationId,
    AdminFailureResponse, AdminHttpMethod, AdminOperationId, AdminSuccessResponse,
    AdminTransportLimits,
};

const JSON_CONTENT_TYPE: &str = "application/json";
const ADMIN_CLIENT_TARGET_MAX_UTF8_BYTES: usize = 32 * 1024;
const HTTP_MINIMUM_MAX_BUFFER_SIZE: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminClientTargetError {
    Empty,
    TooLong,
    InvalidUri,
    AuthorityForbidden,
    WrongVersionPrefix,
    PathTooLong,
    EmptySegment,
    PatternForbidden,
    InvalidPercentEncoding,
}

impl fmt::Display for AdminClientTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin client target is not a bounded canonical v1 target")
    }
}

impl Error for AdminClientTargetError {}

/// Validated relative v1 request target. Query contents are redacted from `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct AdminClientTarget {
    uri: Uri,
}

impl AdminClientTarget {
    pub fn new(value: impl AsRef<str>) -> Result<Self, AdminClientTargetError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(AdminClientTargetError::Empty);
        }
        if value.len() > ADMIN_CLIENT_TARGET_MAX_UTF8_BYTES {
            return Err(AdminClientTargetError::TooLong);
        }
        let uri = value
            .parse::<Uri>()
            .map_err(|_| AdminClientTargetError::InvalidUri)?;
        if uri.scheme().is_some() || uri.authority().is_some() {
            return Err(AdminClientTargetError::AuthorityForbidden);
        }
        let path = uri.path();
        if !path.starts_with("/v1/") || path.ends_with('/') {
            return Err(AdminClientTargetError::WrongVersionPrefix);
        }
        if path.len() > ADMIN_ROUTE_PATH_MAX_UTF8_BYTES {
            return Err(AdminClientTargetError::PathTooLong);
        }
        if path.contains("//") {
            return Err(AdminClientTargetError::EmptySegment);
        }
        if path.contains(['{', '}']) {
            return Err(AdminClientTargetError::PatternForbidden);
        }
        if !valid_percent_encoding(value.as_bytes()) {
            return Err(AdminClientTargetError::InvalidPercentEncoding);
        }
        Ok(Self { uri })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.uri.path_and_query().map_or("", |value| value.as_str())
    }

    #[must_use]
    pub fn path(&self) -> &str {
        self.uri.path()
    }

    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.uri.query()
    }
}

impl fmt::Debug for AdminClientTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminClientTarget")
            .field("path", &"[redacted]")
            .field("has_query", &self.uri.query().is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminClientErrorKind {
    SocketPath,
    QueryLimit,
    RequestEncoding,
    RequestLimit,
    Connect,
    Transport,
    Deadline,
    ResponseHeaders,
    ResponseLimit,
    ResponseContentType,
    ResponseHttpVersion,
    MalformedResponse,
    UnsupportedContractVersion,
    ServerFailure,
}

pub struct AdminClientError {
    kind: AdminClientErrorKind,
    io_kind: Option<io::ErrorKind>,
    failure: Option<AdminFailureResponse>,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl AdminClientError {
    fn simple(kind: AdminClientErrorKind) -> Self {
        Self {
            kind,
            io_kind: None,
            failure: None,
            source: None,
        }
    }

    fn sourced<E>(kind: AdminClientErrorKind, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            io_kind: None,
            failure: None,
            source: Some(Box::new(source)),
        }
    }

    fn connect(source: io::Error) -> Self {
        Self {
            kind: AdminClientErrorKind::Connect,
            io_kind: Some(source.kind()),
            failure: None,
            source: Some(Box::new(source)),
        }
    }

    fn boxed(kind: AdminClientErrorKind, source: Box<dyn Error + Send + Sync>) -> Self {
        Self {
            kind,
            io_kind: None,
            failure: None,
            source: Some(source),
        }
    }

    fn server(failure: AdminFailureResponse) -> Self {
        Self {
            kind: AdminClientErrorKind::ServerFailure,
            io_kind: None,
            failure: Some(failure),
            source: None,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> AdminClientErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn io_kind(&self) -> Option<io::ErrorKind> {
        self.io_kind
    }

    #[must_use]
    pub const fn failure(&self) -> Option<&AdminFailureResponse> {
        self.failure.as_ref()
    }
}

impl fmt::Debug for AdminClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminClientError")
            .field("kind", &self.kind)
            .field("io_kind", &self.io_kind)
            .field(
                "failure_code",
                &self.failure.as_ref().map(|failure| failure.error().code()),
            )
            .field("source", &self.source.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Display for AdminClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded local admin request failed")
    }
}

impl Error for AdminClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}

/// Bounded client for one service-owned Unix administration socket.
pub struct AdminClient {
    socket_path: PathBuf,
    limits: AdminTransportLimits,
}

impl AdminClient {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        limits: AdminTransportLimits,
    ) -> Result<Self, AdminClientError> {
        let socket_path = socket_path.into();
        if !socket_path.is_absolute() || socket_path.file_name().is_none() {
            return Err(AdminClientError::simple(AdminClientErrorKind::SocketPath));
        }
        Ok(Self {
            socket_path,
            limits,
        })
    }

    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    #[must_use]
    pub const fn limits(&self) -> AdminTransportLimits {
        self.limits
    }

    pub async fn get<T>(
        &self,
        target: &AdminClientTarget,
    ) -> Result<AdminSuccessResponse<T>, AdminClientError>
    where
        T: DeserializeOwned + Serialize,
    {
        self.execute(AdminHttpMethod::Get, target, Bytes::new())
            .await
    }

    pub async fn mutate<RequestBody, ResponseBody>(
        &self,
        target: &AdminClientTarget,
        operation_id: AdminOperationId,
        correlation_id: Option<AdminCorrelationId>,
        request: RequestBody,
    ) -> Result<AdminSuccessResponse<ResponseBody>, AdminClientError>
    where
        RequestBody: Serialize,
        ResponseBody: DeserializeOwned + Serialize,
    {
        let envelope = ClientMutationEnvelope {
            contract_version: ADMIN_CONTRACT_VERSION,
            operation_id: &operation_id,
            correlation_id: correlation_id.as_ref(),
            request: &request,
        };
        let body = encode_bounded(&envelope, self.limits.request_body_utf8_bytes() as usize)
            .map_err(|error| match error {
                ClientEncodingError::Limit => {
                    AdminClientError::simple(AdminClientErrorKind::RequestLimit)
                }
                ClientEncodingError::Encoding(error) => {
                    AdminClientError::sourced(AdminClientErrorKind::RequestEncoding, error)
                }
            })?;
        let _: StrictJsonPayload = serde_json::from_slice(&body).map_err(|error| {
            AdminClientError::sourced(AdminClientErrorKind::RequestEncoding, error)
        })?;
        self.execute(AdminHttpMethod::Post, target, Bytes::from(body))
            .await
    }

    async fn execute<T>(
        &self,
        method: AdminHttpMethod,
        target: &AdminClientTarget,
        body: Bytes,
    ) -> Result<AdminSuccessResponse<T>, AdminClientError>
    where
        T: DeserializeOwned + Serialize,
    {
        if query_item_count(target.query()) > self.limits.query_items() as usize {
            return Err(AdminClientError::simple(AdminClientErrorKind::QueryLimit));
        }
        let deadline = self.limits.request_deadline();
        tokio::time::timeout(deadline, self.execute_inner(method, target, body))
            .await
            .map_err(|error| AdminClientError::sourced(AdminClientErrorKind::Deadline, error))?
    }

    async fn execute_inner<T>(
        &self,
        method: AdminHttpMethod,
        target: &AdminClientTarget,
        body: Bytes,
    ) -> Result<AdminSuccessResponse<T>, AdminClientError>
    where
        T: DeserializeOwned + Serialize,
    {
        let stream = tokio::net::UnixStream::connect(&self.socket_path)
            .await
            .map_err(AdminClientError::connect)?;
        let mut connection_builder = http1::Builder::new();
        connection_builder
            .max_headers(self.limits.header_count() as usize)
            .max_buf_size((self.limits.header_bytes() as usize).max(HTTP_MINIMUM_MAX_BUFFER_SIZE));
        let (mut sender, connection) = connection_builder
            .handshake(TokioIo::new(ClientUnixStream(stream)))
            .await
            .map_err(|error| AdminClientError::sourced(AdminClientErrorKind::Transport, error))?;
        let driver = ConnectionDriver::new(tokio::spawn(connection));

        let http_method = match method {
            AdminHttpMethod::Get => Method::GET,
            AdminHttpMethod::Post => Method::POST,
        };
        let mut builder = Request::builder()
            .version(Version::HTTP_11)
            .method(http_method)
            .uri(target.uri.clone())
            .header(HOST, HeaderValue::from_static("localhost"));
        if method == AdminHttpMethod::Post {
            builder = builder.header(CONTENT_TYPE, HeaderValue::from_static(JSON_CONTENT_TYPE));
        }
        let request = builder
            .body(Full::new(body))
            .expect("validated target and static headers must build");
        let response = sender
            .send_request(request)
            .await
            .map_err(|error| AdminClientError::sourced(AdminClientErrorKind::Transport, error))?;

        if response.version() != Version::HTTP_11 {
            return Err(AdminClientError::simple(
                AdminClientErrorKind::ResponseHttpVersion,
            ));
        }
        if header_bytes(response.headers()) > u64::from(self.limits.header_bytes())
            || response.headers().len() > self.limits.header_count() as usize
        {
            return Err(AdminClientError::simple(
                AdminClientErrorKind::ResponseHeaders,
            ));
        }
        if !is_json_content_type(response.headers()) {
            return Err(AdminClientError::simple(
                AdminClientErrorKind::ResponseContentType,
            ));
        }
        let response_limit = self.limits.response_body_utf8_bytes() as usize;
        if content_length(response.headers()).is_some_and(|length| length > response_limit as u64) {
            return Err(AdminClientError::simple(
                AdminClientErrorKind::ResponseLimit,
            ));
        }
        let status = response.status();
        let body = Limited::new(response.into_body(), response_limit)
            .collect()
            .await
            .map_err(|error| {
                if error.downcast_ref::<LengthLimitError>().is_some() {
                    AdminClientError::simple(AdminClientErrorKind::ResponseLimit)
                } else {
                    AdminClientError::boxed(AdminClientErrorKind::Transport, error)
                }
            })?
            .to_bytes();
        driver.finish().await?;
        drop(sender);
        decode_response(status, &body)
    }
}

impl fmt::Debug for AdminClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminClient")
            .field("socket_path", &"[redacted]")
            .field("limits", &self.limits)
            .finish()
    }
}

#[derive(Serialize)]
struct ClientMutationEnvelope<'a, T> {
    contract_version: u32,
    operation_id: &'a AdminOperationId,
    #[serde(skip_serializing_if = "Option::is_none")]
    correlation_id: Option<&'a AdminCorrelationId>,
    request: &'a T,
}

struct ConnectionDriver {
    handle: JoinHandle<Result<(), hyper::Error>>,
    completed: bool,
}

struct ClientUnixStream(tokio::net::UnixStream);

impl AsyncRead for ClientUnixStream {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(context, buffer)
    }
}

impl AsyncWrite for ClientUnixStream {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.0).poll_write(context, buffer)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.0).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match Pin::new(&mut self.0).poll_shutdown(context) {
            Poll::Ready(Err(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::BrokenPipe | io::ErrorKind::NotConnected
                ) =>
            {
                Poll::Ready(Ok(()))
            }
            result => result,
        }
    }
}

impl ConnectionDriver {
    fn new(handle: JoinHandle<Result<(), hyper::Error>>) -> Self {
        Self {
            handle,
            completed: false,
        }
    }

    async fn finish(mut self) -> Result<(), AdminClientError> {
        let result = match (&mut self.handle).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(AdminClientError::sourced(
                AdminClientErrorKind::Transport,
                error,
            )),
            Err(error) => Err(AdminClientError::sourced(
                AdminClientErrorKind::Transport,
                error,
            )),
        };
        self.completed = true;
        result
    }
}

impl Drop for ConnectionDriver {
    fn drop(&mut self) {
        if !self.completed {
            self.handle.abort();
        }
    }
}

fn decode_response<T>(
    status: StatusCode,
    body: &[u8],
) -> Result<AdminSuccessResponse<T>, AdminClientError>
where
    T: DeserializeOwned + Serialize,
{
    let strict = serde_json::from_slice::<StrictResponseEnvelope>(body).map_err(|error| {
        AdminClientError::sourced(AdminClientErrorKind::MalformedResponse, error)
    })?;
    if strict.contract_version != u64::from(ADMIN_CONTRACT_VERSION) {
        return Err(AdminClientError::simple(
            AdminClientErrorKind::UnsupportedContractVersion,
        ));
    }
    if status.is_success() {
        serde_json::from_slice::<AdminSuccessResponse<T>>(body).map_err(|error| {
            AdminClientError::sourced(AdminClientErrorKind::MalformedResponse, error)
        })
    } else {
        let failure = serde_json::from_slice::<AdminFailureResponse>(body).map_err(|error| {
            AdminClientError::sourced(AdminClientErrorKind::MalformedResponse, error)
        })?;
        Err(AdminClientError::server(failure))
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

fn valid_percent_encoding(value: &[u8]) -> bool {
    let mut index = 0;
    while index < value.len() {
        if value[index] == b'%' {
            let Some(high) = value.get(index + 1) else {
                return false;
            };
            let Some(low) = value.get(index + 2) else {
                return false;
            };
            if !high.is_ascii_hexdigit() || !low.is_ascii_hexdigit() {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}

fn is_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value == JSON_CONTENT_TYPE || value == "application/json; charset=utf-8"
        })
}

fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
}

enum ClientEncodingError {
    Limit,
    Encoding(serde_json::Error),
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

fn encode_bounded<T>(value: &T, limit: usize) -> Result<Vec<u8>, ClientEncodingError>
where
    T: Serialize,
{
    let mut writer = CappedWriter::new(limit);
    match serde_json::to_writer(&mut writer, value) {
        Ok(()) => Ok(writer.bytes),
        Err(_) if writer.exceeded => Err(ClientEncodingError::Limit),
        Err(error) => Err(ClientEncodingError::Encoding(error)),
    }
}

#[derive(Clone, Copy)]
struct StrictJsonPayload;

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

struct StrictResponseEnvelope {
    contract_version: u64,
}

impl<'de> Deserialize<'de> for StrictResponseEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(StrictResponseVisitor)
    }
}

struct StrictResponseVisitor;

impl<'de> de::Visitor<'de> for StrictResponseVisitor {
    type Value = StrictResponseEnvelope;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an admin response envelope without duplicate keys or null values")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        let mut contract_version = None;
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            if key == "contract_version" {
                contract_version = Some(map.next_value::<u64>()?);
            } else {
                map.next_value::<StrictJsonPayload>()?;
            }
        }
        Ok(StrictResponseEnvelope {
            contract_version: contract_version
                .ok_or_else(|| de::Error::missing_field("contract_version"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AdminError, AdminErrorCode, AdminErrorMessage, AdminMutationRequest, AdminRouteFailure,
        AdminRouteFailureStatus, AdminRouteOutcome, AdminRouter, AdminServer, CancellationToken,
        EntropyError, EntropySource, UnixAdminSocketBinding, UnixAdminSocketWriterAuthority,
    };
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Notify;

    #[derive(Clone, Copy)]
    struct FixedEntropy;

    impl EntropySource for FixedEntropy {
        fn fill_bytes(&self, destination: &mut [u8]) -> Result<(), EntropyError> {
            destination.fill(0x44);
            Ok(())
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct EchoRequest {
        value: String,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct EchoResponse {
        value: String,
    }

    fn known_error(code: &'static str, message: &'static str) -> AdminError {
        AdminError::new(
            AdminErrorCode::new(code).expect("error code"),
            AdminErrorMessage::new(message).expect("error message"),
        )
    }

    fn echo_router() -> AdminRouter {
        let mut router = AdminRouter::new();
        router
            .route(AdminHttpMethod::Post, "/v1/echo", |request| async move {
                match request.decode_json::<AdminMutationRequest<EchoRequest>>() {
                    Ok(envelope) => request
                        .success(&EchoResponse {
                            value: envelope.into_request().value,
                        })
                        .expect("echo response"),
                    Err(_) => AdminRouteOutcome::failure(AdminRouteFailure::new(
                        AdminRouteFailureStatus::BadRequest,
                        known_error("invalid_echo", "echo request is invalid"),
                    )),
                }
            })
            .expect("echo route");
        router
    }

    async fn binding(directory: &tempfile::TempDir) -> (PathBuf, UnixAdminSocketBinding) {
        let socket = directory.path().join("admin.sock");
        let authority =
            UnixAdminSocketWriterAuthority::acquire(directory.path()).expect("writer authority");
        let binding = UnixAdminSocketBinding::bind(authority, &socket)
            .await
            .expect("socket binding");
        (socket, binding)
    }

    async fn fake_server(
        directory: &tempfile::TempDir,
        name: &str,
        response: Vec<u8>,
        delay: std::time::Duration,
    ) -> (PathBuf, JoinHandle<()>) {
        let socket = directory.path().join(name);
        let listener = tokio::net::UnixListener::bind(&socket).expect("fake listener");
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("fake accept");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).await;
            tokio::time::sleep(delay).await;
            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        });
        (socket, task)
    }

    fn raw_response(version: &str, status: &str, body: &str) -> Vec<u8> {
        format!(
            "{version} {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn limits_with(
        request_body: u32,
        response_body: u32,
        query_items: u32,
        deadline: std::time::Duration,
    ) -> AdminTransportLimits {
        let mut values = AdminTransportLimits::DEFAULT.values();
        values.request_body_utf8_bytes = request_body;
        values.response_body_utf8_bytes = response_body;
        values.query_items = query_items;
        values.request_deadline = deadline;
        AdminTransportLimits::new(values).expect("client limits")
    }

    #[tokio::test]
    async fn server_client_round_trip_preserves_version_and_correlation() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let (socket, binding) = binding(&directory).await;
        let server = AdminServer::new(echo_router(), AdminTransportLimits::DEFAULT, FixedEntropy)
            .expect("admin server");
        let cancellation = CancellationToken::new();
        let server_task = tokio::spawn(server.serve(binding, cancellation.clone()));
        let client = AdminClient::new(&socket, AdminTransportLimits::DEFAULT).expect("client");
        let target = AdminClientTarget::new("/v1/echo").expect("target");
        let correlation = AdminCorrelationId::new("client-round-trip").expect("correlation");

        let response = client
            .mutate::<_, EchoResponse>(
                &target,
                AdminOperationId::new("echo-01").expect("operation"),
                Some(correlation.clone()),
                EchoRequest {
                    value: "bounded".to_owned(),
                },
            )
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "round trip: {error:?}; source={:?}",
                    error.source().map(ToString::to_string)
                )
            });
        assert_eq!(response.correlation_id(), &correlation);
        assert_eq!(
            response.into_result(),
            EchoResponse {
                value: "bounded".to_owned()
            }
        );

        let missing = AdminClientTarget::new("/v1/missing").expect("missing target");
        let error = client
            .get::<EchoResponse>(&missing)
            .await
            .expect_err("missing route");
        assert_eq!(error.kind(), AdminClientErrorKind::ServerFailure);
        assert_eq!(
            error
                .failure()
                .expect("server failure")
                .error()
                .code()
                .as_str(),
            "route_not_found"
        );

        cancellation.cancel();
        server_task
            .await
            .expect("server task")
            .expect("server shutdown");
    }

    #[tokio::test]
    async fn unavailable_socket_and_deadline_are_typed_and_safe() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let missing = directory.path().join("missing.sock");
        let client = AdminClient::new(&missing, AdminTransportLimits::DEFAULT).expect("client");
        let target = AdminClientTarget::new("/v1/status").expect("target");
        let error = client
            .get::<EchoResponse>(&target)
            .await
            .expect_err("unavailable socket");
        assert_eq!(error.kind(), AdminClientErrorKind::Connect);
        assert!(error.io_kind().is_some());
        assert!(!format!("{error:?}").contains("missing.sock"));

        let body =
            r#"{"contract_version":1,"ok":true,"correlation_id":"late","result":{"value":"late"}}"#;
        let (socket, task) = fake_server(
            &directory,
            "slow.sock",
            raw_response("HTTP/1.1", "200 OK", body),
            std::time::Duration::from_millis(100),
        )
        .await;
        let client = AdminClient::new(
            socket,
            limits_with(1024, 1024, 10, std::time::Duration::from_millis(20)),
        )
        .expect("slow client");
        let error = client
            .get::<EchoResponse>(&target)
            .await
            .expect_err("deadline");
        assert_eq!(error.kind(), AdminClientErrorKind::Deadline);
        task.await.expect("fake task");
    }

    #[tokio::test]
    async fn version_malformed_duplicate_and_oversized_responses_fail_closed() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let target = AdminClientTarget::new("/v1/status").expect("target");
        let cases = [
            (
                "version.sock",
                raw_response(
                    "HTTP/1.1",
                    "200 OK",
                    r#"{"contract_version":2,"ok":true,"correlation_id":"future","result":{"value":"future"}}"#,
                ),
                AdminClientErrorKind::UnsupportedContractVersion,
            ),
            (
                "malformed.sock",
                raw_response("HTTP/1.1", "200 OK", "{"),
                AdminClientErrorKind::MalformedResponse,
            ),
            (
                "duplicate.sock",
                raw_response(
                    "HTTP/1.1",
                    "200 OK",
                    r#"{"contract_version":1,"contract_version":1,"ok":true,"correlation_id":"duplicate","result":{"value":"duplicate"}}"#,
                ),
                AdminClientErrorKind::MalformedResponse,
            ),
            (
                "null.sock",
                raw_response(
                    "HTTP/1.1",
                    "200 OK",
                    r#"{"contract_version":1,"ok":true,"correlation_id":"null-result","result":{"value":null}}"#,
                ),
                AdminClientErrorKind::MalformedResponse,
            ),
            (
                "http10.sock",
                raw_response(
                    "HTTP/1.0",
                    "200 OK",
                    r#"{"contract_version":1,"ok":true,"correlation_id":"old-http","result":{"value":"old"}}"#,
                ),
                AdminClientErrorKind::ResponseHttpVersion,
            ),
        ];
        for (name, response, expected) in cases {
            let (socket, task) =
                fake_server(&directory, name, response, std::time::Duration::ZERO).await;
            let client = AdminClient::new(socket, AdminTransportLimits::DEFAULT).expect("client");
            let error = client
                .get::<EchoResponse>(&target)
                .await
                .expect_err("invalid response");
            assert_eq!(
                error.kind(),
                expected,
                "source={:?}",
                error.source().map(ToString::to_string)
            );
            task.await.expect("fake task");
        }

        let oversized = "x".repeat(513);
        let (socket, task) = fake_server(
            &directory,
            "oversized.sock",
            raw_response("HTTP/1.1", "200 OK", &oversized),
            std::time::Duration::ZERO,
        )
        .await;
        let client = AdminClient::new(
            socket,
            limits_with(1024, 512, 10, std::time::Duration::from_secs(1)),
        )
        .expect("client");
        let error = client
            .get::<EchoResponse>(&target)
            .await
            .expect_err("oversized response");
        assert_eq!(error.kind(), AdminClientErrorKind::ResponseLimit);
        task.await.expect("fake task");

        let (socket, task) = fake_server(
            &directory,
            "bad.sock",
            b"not-http\r\n\r\n".to_vec(),
            std::time::Duration::ZERO,
        )
        .await;
        let client = AdminClient::new(socket, AdminTransportLimits::DEFAULT).expect("client");
        let error = client
            .get::<EchoResponse>(&target)
            .await
            .expect_err("invalid HTTP response");
        assert_eq!(error.kind(), AdminClientErrorKind::Transport);
        task.await.expect("fake task");
    }

    #[tokio::test]
    async fn request_and_query_limits_fail_before_socket_access() {
        let directory = tempfile::tempdir().expect("runtime directory");
        let missing = directory.path().join("missing.sock");
        let client = AdminClient::new(
            &missing,
            limits_with(32, 512, 1, std::time::Duration::from_secs(1)),
        )
        .expect("client");
        let target = AdminClientTarget::new("/v1/mutate").expect("target");
        let error = client
            .mutate::<_, EchoResponse>(
                &target,
                AdminOperationId::new("oversized-01").expect("operation"),
                None,
                EchoRequest {
                    value: "x".repeat(128),
                },
            )
            .await
            .expect_err("request limit");
        assert_eq!(error.kind(), AdminClientErrorKind::RequestLimit);

        let null_client =
            AdminClient::new(&missing, AdminTransportLimits::DEFAULT).expect("null client");
        let error = null_client
            .mutate::<_, EchoResponse>(
                &target,
                AdminOperationId::new("null-01").expect("operation"),
                None,
                Option::<EchoRequest>::None,
            )
            .await
            .expect_err("null request");
        assert_eq!(error.kind(), AdminClientErrorKind::RequestEncoding);

        let query = AdminClientTarget::new("/v1/status?a=1&b=2").expect("query target");
        let error = client
            .get::<EchoResponse>(&query)
            .await
            .expect_err("query limit");
        assert_eq!(error.kind(), AdminClientErrorKind::QueryLimit);
    }

    #[test]
    fn target_client_and_error_debug_bound_and_redact_authority() {
        for invalid in [
            "",
            "http://localhost/v1/status",
            "/v2/status",
            "/v1/{route}",
            "/v1/status/",
            "/v1/items/%2",
        ] {
            assert!(AdminClientTarget::new(invalid).is_err(), "{invalid}");
        }
        let target =
            AdminClientTarget::new("/v1/items/secret-id?token=protected").expect("redacted target");
        let debug = format!("{target:?}");
        assert!(!debug.contains("secret-id"));
        assert!(!debug.contains("protected"));

        let directory = tempfile::tempdir().expect("runtime directory");
        let socket = directory.path().join("protected-admin.sock");
        let client = AdminClient::new(&socket, AdminTransportLimits::DEFAULT).expect("client");
        assert!(!format!("{client:?}").contains("protected-admin.sock"));
        assert_eq!(client.socket_path(), socket);
        assert_eq!(client.limits(), AdminTransportLimits::DEFAULT);
        assert_eq!(target.path(), "/v1/items/secret-id");
        assert_eq!(target.query(), Some("token=protected"));
    }

    #[test]
    fn client_source_has_no_runtime_tcp_or_process_authority() {
        let source = include_str!("client.rs");
        for forbidden in [
            concat!("Tcp", "Stream"),
            concat!("Runtime", "::new"),
            concat!("process", "::exit"),
            concat!("tokio", "::signal"),
        ] {
            assert!(
                !source.contains(forbidden),
                "forbidden client authority: {forbidden}"
            );
        }
        assert!(source.contains("UnixStream::connect"));
    }

    #[test]
    fn shared_client_handles_are_thread_safe() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AdminClient>();
        assert_send_sync::<Arc<AdminClient>>();
    }

    #[tokio::test]
    async fn cancelling_driver_finish_aborts_and_drops_the_connection_task() {
        struct DropNotify(Arc<Notify>);

        impl Drop for DropNotify {
            fn drop(&mut self) {
                self.0.notify_one();
            }
        }

        let dropped = Arc::new(Notify::new());
        let task_dropped = Arc::clone(&dropped);
        let handle = tokio::spawn(async move {
            let _drop_notify = DropNotify(task_dropped);
            std::future::pending::<()>().await;
            Ok::<(), hyper::Error>(())
        });
        let driver = ConnectionDriver::new(handle);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), driver.finish())
                .await
                .is_err()
        );
        tokio::time::timeout(std::time::Duration::from_secs(1), dropped.notified())
            .await
            .expect("aborted driver task must drop");
    }

    #[test]
    fn strict_response_target_and_error_helpers_cover_the_full_value_surface() {
        for document in [
            "true",
            "-3",
            "4",
            "2.5",
            r#""text""#,
            "[true,2]",
            r#"{"value":3}"#,
        ] {
            serde_json::from_str::<StrictJsonPayload>(document).unwrap();
        }
        for rejected in ["null", "[1,null]", r#"{"same":1,"same":2}"#] {
            assert!(serde_json::from_str::<StrictJsonPayload>(rejected).is_err());
        }

        let target = AdminClientTarget::new("/v1/items/value%2D1?page=1&limit=2").unwrap();
        assert_eq!(target.as_str(), "/v1/items/value%2D1?page=1&limit=2");
        assert_eq!(target.path(), "/v1/items/value%2D1");
        assert_eq!(target.query(), Some("page=1&limit=2"));
        assert_eq!(query_item_count(None), 0);
        assert_eq!(query_item_count(Some("")), 0);
        assert_eq!(query_item_count(target.query()), 2);
        assert!(valid_percent_encoding(b"/v1/items/%2d"));
        assert!(!valid_percent_encoding(b"/v1/items/%"));
        assert!(!valid_percent_encoding(b"/v1/items/%2"));
        assert!(!valid_percent_encoding(b"/v1/items/%GG"));
        assert!(!valid_percent_encoding(b"/v1/items/%G0"));
        assert!(!valid_percent_encoding(b"/v1/items/%0G"));

        let invalid_targets = [
            (String::new(), AdminClientTargetError::Empty),
            (
                "x".repeat(ADMIN_CLIENT_TARGET_MAX_UTF8_BYTES + 1),
                AdminClientTargetError::TooLong,
            ),
            (
                "http://[invalid".to_owned(),
                AdminClientTargetError::InvalidUri,
            ),
            (
                "http://localhost/v1/status".to_owned(),
                AdminClientTargetError::AuthorityForbidden,
            ),
            (
                "/v2/status".to_owned(),
                AdminClientTargetError::WrongVersionPrefix,
            ),
            (
                format!("/v1/{}", "x".repeat(ADMIN_ROUTE_PATH_MAX_UTF8_BYTES)),
                AdminClientTargetError::PathTooLong,
            ),
            (
                "/v1//status".to_owned(),
                AdminClientTargetError::EmptySegment,
            ),
            (
                "/v1/{status}".to_owned(),
                AdminClientTargetError::PatternForbidden,
            ),
            (
                "/v1/items/%GG".to_owned(),
                AdminClientTargetError::InvalidPercentEncoding,
            ),
        ];
        for (target, expected) in invalid_targets {
            assert_eq!(AdminClientTarget::new(target).unwrap_err(), expected);
        }
        assert!(!AdminClientTargetError::Empty.to_string().is_empty());
        assert_eq!(
            AdminClient::new("relative.sock", AdminTransportLimits::DEFAULT)
                .unwrap_err()
                .kind(),
            AdminClientErrorKind::SocketPath
        );
        assert_eq!(
            AdminClient::new("/", AdminTransportLimits::DEFAULT)
                .unwrap_err()
                .kind(),
            AdminClientErrorKind::SocketPath
        );

        let mut headers = HeaderMap::new();
        assert!(!is_json_content_type(&headers));
        assert_eq!(content_length(&headers), None);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static(JSON_CONTENT_TYPE));
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("12"));
        assert!(is_json_content_type(&headers));
        assert_eq!(content_length(&headers), Some(12));
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("invalid"));
        assert!(is_json_content_type(&headers));
        assert_eq!(content_length(&headers), None);
        assert!(header_bytes(&headers) > 0);

        let failure = AdminFailureResponse::new(
            AdminCorrelationId::new("safe-correlation").unwrap(),
            known_error("known_failure", "known failure"),
        );
        let server = AdminClientError::server(failure.clone());
        assert_eq!(server.kind(), AdminClientErrorKind::ServerFailure);
        assert_eq!(server.failure(), Some(&failure));
        assert_eq!(server.io_kind(), None);
        assert!(server.source().is_none());
        assert!(!server.to_string().is_empty());

        let connect = AdminClientError::connect(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "sensitive socket",
        ));
        assert_eq!(connect.kind(), AdminClientErrorKind::Connect);
        assert_eq!(connect.io_kind(), Some(io::ErrorKind::ConnectionRefused));
        assert!(connect.source().is_some());
        assert!(!format!("{connect:?}").contains("sensitive socket"));

        let malformed = decode_response::<EchoResponse>(StatusCode::OK, b"[]").unwrap_err();
        assert_eq!(malformed.kind(), AdminClientErrorKind::MalformedResponse);
        let unsupported = decode_response::<EchoResponse>(
            StatusCode::OK,
            br#"{"contract_version":2,"ok":true,"correlation_id":"safe","result":{"value":"x"}}"#,
        )
        .unwrap_err();
        assert_eq!(
            unsupported.kind(),
            AdminClientErrorKind::UnsupportedContractVersion
        );

        use std::io::Write as _;
        let mut writer = CappedWriter::new(4);
        writer.flush().unwrap();
        assert_eq!(writer.write(b"four").unwrap(), 4);
        assert!(writer.write(b"x").is_err());
    }
}
