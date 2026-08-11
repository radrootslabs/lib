//! Bounded HTTP/1.1 operations server with an exact passive route inventory.

use core::fmt;
use std::convert::Infallible;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use http::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue};
use http::{Method, Request, Response, StatusCode, Version};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::{
    BoundedMetricsSnapshot, LIVEZ_PATH, METRICS_CONTENT_TYPE, OPERATIONS_HEALTH_CONTENT_TYPE,
    OperationsListenerConfig, OperationsTransportLimits, READYZ_PATH, livez, readyz,
};
use crate::{CachedServiceStateReader, CancellationToken, MonotonicClock, SystemMonotonicClock};

pub const METRICS_PATH: &str = "/metrics";

pub const OPERATIONS_HTTP_MIN_HEADER_BYTES: u32 = 8 * 1024;
const NOT_FOUND_BODY: &[u8] = b"not found\n";
const VERSION_UNSUPPORTED_BODY: &[u8] = b"HTTP/1.1 required\n";
const HEADERS_TOO_LARGE_BODY: &[u8] = b"request headers too large\n";
const METRICS_UNAVAILABLE_BODY: &[u8] = b"metrics unavailable\n";
const REQUEST_TIMEOUT_BODY: &[u8] = b"request timeout\n";
const NO_STORE: HeaderValue = HeaderValue::from_static("no-store");

/// Safe runtime failure for the operations server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationsServerError {
    Disabled,
    HeaderLimitBelowParserFloor,
    Bind { kind: io::ErrorKind },
    LocalAddress { kind: io::ErrorKind },
    Accept { kind: io::ErrorKind },
    ConnectionTaskPanicked,
}

impl fmt::Display for OperationsServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded operations server failed")
    }
}

impl Error for OperationsServerError {}

struct OperationsServerState {
    cache: CachedServiceStateReader<BoundedMetricsSnapshot>,
    limits: OperationsTransportLimits,
    clock: Arc<dyn MonotonicClock>,
}

/// An unbound operations server with no route-registration extension point.
pub struct OperationsServer {
    listen: SocketAddr,
    state: Arc<OperationsServerState>,
}

impl fmt::Debug for OperationsServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationsServer")
            .field("listen", &"[redacted]")
            .field("limits", &self.state.limits)
            .finish()
    }
}

impl OperationsServer {
    pub fn new(
        config: OperationsListenerConfig,
        cache: CachedServiceStateReader<BoundedMetricsSnapshot>,
    ) -> Result<Self, OperationsServerError> {
        Self::new_with_clock(config, cache, SystemMonotonicClock::new())
    }

    pub fn new_with_clock<C>(
        config: OperationsListenerConfig,
        cache: CachedServiceStateReader<BoundedMetricsSnapshot>,
        clock: C,
    ) -> Result<Self, OperationsServerError>
    where
        C: MonotonicClock + 'static,
    {
        let listen = config
            .listen()
            .ok_or(OperationsServerError::Disabled)?
            .socket_addr();
        let limits = config.limits().ok_or(OperationsServerError::Disabled)?;
        if limits.header_bytes() < OPERATIONS_HTTP_MIN_HEADER_BYTES {
            return Err(OperationsServerError::HeaderLimitBelowParserFloor);
        }
        Ok(Self {
            listen,
            state: Arc::new(OperationsServerState {
                cache,
                limits,
                clock: Arc::new(clock),
            }),
        })
    }

    /// Binds the exact validated socket address without starting admission.
    pub async fn bind(self) -> Result<BoundOperationsServer, OperationsServerError> {
        let listener = TcpListener::bind(self.listen)
            .await
            .map_err(|error| OperationsServerError::Bind { kind: error.kind() })?;
        let local_address = listener
            .local_addr()
            .map_err(|error| OperationsServerError::LocalAddress { kind: error.kind() })?;
        Ok(BoundOperationsServer {
            listener,
            local_address,
            state: self.state,
        })
    }
}

/// One successfully bound listener ready for explicit cancellation-owned service.
pub struct BoundOperationsServer {
    listener: TcpListener,
    local_address: SocketAddr,
    state: Arc<OperationsServerState>,
}

