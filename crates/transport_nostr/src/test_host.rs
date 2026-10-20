//! Test-only loopback I/O for private shared-session lifecycle witnesses.
//! Charges complete frames for shared cleanup tests. Actual native policy, TLS
//! and predecode byte/frame budgets run in the native provider suite.

use futures::{
    SinkExt, StreamExt,
    lock::Mutex,
    stream::{SplitSink, SplitStream},
};
use radroots_transport::host::websocket::{Admission, Connector, Error, Frame, IoFuture, Socket};
use std::{num::NonZeroUsize, sync::Arc, time::Duration};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

#[derive(Debug)]
pub(crate) struct LoopbackConnector;

pub(crate) fn transport(config: crate::Config) -> crate::NostrTransport {
    crate::NostrTransport::new(config, Arc::new(LoopbackConnector))
}

impl Connector for LoopbackConnector {
    fn supports_ping(&self) -> bool {
        true
    }
    fn connect<'a>(
        &'a self,
        endpoint: &'a str,
        timeout: Duration,
        _: NonZeroUsize,
        admission: Arc<dyn Admission>,
    ) -> IoFuture<'a, Result<Arc<dyn Socket>, Error>> {
        Box::pin(async move {
            let url = url::Url::parse(endpoint).map_err(|_| Error::Policy)?;
            let ip = url
                .host_str()
                .and_then(|host| host.parse::<std::net::IpAddr>().ok())
                .ok_or(Error::Policy)?;
            if url.scheme() != "ws" || !ip.is_loopback() || !admission.permits_address(ip) {
                return Err(Error::Policy);
            }
            let (socket, _) =
                tokio::time::timeout(timeout, tokio_tungstenite::connect_async(endpoint))
                    .await
                    .map_err(|_| Error::Timeout)?
                    .map_err(|_| Error::Unavailable)?;
            let (sink, stream) = socket.split();
            Ok(Arc::new(LoopbackSocket {
                sink: Mutex::new(sink),
                stream: Mutex::new(stream),
                admission,
            }) as Arc<dyn Socket>)
        })
    }
}

type Wire = WebSocketStream<MaybeTlsStream<TcpStream>>;
struct LoopbackSocket {
    sink: Mutex<SplitSink<Wire, Message>>,
    stream: Mutex<SplitStream<Wire>>,
    admission: Arc<dyn Admission>,
}

impl Socket for LoopbackSocket {
    fn send(&self, frame: Frame) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            futures::future::poll_fn(|context| {
                std::task::Poll::Ready(if self.admission.admitted_write(context) {
                    Ok(())
                } else {
                    Err(Error::Closed)
                })
            })
            .await?;
            let message = match frame {
                Frame::Text(text) => Message::Text(text.into()),
                Frame::Binary(bytes) => Message::Binary(bytes.into()),
                Frame::Ping(bytes) => Message::Ping(bytes.into()),
                Frame::Pong(bytes) => Message::Pong(bytes.into()),
                Frame::Close(None) => Message::Close(None),
                _ => return Err(Error::Unsupported),
            };
            self.sink
                .lock()
                .await
                .send(message)
                .await
                .map_err(|_| Error::Unavailable)
        })
    }
    fn receive(&self) -> IoFuture<'_, Result<Option<Frame>, Error>> {
        Box::pin(async move {
            let mut stream = self.stream.lock().await;
            let message = futures::future::poll_fn(|context| {
                if !self.admission.admitted_read(context) {
                    return std::task::Poll::Ready(Some(Err(
                        tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                    )));
                }
                stream.poll_next_unpin(context)
            })
            .await
            .transpose()
            .map_err(|_| Error::Unavailable)?;
            message
                .map(|message| {
                    let data = matches!(message, Message::Text(_) | Message::Binary(_));
                    if !self.admission.charge(message.len(), 1, usize::from(data)) {
                        return Err(Error::Closed);
                    }
                    Ok(match message {
                        Message::Text(text) => Frame::Text(text.to_string()),
                        Message::Binary(bytes) => Frame::Binary(bytes.to_vec()),
                        Message::Ping(bytes) => Frame::Ping(bytes.to_vec()),
                        Message::Pong(bytes) => Frame::Pong(bytes.to_vec()),
                        Message::Close(_) => Frame::Close(None),
                        Message::Frame(_) => return Err(Error::Protocol),
                    })
                })
                .transpose()
        })
    }
    fn close(&self) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.sink
                .lock()
                .await
                .close()
                .await
                .map_err(|_| Error::Unavailable)
        })
    }
}
