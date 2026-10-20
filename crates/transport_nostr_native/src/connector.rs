use crate::{socket::NativeSocket, wire::MeteredIo};
use core::{future::Future, pin::pin, task::Poll};
use futures::StreamExt;
use radroots_transport::host::websocket::{Admission, Connector, Error, IoFuture, Socket};
use std::{net::SocketAddr, num::NonZeroUsize, sync::Arc, time::Duration};
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use url::Url;

const MAX_RESOLVED_ADDRESSES: usize = 32;

/// Inert native I/O provider. The shared session supplies its original
/// destination authority, ingress generation, limits and connection deadline.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeConnector;

impl Connector for NativeConnector {
    fn supports_ping(&self) -> bool {
        true
    }

    fn connect<'a>(
        &'a self,
        endpoint: &'a str,
        timeout: Duration,
        max_message_bytes: NonZeroUsize,
        admission: Arc<dyn Admission>,
    ) -> IoFuture<'a, Result<Arc<dyn Socket>, Error>> {
        Box::pin(async move {
            let parsed = Url::parse(endpoint).map_err(|_| Error::Policy)?;
            if !matches!(parsed.scheme(), "ws" | "wss")
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.fragment().is_some()
            {
                return Err(Error::Policy);
            }
            let host = parsed
                .host_str()
                .ok_or(Error::Policy)?
                .trim_matches(['[', ']']);
            let port = parsed.port_or_known_default().ok_or(Error::Policy)?;
            let metering = Arc::clone(&admission);
            let connect = async {
                let addresses = resolve_bounded(host, port).await?;
                if addresses
                    .iter()
                    .any(|address| !admission.permits_address(address.ip()))
                {
                    return Err(Error::Policy);
                }
                let tcp = connect_pinned(&addresses).await?;
                let plaintext = if parsed.scheme() == "wss" {
                    let mut roots = rustls::RootCertStore::empty();
                    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                    let config = rustls::ClientConfig::builder()
                        .with_root_certificates(roots)
                        .with_no_client_auth();
                    let server_name = rustls::pki_types::ServerName::try_from(host.to_owned())
                        .map_err(|_| Error::Policy)?;
                    let tls = tokio_rustls::TlsConnector::from(Arc::new(config))
                        .connect(server_name, tcp)
                        .await
                        .map_err(|_| Error::Unavailable)?;
                    MaybeTlsStream::Rustls(tls)
                } else {
                    MaybeTlsStream::Plain(tcp)
                };
                let metered = MeteredIo::new(plaintext, metering, max_message_bytes.get());
                let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
                    .max_message_size(Some(max_message_bytes.get()))
                    .max_frame_size(Some(max_message_bytes.get()));
                let (stream, _) =
                    tokio_tungstenite::client_async_with_config(endpoint, metered, Some(config))
                        .await
                        .map_err(|_| Error::Unavailable)?;
                let (sink, stream) = stream.split();
                Ok(
                    Arc::new(NativeSocket::new(sink, stream, max_message_bytes.get()))
                        as Arc<dyn Socket>,
                )
            };
            let mut connect = pin!(connect);
            let admitted = futures::future::poll_fn(|context| {
                if !admission.admitted_read(context) || !admission.admitted_write(context) {
                    return Poll::Ready(Err(Error::Policy));
                }
                connect.as_mut().poll(context)
            });
            tokio::time::timeout(timeout, admitted)
                .await
                .map_err(|_| Error::Timeout)?
        })
    }
}

async fn resolve_bounded(host: &str, port: u16) -> Result<Vec<SocketAddr>, Error> {
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| Error::Unavailable)?;
    let bounded = addresses
        .take(MAX_RESOLVED_ADDRESSES + 1)
        .collect::<Vec<_>>();
    if bounded.is_empty() || bounded.len() > MAX_RESOLVED_ADDRESSES {
        return Err(Error::Policy);
    }
    Ok(bounded)
}

async fn connect_pinned(addresses: &[SocketAddr]) -> Result<TcpStream, Error> {
    for address in addresses {
        if let Ok(stream) = TcpStream::connect(address).await {
            return Ok(stream);
        }
    }
    Err(Error::Unavailable)
}