impl fmt::Debug for BoundOperationsServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundOperationsServer")
            .field("local_address", &"[redacted]")
            .field("limits", &self.state.limits)
            .finish()
    }
}

impl BoundOperationsServer {
    #[must_use]
    pub const fn local_address(&self) -> SocketAddr {
        self.local_address
    }

    /// Stops admission on cancellation and drains every bounded connection task.
    pub async fn serve(self, cancellation: CancellationToken) -> Result<(), OperationsServerError> {
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
                        break Err(OperationsServerError::ConnectionTaskPanicked);
                    }
                }
                accepted = self.listener.accept() => {
                    let (stream, _) = match accepted {
                        Ok(accepted) => accepted,
                        Err(error) => {
                            cancellation.cancel();
                            break Err(OperationsServerError::Accept { kind: error.kind() });
                        }
                    };
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
                result = Err(OperationsServerError::ConnectionTaskPanicked);
            }
        }
        result
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    state: Arc<OperationsServerState>,
    cancellation: CancellationToken,
) {
    let admitted = tokio::select! {
        biased;
        () = cancellation.cancelled() => return,
        admitted = tokio::time::timeout(
            state.limits.idle_timeout(),
            read_request_head(&mut stream, state.limits.header_bytes() as usize),
        ) => admitted,
    };
    let prefix = match admitted {
        Ok(Ok(prefix)) => prefix,
        Ok(Err(RequestHeadError::TooLarge)) => {
            let _ = tokio::time::timeout(
                state.limits.request_deadline(),
                write_header_limit_response(&mut stream, state.limits),
            )
            .await;
            return;
        }
        Ok(Err(RequestHeadError::Incomplete | RequestHeadError::Read)) | Err(_) => return,
    };

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
        .max_buf_size(state.limits.header_bytes() as usize)
        .header_read_timeout(state.limits.idle_timeout())
        .timer(TokioTimer::new());

    let connection_deadline = state
        .limits
        .request_deadline()
        .saturating_add(state.limits.idle_timeout());
    let stream = PrefixedTcpStream::new(prefix, stream);
    let mut connection = Box::pin(builder.serve_connection(TokioIo::new(stream), service));
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
    state: Arc<OperationsServerState>,
) -> Response<Full<Bytes>> {
    let deadline = match state.clock.deadline_after(state.limits.request_deadline()) {
        Ok(deadline) => deadline,
        Err(_) => {
            return fixed_response(
                StatusCode::GATEWAY_TIMEOUT,
                OPERATIONS_HEALTH_CONTENT_TYPE,
                REQUEST_TIMEOUT_BODY,
                state.limits,
            );
        }
    };
    if deadline.is_reached_at(state.clock.now_monotonic()) {
        return fixed_response(
            StatusCode::GATEWAY_TIMEOUT,
            OPERATIONS_HEALTH_CONTENT_TYPE,
            REQUEST_TIMEOUT_BODY,
            state.limits,
        );
    }
    let response = process_request(request, Arc::clone(&state));
    if deadline.is_reached_at(state.clock.now_monotonic()) {
        fixed_response(
            StatusCode::GATEWAY_TIMEOUT,
            OPERATIONS_HEALTH_CONTENT_TYPE,
            REQUEST_TIMEOUT_BODY,
            state.limits,
        )
    } else {
        response
    }
}

fn process_request(
    request: Request<Incoming>,
    state: Arc<OperationsServerState>,
) -> Response<Full<Bytes>> {
    if request.version() != Version::HTTP_11 {
        return fixed_response(
            StatusCode::HTTP_VERSION_NOT_SUPPORTED,
            OPERATIONS_HEALTH_CONTENT_TYPE,
            VERSION_UNSUPPORTED_BODY,
            state.limits,
        );
    }
    if request.headers().len() > state.limits.header_count() as usize {
        return fixed_response(
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            OPERATIONS_HEALTH_CONTENT_TYPE,
            HEADERS_TOO_LARGE_BODY,
            state.limits,
        );
    }
    if request.method() != Method::GET || request.uri().query().is_some() {
        return not_found(state.limits);
    }

    match request.uri().path() {
        LIVEZ_PATH => {
            let response = livez(&state.cache);
            fixed_response(
                response.status(),
                response.content_type(),
                response.body(),
                state.limits,
            )
        }
        READYZ_PATH => {
            let response = readyz(&state.cache);
            fixed_response(
                response.status(),
                response.content_type(),
                response.body(),
                state.limits,
            )
        }
        METRICS_PATH => {
            let snapshot = state.cache.snapshot();
            match snapshot
                .metrics()
                .render(state.limits.response_body_utf8_bytes() as usize)
            {
                Ok(body) => response(StatusCode::OK, METRICS_CONTENT_TYPE, body),
                Err(_) => fixed_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    OPERATIONS_HEALTH_CONTENT_TYPE,
                    METRICS_UNAVAILABLE_BODY,
                    state.limits,
                ),
            }
        }
        _ => not_found(state.limits),
    }
}

