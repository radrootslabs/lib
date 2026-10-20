//! One upstream protocol session over caller-selected byte I/O.

use crate::{RelayEndpoint, RelayUrl, RelayUrlPolicy, relay::policy_error};
use async_wsocket::Message;
use futures::{Sink, future::BoxFuture, stream};
use nostr_sdk::{
    error::Error as TransportError,
    transport::websocket::{WebSocketSink, WebSocketStream, WebSocketTransport},
};
use radroots_transport::host::websocket::{
    Admission, Connector, Error as HostError, Frame, Socket,
};
use std::{
    collections::BTreeMap,
    fmt,
    net::{IpAddr, SocketAddr},
    num::NonZeroUsize,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use url::Url;

pub(crate) const MAX_WIRE_MESSAGE_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct SessionWebsocketTransport {
    host: Arc<dyn Connector>,
    policies: Arc<BTreeMap<String, RelayUrlPolicy>>,
    connect_timeout: Duration,
    pub(crate) writers: crate::socket_write::WriterRegistry,
    pub(crate) ingress: crate::source_ingress::IngressRegistry,
}

impl SessionWebsocketTransport {
    pub(crate) fn new(
        endpoints: &[RelayEndpoint],
        connect_timeout: Duration,
        host: Arc<dyn Connector>,
    ) -> Self {
        Self {
            host,
            connect_timeout,
            ingress: crate::source_ingress::IngressRegistry::new(
                endpoints
                    .iter()
                    .map(|endpoint| endpoint.url().as_str().to_owned()),
            ),
            writers: crate::socket_write::WriterRegistry::new(
                endpoints
                    .iter()
                    .map(|endpoint| endpoint.url().as_str().to_owned()),
            ),
            policies: Arc::new(
                endpoints
                    .iter()
                    .map(|endpoint| (endpoint.url().as_str().to_owned(), endpoint.policy()))
                    .collect(),
            ),
        }
    }
}

impl WebSocketTransport for SessionWebsocketTransport {
    fn support_ping(&self) -> bool {
        self.host.supports_ping()
    }

    fn connect<'a>(
        &'a self,
        url: &'a Url,
        proxy: Option<SocketAddr>,
    ) -> BoxFuture<'a, Result<(WebSocketSink, WebSocketStream), TransportError>> {
        Box::pin(async move {
            if proxy.is_some() {
                return Err(policy_error(
                    "proxy and Tor connection modes are not configured",
                ));
            }
            let policy = self
                .policies
                .get(configured_policy_key(url))
                .copied()
                .ok_or_else(|| policy_error("relay URL is not configured"))?;
            let relay = RelayUrl::parse(url.as_str(), policy)
                .map_err(|_| policy_error("relay URL is denied by network policy"))?;
            let ingress = self
                .ingress
                .connection(relay.as_str())
                .ok_or_else(|| policy_error("relay ingress unavailable"))?;
            let admission = Arc::new(ConnectionAdmission { policy, ingress });
            let limit = NonZeroUsize::new(MAX_WIRE_MESSAGE_BYTES)
                .expect("the fixed wire limit is positive");
            let socket = crate::host_time::timeout(
                self.connect_timeout,
                self.host
                    .connect(relay.as_str(), self.connect_timeout, limit, admission),
            )
            .await
            .map_err(|_| policy_error("relay connection deadline elapsed"))?
            .map_err(host_error)?;
            let writer =
                crate::socket_write::SocketWriter::new(Box::pin(HostSink::new(socket.clone())));
            self.writers.install(relay.as_str(), &writer)?;
            let sink: WebSocketSink = Box::pin(crate::socket_write::SharedSocketSink::new(writer));
            let receiver = stream::try_unfold(socket, |socket| async move {
                let frame = socket.receive().await.map_err(host_error)?;
                let message = frame.map(from_host_frame).transpose()?.flatten();
                Ok(message.map(|message| (message, socket)))
            });
            Ok((sink, Box::pin(receiver) as WebSocketStream))
        })
    }
}

struct ConnectionAdmission {
    policy: RelayUrlPolicy,
    ingress: crate::source_ingress::IngressConnection,
}

impl Admission for ConnectionAdmission {
    fn permits_address(&self, address: IpAddr) -> bool {
        self.policy.accepts_address(address)
    }
    fn admitted_read(&self, context: &Context<'_>) -> bool {
        self.ingress.admitted(context)
    }
    fn admitted_write(&self, context: &Context<'_>) -> bool {
        self.ingress.admitted_write(context)
    }
    fn charge(&self, bytes: usize, frames: usize, data: usize) -> bool {
        self.ingress.charge(bytes, frames, data)
    }
}

fn configured_policy_key(url: &Url) -> &str {
    if url.path() == "/" && url.query().is_none() && url.fragment().is_none() {
        url.as_str().strip_suffix('/').unwrap_or(url.as_str())
    } else {
        url.as_str()
    }
}

fn host_error(error: HostError) -> TransportError {
    match error {
        HostError::Policy => policy_error("relay socket policy rejected"),
        HostError::Timeout => TransportError::with_static_message(
            nostr_sdk::error::ErrorKind::Timeout,
            "relay connection timed out",
        ),
        other => TransportError::transport(other),
    }
}

