use futures::{FutureExt, SinkExt, StreamExt, task::AtomicWaker};
use radroots_transport::host::websocket::{Admission, Connector, Error, Frame, Socket};
use radroots_transport_nostr_native::NativeConnector;
use std::{
    net::IpAddr,
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Context,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const WATCHDOG: Duration = Duration::from_secs(5);

struct Permit {
    open: AtomicBool,
    addresses: bool,
    bytes: AtomicUsize,
    limit: AtomicUsize,
    frames: AtomicUsize,
    reads: AtomicUsize,
    reader_seen: tokio::sync::Notify,
    reader: AtomicWaker,
    writer: AtomicWaker,
}

impl Permit {
    fn new(addresses: bool) -> Arc<Self> {
        Arc::new(Self {
            open: AtomicBool::new(true),
            addresses,
            bytes: AtomicUsize::new(0),
            limit: AtomicUsize::new(usize::MAX),
            frames: AtomicUsize::new(0),
            reads: AtomicUsize::new(0),
            reader_seen: tokio::sync::Notify::new(),
            reader: AtomicWaker::new(),
            writer: AtomicWaker::new(),
        })
    }
    fn revoke(&self) {
        self.open.store(false, Ordering::SeqCst);
        self.reader.wake();
        self.writer.wake();
    }
}

impl Admission for Permit {
    fn permits_address(&self, address: IpAddr) -> bool {
        self.addresses && address.is_loopback()
    }
    fn admitted_read(&self, context: &Context<'_>) -> bool {
        self.reader.register(context.waker());
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.reader_seen.notify_waiters();
        self.open.load(Ordering::SeqCst)
    }
    fn admitted_write(&self, context: &Context<'_>) -> bool {
        self.writer.register(context.waker());
        self.open.load(Ordering::SeqCst)
    }
    fn charge(&self, bytes: usize, frames: usize, _: usize) -> bool {
        let prior = self.bytes.fetch_add(bytes, Ordering::SeqCst);
        self.frames.fetch_add(frames, Ordering::SeqCst);
        prior
            .checked_add(bytes)
            .is_some_and(|total| total <= self.limit.load(Ordering::SeqCst))
    }
}

async fn connect(url: &str, permit: Arc<Permit>, limit: usize) -> Arc<dyn Socket> {
    NativeConnector
        .connect(
            url,
            Duration::from_secs(1),
            NonZeroUsize::new(limit).unwrap(),
            permit,
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn construction_unpolled_connection_and_denied_authority_open_no_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let permit = Permit::new(true);
    let future = NativeConnector.connect(
        &url,
        Duration::from_secs(1),
        NonZeroUsize::new(1024).unwrap(),
        permit.clone(),
    );
    drop(future);
    assert!(listener.accept().now_or_never().is_none());
    permit.revoke();
    assert_eq!(
        NativeConnector
            .connect(
                &url,
                Duration::from_secs(1),
                NonZeroUsize::new(1024).unwrap(),
                permit
            )
            .await
            .err(),
        Some(Error::Policy)
    );
    assert_eq!(
        NativeConnector
            .connect(
                &url,
                Duration::from_secs(1),
                NonZeroUsize::new(1024).unwrap(),
                Permit::new(false)
            )
            .await
            .err(),
        Some(Error::Policy)
    );
    assert!(listener.accept().now_or_never().is_none());
    for url in [
        "https://127.0.0.1",
        "ws://user:secret@127.0.0.1",
        "ws://127.0.0.1/#fragment",
        "invalid",
    ] {
        assert_eq!(
            NativeConnector
                .connect(
                    url,
                    Duration::from_secs(1),
                    NonZeroUsize::new(1024).unwrap(),
                    Permit::new(true)
                )
                .await
                .err(),
            Some(Error::Policy)
        );
    }
}

#[tokio::test]
async fn original_text_binary_and_control_frames_share_one_socket_then_close() {
    tokio::time::timeout(WATCHDOG, async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let original = " \n[\"EVENT\", { \"z\":1, \"a\":2 }] \t";
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(tcp).await.unwrap();
            assert_eq!(
                socket.next().await.unwrap().unwrap(),
                Message::Text(original.into())
            );
            socket.send(Message::Text(original.into())).await.unwrap();
            let binary = vec![0, 255, 1];
            assert_eq!(
                socket.next().await.unwrap().unwrap(),
                Message::Binary(binary.clone().into())
            );
            socket.send(Message::Binary(binary.into())).await.unwrap();
            assert_eq!(
                socket.next().await.unwrap().unwrap(),
                Message::Ping(vec![7].into())
            );
            socket.flush().await.unwrap();
            assert_eq!(
                socket.next().await.unwrap().unwrap(),
                Message::Pong(vec![9].into())
            );
            assert!(matches!(
                socket.next().await.unwrap().unwrap(),
                Message::Close(_)
            ));
            socket.flush().await.unwrap();
        });
        let permit = Permit::new(true);
        let socket = connect(&url, permit.clone(), 1024).await;
        assert!(NativeConnector.supports_ping());
        socket.send(Frame::Text(original.to_owned())).await.unwrap();
        assert_eq!(
            socket.receive().await.unwrap(),
            Some(Frame::Text(original.to_owned()))
        );
        socket.send(Frame::Binary(vec![0, 255, 1])).await.unwrap();
        assert_eq!(
            socket.receive().await.unwrap(),
            Some(Frame::Binary(vec![0, 255, 1]))
        );
        socket.send(Frame::Ping(vec![7])).await.unwrap();
        assert_eq!(socket.receive().await.unwrap(), Some(Frame::Pong(vec![7])));
        socket.send(Frame::Pong(vec![9])).await.unwrap();
        assert!(permit.frames.load(Ordering::SeqCst) >= 3);
        socket.close().await.unwrap();
        server.await.unwrap();
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn secure_endpoint_sends_tls_and_refuses_a_plaintext_upgrade() {
    tokio::time::timeout(WATCHDOG, async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("wss://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut prefix = [0; 5];
            socket.read_exact(&mut prefix).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 101 Switching Protocols\r\n\r\n")
                .await
                .unwrap();
            prefix
        });
        assert_eq!(
            NativeConnector
                .connect(
                    &url,
                    Duration::from_secs(1),
                    NonZeroUsize::new(1024).unwrap(),
                    Permit::new(true)
                )
                .await
                .err(),
            Some(Error::Unavailable)
        );
        let prefix = server.await.unwrap();
        assert_eq!(&prefix[..2], &[0x16, 0x03]);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn handshake_deadline_disposes_the_original_tcp_connection() {
    tokio::time::timeout(WATCHDOG, async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 4096];
            loop {
                if socket.read(&mut bytes).await.unwrap() == 0 {
                    break;
                }
            }
        });
        assert_eq!(
            NativeConnector
                .connect(
                    &url,
                    Duration::from_millis(50),
                    NonZeroUsize::new(1024).unwrap(),
                    Permit::new(true)
                )
                .await
                .err(),
            Some(Error::Timeout)
        );
        server.await.unwrap();
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn revocation_wakes_a_pending_read_and_prevents_later_writes() {
    tokio::time::timeout(WATCHDOG, async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(tcp).await.unwrap();
            while let Some(message) = socket.next().await {
                if message.is_err() {
                    break;
                }
            }
        });
        let permit = Permit::new(true);
        let socket = connect(&url, permit.clone(), 1024).await;
        let before = permit.reads.load(Ordering::SeqCst);
        let reader = socket.clone();
        let receive = tokio::spawn(async move { reader.receive().await });
        loop {
            let observed = permit.reader_seen.notified();
            tokio::pin!(observed);
            observed.as_mut().enable();
            if permit.reads.load(Ordering::SeqCst) > before {
                break;
            }
            observed.await;
        }
        permit.revoke();
        assert_eq!(receive.await.unwrap(), Err(Error::Unavailable));
        assert_eq!(
            socket.send(Frame::Text("denied".to_owned())).await,
            Err(Error::Unavailable)
        );
        drop(socket);
        server.await.unwrap();
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn oversized_frames_and_refused_raw_bytes_never_reach_the_message_reader() {
    for raw_budget in [false, true] {
        tokio::time::timeout(WATCHDOG, async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("ws://{}", listener.local_addr().unwrap());
            let (ready, wait) = oneshot::channel();
            let server = tokio::spawn(async move {
                let (tcp, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(tcp).await.unwrap();
                wait.await.unwrap();
                socket
                    .send(Message::Binary(vec![1; 128].into()))
                    .await
                    .unwrap();
            });
            let permit = Permit::new(true);
            let socket = connect(&url, permit.clone(), if raw_budget { 1024 } else { 64 }).await;
            if raw_budget {
                permit
                    .limit
                    .store(permit.bytes.load(Ordering::SeqCst), Ordering::SeqCst);
            }
            ready.send(()).unwrap();
            assert_eq!(socket.receive().await, Err(Error::Unavailable));
            drop(socket);
            server.await.unwrap();
        })
        .await
        .unwrap();
    }
}
