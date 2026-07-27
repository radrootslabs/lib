#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use radroots_nostr::prelude::RadrootsNostrClient;

#[cfg(target_arch = "wasm32")]
pub(crate) fn request_scoped_nostr_client() -> RadrootsNostrClient {
    RadrootsNostrClient::new_signerless()
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use async_wsocket::{ConnectionMode, Message};
    use futures::{Sink, StreamExt};
    use nostr_relay_pool::transport::error::TransportError;
    use nostr_relay_pool::transport::websocket::{
        WebSocketSink, WebSocketStream, WebSocketTransport,
    };
    use radroots_nostr::prelude::RadrootsNostrClient;
    use std::collections::BTreeSet;
    use std::fmt;
    use std::future::Future;
    use std::net::{IpAddr, SocketAddr};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use std::time::Duration;
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream as TokioWebSocketStream};

    use crate::{RadrootsRelayUrl, RadrootsRelayUrlPolicy};

    type ResolveFuture<'a> =
        Pin<Box<dyn Future<Output = Result<Vec<SocketAddr>, PinnedConnectError>> + Send + 'a>>;
    type NativeWebSocket = TokioWebSocketStream<MaybeTlsStream<TcpStream>>;

    trait RelayDnsResolver: fmt::Debug + Send + Sync {
        fn resolve<'a>(&'a self, host: &'a str, port: u16) -> ResolveFuture<'a>;
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct SystemRelayDnsResolver;

    impl RelayDnsResolver for SystemRelayDnsResolver {
        fn resolve<'a>(&'a self, host: &'a str, port: u16) -> ResolveFuture<'a> {
            Box::pin(async move {
                tokio::net::lookup_host((host, port))
                    .await
                    .map(|addresses| addresses.collect())
                    .map_err(|_| PinnedConnectError::ResolutionFailed)
            })
        }
    }

    #[derive(Clone, Debug)]
    struct PinnedWebsocketTransport<R> {
        resolver: Arc<R>,
    }

    impl<R> PinnedWebsocketTransport<R> {
        fn new(resolver: R) -> Self {
            Self {
                resolver: Arc::new(resolver),
            }
        }
    }

    impl<R> WebSocketTransport for PinnedWebsocketTransport<R>
    where
        R: RelayDnsResolver + 'static,
    {
        fn support_ping(&self) -> bool {
            true
        }

        fn connect<'a>(
            &'a self,
            url: &'a nostr::Url,
            mode: &'a ConnectionMode,
            timeout: Duration,
        ) -> nostr::util::BoxedFuture<'a, Result<(WebSocketSink, WebSocketStream), TransportError>>
        {
            Box::pin(async move {
                if !matches!(mode, ConnectionMode::Direct) {
                    return Err(TransportError::backend(
                        PinnedConnectError::ProxyModeForbidden,
                    ));
                }
                let socket = tokio::time::timeout(timeout, self.connect_pinned(url))
                    .await
                    .map_err(|_| TransportError::backend(PinnedConnectError::DeadlineExceeded))?
                    .map_err(TransportError::backend)?;
                let (sink, stream) = socket.split();
                let sink: WebSocketSink = Box::new(PinnedSink(sink));
                let stream: WebSocketStream = Box::pin(stream.map(|message| {
                    message
                        .map_err(TransportError::backend)
                        .and_then(native_message)
                }));
                Ok((sink, stream))
            })
        }
    }

    impl<R> PinnedWebsocketTransport<R>
    where
        R: RelayDnsResolver,
    {
        async fn connect_pinned(
            &self,
            url: &nostr::Url,
        ) -> Result<NativeWebSocket, PinnedConnectError> {
            let addresses = self.resolve_and_validate(url).await?;
            let mut connected = false;
            for address in addresses {
                let stream = match TcpStream::connect(address).await {
                    Ok(stream) => stream,
                    Err(_) => continue,
                };
                connected = true;
                if let Ok((socket, response)) =
                    tokio_tungstenite::client_async_tls(url.as_str(), stream).await
                {
                    if response.status().as_u16() == 101 {
                        return Ok(socket);
                    }
                    return Err(PinnedConnectError::RedirectOrHandshakeRejected);
                }
            }
            if connected {
                Err(PinnedConnectError::TlsValidationFailed)
            } else {
                Err(PinnedConnectError::ConnectionFailed)
            }
        }

        async fn resolve_and_validate(
            &self,
            url: &nostr::Url,
        ) -> Result<Vec<SocketAddr>, PinnedConnectError> {
            let host = url.host_str().ok_or(PinnedConnectError::InvalidRelayUrl)?;
            let port = url
                .port_or_known_default()
                .ok_or(PinnedConnectError::InvalidRelayUrl)?;
            let is_exact_loopback_host = matches!(host, "localhost" | "127.0.0.1" | "::1");
            let policy = if is_exact_loopback_host {
                RadrootsRelayUrlPolicy::Localhost
            } else {
                RadrootsRelayUrlPolicy::Public
            };
            let relay = RadrootsRelayUrl::parse(url.as_str(), policy)
                .map_err(|_| PinnedConnectError::InvalidRelayUrl)?;
            let addresses = self.resolver.resolve(host, port).await?;
            if addresses.is_empty() {
                return Err(PinnedConnectError::ResolutionReturnedNoAddresses);
            }
            if addresses.iter().any(|address| address.port() != port) {
                return Err(PinnedConnectError::DestinationForbidden);
            }
            if is_exact_loopback_host {
                if addresses
                    .iter()
                    .any(|address| !is_exact_loopback(address.ip()))
                {
                    return Err(PinnedConnectError::DestinationForbidden);
                }
            } else {
                relay
                    .validate_public_resolved_ip_addrs(addresses.iter().map(|address| address.ip()))
                    .map_err(|_| PinnedConnectError::DestinationForbidden)?;
            }
            Ok(addresses
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect())
        }
    }

    fn is_exact_loopback(address: IpAddr) -> bool {
        match address {
            IpAddr::V4(address) => address.is_loopback(),
            IpAddr::V6(address) => address.is_loopback(),
        }
    }

    struct PinnedSink(futures::stream::SplitSink<NativeWebSocket, TungsteniteMessage>);

    impl Sink<Message> for PinnedSink {
        type Error = TransportError;

        fn poll_ready(
            mut self: Pin<&mut Self>,
            context: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Pin::new(&mut self.0)
                .poll_ready(context)
                .map_err(TransportError::backend)
        }

        fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            Pin::new(&mut self.0)
                .start_send(item.into())
                .map_err(TransportError::backend)
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            context: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Pin::new(&mut self.0)
                .poll_flush(context)
                .map_err(TransportError::backend)
        }

        fn poll_close(
            mut self: Pin<&mut Self>,
            context: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Pin::new(&mut self.0)
                .poll_close(context)
                .map_err(TransportError::backend)
        }
    }

    fn native_message(message: TungsteniteMessage) -> Result<Message, TransportError> {
        match message {
            TungsteniteMessage::Text(value) => Ok(Message::Text(value.to_string())),
            TungsteniteMessage::Binary(value) => Ok(Message::Binary(value.to_vec())),
            TungsteniteMessage::Ping(value) => Ok(Message::Ping(value.to_vec())),
            TungsteniteMessage::Pong(value) => Ok(Message::Pong(value.to_vec())),
            TungsteniteMessage::Close(value) => Ok(Message::Close(value.map(|frame| {
                async_wsocket::message::CloseFrame {
                    code: frame.code.into(),
                    reason: frame.reason.to_string(),
                }
            }))),
            TungsteniteMessage::Frame(_) => Err(TransportError::backend(
                PinnedConnectError::UnexpectedRawFrame,
            )),
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum PinnedConnectError {
        InvalidRelayUrl,
        ProxyModeForbidden,
        ResolutionFailed,
        ResolutionReturnedNoAddresses,
        DestinationForbidden,
        ConnectionFailed,
        TlsValidationFailed,
        RedirectOrHandshakeRejected,
        DeadlineExceeded,
        UnexpectedRawFrame,
    }

    impl fmt::Display for PinnedConnectError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(match self {
                Self::InvalidRelayUrl => "relay URL rejected",
                Self::ProxyModeForbidden => "proxy connection mode rejected",
                Self::ResolutionFailed => "relay DNS resolution failed",
                Self::ResolutionReturnedNoAddresses => "relay DNS resolution returned no addresses",
                Self::DestinationForbidden => "relay destination rejected",
                Self::ConnectionFailed => "relay connection failed",
                Self::TlsValidationFailed => "relay TLS validation failed",
                Self::RedirectOrHandshakeRejected => "relay redirect or handshake rejected",
                Self::DeadlineExceeded => "relay connection deadline exceeded",
                Self::UnexpectedRawFrame => "relay returned an unexpected raw frame",
            })
        }
    }

    impl std::error::Error for PinnedConnectError {}

    pub(crate) fn request_scoped_nostr_client() -> RadrootsNostrClient {
        let inner = nostr_sdk::ClientBuilder::new()
            .websocket_transport(PinnedWebsocketTransport::new(SystemRelayDnsResolver))
            .build();
        RadrootsNostrClient::from_inner(inner)
    }

    #[cfg(test)]
    mod tests {
        use super::{
            PinnedConnectError, PinnedWebsocketTransport, RelayDnsResolver, ResolveFuture,
        };
        use async_wsocket::ConnectionMode;
        use nostr_relay_pool::transport::websocket::WebSocketTransport;
        use std::collections::VecDeque;
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
        use std::sync::Mutex;
        use std::time::Duration;

        #[derive(Debug)]
        struct ScriptedResolver {
            answers: Mutex<VecDeque<Vec<SocketAddr>>>,
        }

        impl ScriptedResolver {
            fn new(answers: impl IntoIterator<Item = Vec<SocketAddr>>) -> Self {
                Self {
                    answers: Mutex::new(answers.into_iter().collect()),
                }
            }
        }

        impl RelayDnsResolver for ScriptedResolver {
            fn resolve<'a>(&'a self, _host: &'a str, _port: u16) -> ResolveFuture<'a> {
                Box::pin(async move {
                    self.answers
                        .lock()
                        .expect("scripted resolver lock")
                        .pop_front()
                        .ok_or(PinnedConnectError::ResolutionFailed)
                })
            }
        }

        fn socket(address: IpAddr, port: u16) -> SocketAddr {
            SocketAddr::new(address, port)
        }

        #[tokio::test]
        async fn relay_security_resolution_rejects_mixed_special_and_rebinding_answers() {
            let public = socket(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)), 443);
            let private = socket(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 443);
            let metadata = socket(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)), 443);
            let mapped_private =
                socket(IpAddr::V6(Ipv4Addr::new(10, 0, 0, 1).to_ipv6_mapped()), 443);
            let transport = PinnedWebsocketTransport::new(ScriptedResolver::new([
                vec![public, public],
                vec![public, private],
                vec![metadata],
                vec![mapped_private],
                vec![private],
            ]));
            let url = nostr::Url::parse("wss://relay.example").unwrap();

            assert_eq!(
                transport.resolve_and_validate(&url).await.unwrap(),
                vec![public]
            );
            for _ in 0..4 {
                assert_eq!(
                    transport.resolve_and_validate(&url).await.unwrap_err(),
                    PinnedConnectError::DestinationForbidden
                );
            }
        }

        #[tokio::test]
        async fn relay_security_local_policy_requires_exact_loopback_resolution() {
            let loopback_v4 = socket(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
            let loopback_v6 = socket(IpAddr::V6(Ipv6Addr::LOCALHOST), 80);
            let public = socket(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)), 80);
            let transport = PinnedWebsocketTransport::new(ScriptedResolver::new([
                vec![loopback_v6, loopback_v4],
                vec![loopback_v4, public],
                Vec::new(),
            ]));
            let local = nostr::Url::parse("ws://localhost").unwrap();

            assert_eq!(
                transport.resolve_and_validate(&local).await.unwrap(),
                vec![loopback_v4, loopback_v6]
            );
            assert_eq!(
                transport.resolve_and_validate(&local).await.unwrap_err(),
                PinnedConnectError::DestinationForbidden
            );
            assert_eq!(
                transport.resolve_and_validate(&local).await.unwrap_err(),
                PinnedConnectError::ResolutionReturnedNoAddresses
            );
        }

        #[tokio::test]
        async fn relay_security_rejects_wrong_port_and_every_special_use_address_class() {
            let forbidden = [
                IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
                IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
                IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1)),
                IpAddr::V4(Ipv4Addr::BROADCAST),
                IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
                IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
                IpAddr::V4(Ipv4Addr::new(192, 0, 0, 8)),
                IpAddr::V4(Ipv4Addr::new(192, 88, 99, 2)),
                IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1)),
                IpAddr::V4(Ipv4Addr::new(240, 0, 0, 1)),
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                IpAddr::V6(Ipv6Addr::LOCALHOST),
                IpAddr::V6("ff02::1".parse().unwrap()),
                IpAddr::V6("fc00::1".parse().unwrap()),
                IpAddr::V6("fe80::1".parse().unwrap()),
                IpAddr::V6("64:ff9b::a00:1".parse().unwrap()),
                IpAddr::V6("2001:db8::1".parse().unwrap()),
                IpAddr::V6("2001::1".parse().unwrap()),
                IpAddr::V6("2002::1".parse().unwrap()),
                IpAddr::V6("3fff::1".parse().unwrap()),
                IpAddr::V6(Ipv4Addr::new(10, 0, 0, 1).to_ipv6_mapped()),
            ];
            let mut answers = Vec::with_capacity(forbidden.len() + 1);
            answers.push(vec![socket(
                IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
                8443,
            )]);
            answers.extend(
                forbidden
                    .into_iter()
                    .map(|address| vec![socket(address, 443)]),
            );
            let transport = PinnedWebsocketTransport::new(ScriptedResolver::new(answers));
            let url = nostr::Url::parse("wss://relay.example").unwrap();

            for _ in 0..24 {
                assert_eq!(
                    transport.resolve_and_validate(&url).await.unwrap_err(),
                    PinnedConnectError::DestinationForbidden
                );
            }
        }

        #[tokio::test]
        async fn relay_security_rejects_proxy_mode_before_resolution() {
            let transport = PinnedWebsocketTransport::new(ScriptedResolver::new([]));
            let url = nostr::Url::parse("wss://relay.example").unwrap();
            let mode = ConnectionMode::proxy(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 9050));

            let result =
                WebSocketTransport::connect(&transport, &url, &mode, Duration::from_millis(10))
                    .await;
            let error = match result {
                Ok(_) => panic!("proxy mode must be rejected"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("proxy connection mode rejected"));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::request_scoped_nostr_client;