fn not_found(limits: OperationsTransportLimits) -> Response<Full<Bytes>> {
    fixed_response(
        StatusCode::NOT_FOUND,
        OPERATIONS_HEALTH_CONTENT_TYPE,
        NOT_FOUND_BODY,
        limits,
    )
}

fn fixed_response(
    status: StatusCode,
    content_type: &'static str,
    body: &'static [u8],
    limits: OperationsTransportLimits,
) -> Response<Full<Bytes>> {
    if body.len() > limits.response_body_utf8_bytes() as usize {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            OPERATIONS_HEALTH_CONTENT_TYPE,
            Vec::new(),
        );
    }
    response(status, content_type, body.to_vec())
}

fn response(
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(CACHE_CONTROL, NO_STORE);
    response
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestHeadError {
    TooLarge,
    Incomplete,
    Read,
}

async fn read_request_head(
    stream: &mut TcpStream,
    maximum: usize,
) -> Result<Vec<u8>, RequestHeadError> {
    let allocation = maximum.checked_add(1).ok_or(RequestHeadError::TooLarge)?;
    let mut head = Vec::with_capacity(allocation);
    let mut scan_from = 0;
    loop {
        if let Some(index) = head[scan_from..]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        {
            let end = scan_from + index + 4;
            return if end <= maximum {
                Ok(head)
            } else {
                Err(RequestHeadError::TooLarge)
            };
        }
        if head.len() > maximum {
            return Err(RequestHeadError::TooLarge);
        }
        scan_from = head.len().saturating_sub(3);
        let remaining = allocation.saturating_sub(head.len());
        if remaining == 0 {
            return Err(RequestHeadError::TooLarge);
        }
        let mut chunk = [0_u8; 1024];
        let chunk_limit = remaining.min(chunk.len());
        let read = stream
            .read(&mut chunk[..chunk_limit])
            .await
            .map_err(|_| RequestHeadError::Read)?;
        if read == 0 {
            return Err(RequestHeadError::Incomplete);
        }
        head.extend_from_slice(&chunk[..read]);
    }
}

async fn write_header_limit_response(
    stream: &mut TcpStream,
    limits: OperationsTransportLimits,
) -> io::Result<()> {
    let body = if HEADERS_TOO_LARGE_BODY.len() <= limits.response_body_utf8_bytes() as usize {
        HEADERS_TOO_LARGE_BODY
    } else {
        &[]
    };
    let head = format!(
        concat!(
            "HTTP/1.1 431 Request Header Fields Too Large\r\n",
            "content-type: text/plain; charset=utf-8\r\n",
            "cache-control: no-store\r\n",
            "connection: close\r\n",
            "content-length: {}\r\n\r\n"
        ),
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.shutdown().await
}

struct PrefixedTcpStream {
    prefix: Vec<u8>,
    offset: usize,
    stream: TcpStream,
}

impl PrefixedTcpStream {
    fn new(prefix: Vec<u8>, stream: TcpStream) -> Self {
        Self {
            prefix,
            offset: 0,
            stream,
        }
    }
}

impl AsyncRead for PrefixedTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.offset < self.prefix.len() {
            let count = output
                .remaining()
                .min(self.prefix.len().saturating_sub(self.offset));
            output.put_slice(&self.prefix[self.offset..self.offset + count]);
            self.offset += count;
            if self.offset == self.prefix.len() {
                self.prefix = Vec::new();
                self.offset = 0;
            }
            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.stream).poll_read(context, output)
        }
    }
}