fn from_host_frame(frame: Frame) -> Result<Option<Message>, TransportError> {
    Ok(Some(match frame {
        Frame::Text(text) => {
            validate_wire_identifiers(&text)?;
            Message::Text(text)
        }
        Frame::Binary(bytes) => {
            if bytes.len() > MAX_WIRE_MESSAGE_BYTES {
                return Err(policy_error("relay message exceeds the wire limit"));
            }
            // Preserve the original host bytes. The upstream Nostr session
            // explicitly ignores binary messages; never promote them to text.
            Message::Binary(bytes)
        }
        #[cfg(not(target_arch = "wasm32"))]
        Frame::Ping(bytes) => Message::Ping(bytes),
        #[cfg(not(target_arch = "wasm32"))]
        Frame::Pong(bytes) => Message::Pong(bytes),
        #[cfg(not(target_arch = "wasm32"))]
        Frame::Close(frame) => {
            Message::Close(frame.map(|frame| async_wsocket::message::CloseFrame {
                code: frame.code,
                reason: frame.reason,
            }))
        }
        #[cfg(target_arch = "wasm32")]
        Frame::Close(_) => return Ok(None),
        _ => return Err(policy_error("unsupported host frame")),
    }))
}

fn validate_wire_identifiers(text: &str) -> Result<(), TransportError> {
    if text.len() > MAX_WIRE_MESSAGE_BYTES {
        return Err(policy_error("relay message exceeds the wire limit"));
    }
    // Inspect original identifiers before the upstream fixed-array decoder.
    // Never normalize or reserialize the accepted original message.
    let Ok(serde_json::Value::Array(fields)) = serde_json::from_str(text) else {
        return Ok(());
    };
    let canonical_hex = |value: Option<&serde_json::Value>, length| {
        value
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| {
                value.len() == length
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            })
    };
    let valid = match fields.first().and_then(serde_json::Value::as_str) {
        Some("OK") => canonical_hex(fields.get(1), 64),
        Some("EVENT") => fields.get(2).is_some_and(|event| {
            canonical_hex(event.get("id"), 64)
                && canonical_hex(event.get("pubkey"), 64)
                && canonical_hex(event.get("sig"), 128)
        }),
        _ => true,
    };
    if !valid {
        return Err(policy_error("relay message has a noncanonical identifier"));
    }
    Ok(())
}

fn to_host_frame(message: Message) -> Frame {
    match message {
        Message::Text(text) => Frame::Text(text),
        Message::Binary(bytes) => Frame::Binary(bytes),
        #[cfg(not(target_arch = "wasm32"))]
        Message::Ping(bytes) => Frame::Ping(bytes),
        #[cfg(not(target_arch = "wasm32"))]
        Message::Pong(bytes) => Frame::Pong(bytes),
        #[cfg(not(target_arch = "wasm32"))]
        Message::Close(frame) => {
            Frame::Close(
                frame.map(|frame| radroots_transport::host::websocket::CloseFrame {
                    code: frame.code,
                    reason: frame.reason,
                }),
            )
        }
    }
}

struct HostSink {
    socket: Arc<dyn Socket>,
    pending: Option<BoxFuture<'static, Result<(), TransportError>>>,
    closing: bool,
}

impl HostSink {
    fn new(socket: Arc<dyn Socket>) -> Self {
        Self {
            socket,
            pending: None,
            closing: false,
        }
    }
    fn poll_pending(&mut self, context: &mut Context<'_>) -> Poll<Result<(), TransportError>> {
        if let Some(pending) = &mut self.pending {
            let result = futures::ready!(pending.as_mut().poll(context));
            self.pending = None;
            return Poll::Ready(result);
        }
        Poll::Ready(Ok(()))
    }
}

impl Sink<Message> for HostSink {
    type Error = TransportError;
    fn poll_ready(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>> {
        if self.closing {
            return Poll::Ready(Err(policy_error("host socket is closing")));
        }
        self.poll_pending(context)
    }
    fn start_send(mut self: Pin<&mut Self>, message: Message) -> Result<(), TransportError> {
        if self.closing || self.pending.is_some() {
            return Err(policy_error("host socket is not ready"));
        }
        let socket = self.socket.clone();
        let frame = to_host_frame(message);
        self.pending = Some(Box::pin(async move {
            socket.send(frame).await.map_err(host_error)
        }));
        Ok(())
    }
    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>> {
        self.poll_pending(context)
    }
    fn poll_close(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>> {
        futures::ready!(self.poll_pending(context))?;
        if !self.closing {
            self.closing = true;
            let socket = self.socket.clone();
            self.pending = Some(Box::pin(
                async move { socket.close().await.map_err(host_error) },
            ));
        }
        self.poll_pending(context)
    }
}

impl fmt::Debug for ConnectionAdmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ConnectionAdmission([redacted])")
    }
}

#[cfg(test)]
#[path = "session_socket_tests.rs"]
mod tests;