impl AsyncWrite for PrefixedTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.stream).poll_write(context, buffer)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::{
        CachedServiceState, CommonMetricGroup, MetricDescriptor, MetricKind, MetricLabel,
        MetricLabelKey, MetricName, MetricSample, MetricValue, MonotonicTime, OperationsBindPolicy,
        OperationsListenAddress, OperationsTransportLimitValues, Readiness, ReasonCodes,
        ServiceOperationalState, ServicePhase, cached_service_state,
    };

    fn limits() -> OperationsTransportLimits {
        OperationsTransportLimits::new(OperationsTransportLimitValues {
            header_count: 16,
            header_bytes: OPERATIONS_HTTP_MIN_HEADER_BYTES,
            response_body_utf8_bytes: 4096,
            concurrent_connections: 4,
            request_deadline: Duration::from_millis(200),
            idle_timeout: Duration::from_millis(200),
        })
        .unwrap()
    }

    fn snapshot(
        phase: ServicePhase,
        readiness: Readiness,
    ) -> CachedServiceStateReader<BoundedMetricsSnapshot> {
        let descriptor = MetricDescriptor::new(
            CommonMetricGroup::Phase,
            MetricName::new("radroots_phase").unwrap(),
            "current phase",
            MetricKind::Gauge,
            [MetricLabelKey::Phase],
        )
        .unwrap();
        let sample = MetricSample::new(
            MetricName::new("radroots_phase").unwrap(),
            MetricValue::Gauge(1),
            [MetricLabel::phase(phase)],
        )
        .unwrap();
        let metrics = BoundedMetricsSnapshot::new([descriptor], [sample]).unwrap();
        let operational =
            ServiceOperationalState::new(phase, readiness, ReasonCodes::empty()).unwrap();
        cached_service_state(CachedServiceState::new(operational, metrics)).1
    }

    fn config(address: SocketAddr, limits: OperationsTransportLimits) -> OperationsListenerConfig {
        OperationsListenerConfig::enabled(
            OperationsListenAddress::new(address).unwrap(),
            OperationsBindPolicy::LoopbackOnly,
            limits,
        )
        .unwrap()
    }

    async fn bound(limits: OperationsTransportLimits) -> BoundOperationsServer {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9);
        OperationsServer::new(
            config(address, limits),
            snapshot(ServicePhase::Ready, Readiness::READY),
        )
        .unwrap()
        .bind_with_ephemeral_port_for_test()
        .await
        .unwrap()
    }

    impl OperationsServer {
        async fn bind_with_ephemeral_port_for_test(
            self,
        ) -> Result<BoundOperationsServer, OperationsServerError> {
            let listener = TcpListener::bind(SocketAddr::new(self.listen.ip(), 0))
                .await
                .map_err(|error| OperationsServerError::Bind { kind: error.kind() })?;
            let local_address = listener
                .local_addr()
                .map_err(|error| OperationsServerError::LocalAddress { kind: error.kind() })?;
            Ok(BoundOperationsServer {
                listener,
                local_address,
                state: self.state,
            })
        }
    }

    async fn raw_request(address: SocketAddr, request: &[u8]) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_all(request).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        response
    }

    fn response_text(response: &[u8]) -> &str {
        std::str::from_utf8(response).unwrap()
    }

    fn padded_request_head(total_bytes: usize) -> Vec<u8> {
        const PREFIX: &[u8] = b"GET /livez HTTP/1.1\r\nx-pad: ";
        const SUFFIX: &[u8] = b"\r\n\r\n";
        assert!(total_bytes >= PREFIX.len() + SUFFIX.len());
        let mut request = Vec::with_capacity(total_bytes);
        request.extend_from_slice(PREFIX);
        request.resize(total_bytes - SUFFIX.len(), b'a');
        request.extend_from_slice(SUFFIX);
        assert_eq!(request.len(), total_bytes);
        request
    }

    #[tokio::test]
    async fn serves_only_exact_passive_routes_with_exact_content_types() {
        let server = bound(limits()).await;
        let address = server.local_address();
        let cancellation = CancellationToken::new();
        let serve_cancel = cancellation.clone();
        let task = tokio::spawn(server.serve(serve_cancel));

        let live = raw_request(address, b"GET /livez HTTP/1.1\r\nhost: localhost\r\n\r\n").await;
        let ready = raw_request(address, b"GET /readyz HTTP/1.1\r\nhost: localhost\r\n\r\n").await;
        let metrics =
            raw_request(address, b"GET /metrics HTTP/1.1\r\nhost: localhost\r\n\r\n").await;
        assert!(
            response_text(&live).starts_with("HTTP/1.1 200 OK\r\n"),
            "{}",
            response_text(&live)
        );
        assert!(response_text(&live).contains("content-type: text/plain; charset=utf-8\r\n"));
        assert!(response_text(&live).ends_with("live\n"));
        assert!(response_text(&ready).ends_with("ready\n"));
        assert!(
            response_text(&metrics)
                .contains("content-type: text/plain; version=0.0.4; charset=utf-8\r\n")
        );
        assert!(response_text(&metrics).contains("# TYPE radroots_phase gauge\n"));

        for request in [
            &b"GET /status HTTP/1.1\r\nhost: localhost\r\n\r\n"[..],
            &b"POST /readyz HTTP/1.1\r\nhost: localhost\r\ncontent-length: 0\r\n\r\n"[..],
            &b"GET /readyz?probe=1 HTTP/1.1\r\nhost: localhost\r\n\r\n"[..],
            &b"GET /v1/status HTTP/1.1\r\nhost: localhost\r\n\r\n"[..],
        ] {
            let rejected = raw_request(address, request).await;
            assert!(response_text(&rejected).starts_with("HTTP/1.1 404 Not Found\r\n"));
            assert!(response_text(&rejected).ends_with("not found\n"));
        }

        cancellation.cancel();
        assert_eq!(task.await.unwrap(), Ok(()));
    }

    #[tokio::test]
    async fn enforces_http_header_and_metrics_response_limits() {
        let mut values = limits().values();
        values.response_body_utf8_bytes = 32;
        let server = bound(OperationsTransportLimits::new(values).unwrap()).await;
        let address = server.local_address();
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(cancellation.clone()));

        let exact_head = padded_request_head(OPERATIONS_HTTP_MIN_HEADER_BYTES as usize);
        let exact = raw_request(address, &exact_head).await;
        assert!(response_text(&exact).starts_with("HTTP/1.1 200 OK\r\n"));
        let over_head = padded_request_head(OPERATIONS_HTTP_MIN_HEADER_BYTES as usize + 1);
        let headers = raw_request(address, &over_head).await;
        assert!(
            response_text(&headers).starts_with("HTTP/1.1 431 Request Header Fields Too Large\r\n"),
            "{}",
            response_text(&headers)
        );
        let metrics = raw_request(address, b"GET /metrics HTTP/1.1\r\nh: x\r\n\r\n").await;
        assert!(response_text(&metrics).starts_with("HTTP/1.1 503 Service Unavailable\r\n"));
        assert!(response_text(&metrics).ends_with("metrics unavailable\n"));
        let version = raw_request(address, b"GET /livez HTTP/1.0\r\n\r\n").await;
        assert!(
            response_text(&version).starts_with("HTTP/1.0 505 HTTP Version Not Supported\r\n"),
            "{}",
            response_text(&version)
        );

        cancellation.cancel();
        assert_eq!(task.await.unwrap(), Ok(()));
    }

    #[tokio::test]
    async fn bind_failure_and_disabled_configuration_are_typed() {
        assert_eq!(
            OperationsServer::new(
                OperationsListenerConfig::disabled(),
                snapshot(ServicePhase::Ready, Readiness::READY),
            )
            .unwrap_err(),
            OperationsServerError::Disabled
        );

        let mut below_floor = limits().values();
        below_floor.header_bytes = OPERATIONS_HTTP_MIN_HEADER_BYTES - 1;
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9);
        assert_eq!(
            OperationsServer::new(
                config(
                    address,
                    OperationsTransportLimits::new(below_floor).unwrap()
                ),
                snapshot(ServicePhase::Ready, Readiness::READY),
            )
            .unwrap_err(),
            OperationsServerError::HeaderLimitBelowParserFloor
        );

        let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = occupied.local_addr().unwrap();
        let error = OperationsServer::new(
            config(address, limits()),
            snapshot(ServicePhase::Ready, Readiness::READY),
        )
        .unwrap()
        .bind()
        .await
        .unwrap_err();
        assert_eq!(
            error,
            OperationsServerError::Bind {
                kind: io::ErrorKind::AddrInUse
            }
        );
    }

    #[tokio::test]
    async fn cancellation_stops_admission_and_drains_partial_connections() {
        let mut values = limits().values();
        values.request_deadline = Duration::from_millis(20);
        values.idle_timeout = Duration::from_millis(20);
        let server = bound(OperationsTransportLimits::new(values).unwrap()).await;
        let address = server.local_address();
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(cancellation.clone()));
        let mut partial = TcpStream::connect(address).await.unwrap();
        partial.write_all(b"GET /livez HTTP/1.1\r\n").await.unwrap();

        cancellation.cancel();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap(),
            Ok(())
        );
        assert!(TcpStream::connect(address).await.is_err());
    }

    #[tokio::test]
    async fn connection_saturation_sheds_and_recovers_without_queueing() {
        let mut values = limits().values();
        values.concurrent_connections = 1;
        values.request_deadline = Duration::from_millis(500);
        values.idle_timeout = Duration::from_millis(500);
        let server = bound(OperationsTransportLimits::new(values).unwrap()).await;
        let address = server.local_address();
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(cancellation.clone()));

        let mut occupied = TcpStream::connect(address).await.unwrap();
        occupied
            .write_all(b"GET /livez HTTP/1.1\r\n")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;

        let mut shed_stream = TcpStream::connect(address).await.unwrap();
        shed_stream
            .write_all(b"GET /livez HTTP/1.1\r\nhost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut shed = Vec::new();
        let shed_result = shed_stream.read_to_end(&mut shed).await;
        assert!(shed.is_empty());
        assert!(
            shed_result.is_ok()
                || shed_result.is_err_and(|error| error.kind() == io::ErrorKind::ConnectionReset)
        );
        drop(occupied);
        tokio::time::sleep(Duration::from_millis(20)).await;

        let recovered =
            raw_request(address, b"GET /livez HTTP/1.1\r\nhost: localhost\r\n\r\n").await;
        assert!(response_text(&recovered).starts_with("HTTP/1.1 200 OK\r\n"));

        cancellation.cancel();
        assert_eq!(task.await.unwrap(), Ok(()));
    }

    struct PostRenderDeadlineClock {
        calls: AtomicUsize,
    }

    impl MonotonicClock for PostRenderDeadlineClock {
        fn now_monotonic(&self) -> MonotonicTime {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let elapsed = if call < 2 {
                Duration::ZERO
            } else {
                Duration::from_millis(2)
            };
            MonotonicTime::from_duration_since_origin(elapsed)
        }
    }

    #[tokio::test]
    async fn synchronous_render_cannot_return_success_after_request_deadline() {
        let mut values = limits().values();
        values.request_deadline = Duration::from_millis(1);
        let limits = OperationsTransportLimits::new(values).unwrap();
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9);
        let server = OperationsServer::new_with_clock(
            config(address, limits),
            snapshot(ServicePhase::Ready, Readiness::READY),
            PostRenderDeadlineClock {
                calls: AtomicUsize::new(0),
            },
        )
        .unwrap()
        .bind_with_ephemeral_port_for_test()
        .await
        .unwrap();
        let address = server.local_address();
        let cancellation = CancellationToken::new();
        let task = tokio::spawn(server.serve(cancellation.clone()));

        let response =
            raw_request(address, b"GET /metrics HTTP/1.1\r\nhost: localhost\r\n\r\n").await;
        assert!(response_text(&response).starts_with("HTTP/1.1 504 Gateway Timeout\r\n"));
        assert!(response_text(&response).ends_with("request timeout\n"));

        cancellation.cancel();
        assert_eq!(task.await.unwrap(), Ok(()));
    }
}
