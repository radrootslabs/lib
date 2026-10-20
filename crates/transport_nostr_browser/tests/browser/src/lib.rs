#![forbid(unsafe_code)]

mod clock;
mod shared;

use futures::{FutureExt, task::AtomicWaker};
use radroots_transport::host::websocket::{Admission, Connector, Error, Frame, Socket};
use radroots_transport_nostr_browser::BrowserConnector;
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
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = globalThis, js_name = waitForBrowserSocketClose)]
    fn wait_for_close(route: &str) -> js_sys::Promise;
    #[wasm_bindgen(js_namespace = globalThis, js_name = forceBrowserSocketBackpressure)]
    fn force_backpressure();
}

struct Grant {
    live: AtomicBool,
    charge: AtomicBool,
    bytes: AtomicUsize,
    frames: AtomicUsize,
    reader: AtomicWaker,
    writer: AtomicWaker,
}
impl Grant {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            live: AtomicBool::new(true),
            charge: AtomicBool::new(true),
            bytes: AtomicUsize::new(0),
            frames: AtomicUsize::new(0),
            reader: AtomicWaker::new(),
            writer: AtomicWaker::new(),
        })
    }
    fn revoke(&self) {
        self.live.store(false, Ordering::SeqCst);
        self.reader.wake();
        self.writer.wake();
    }
}
impl Admission for Grant {
    fn permits_address(&self, _: IpAddr) -> bool {
        self.live.load(Ordering::SeqCst)
    }
    fn admitted_read(&self, context: &Context<'_>) -> bool {
        self.reader.register(context.waker());
        self.live.load(Ordering::SeqCst)
    }
    fn admitted_write(&self, context: &Context<'_>) -> bool {
        self.writer.register(context.waker());
        self.live.load(Ordering::SeqCst)
    }
    fn charge(&self, bytes: usize, frames: usize, data: usize) -> bool {
        assert_eq!(frames, data);
        self.bytes.fetch_add(bytes, Ordering::SeqCst);
        self.frames.fetch_add(frames, Ordering::SeqCst);
        self.charge.load(Ordering::SeqCst) && self.live.load(Ordering::SeqCst)
    }
}
fn check(value: bool, name: &str) -> Result<(), JsValue> {
    if value {
        Ok(())
    } else {
        Err(JsValue::from_str(name))
    }
}
fn error(value: Error) -> JsValue {
    JsValue::from_str(&value.to_string())
}
async fn pause() {
    async_utility::time::sleep(Duration::from_millis(40)).await;
}
async fn connect(
    base: &str,
    route: &str,
    limit: usize,
    grant: Arc<Grant>,
) -> Result<Arc<dyn Socket>, JsValue> {
    BrowserConnector
        .connect(
            &format!("{base}/{route}"),
            Duration::from_secs(2),
            NonZeroUsize::new(limit).unwrap(),
            grant,
        )
        .await
        .map_err(error)
}

#[wasm_bindgen]
pub async fn verify_socket(base: String) -> Result<js_sys::Array, JsValue> {
    let passed = js_sys::Array::new();
    let grant = Grant::new();
    let endpoint = format!("{base}/unpolled");
    let future = BrowserConnector.connect(
        &endpoint,
        Duration::from_secs(1),
        NonZeroUsize::new(1024).unwrap(),
        grant.clone(),
    );
    drop(future);
    pause().await;
    passed.push(&"unpolled inert".into());

    let endpoint = format!("{base}/cancelled-owner");
    let mut attempt = BrowserConnector.connect(
        &endpoint,
        Duration::from_secs(1),
        NonZeroUsize::new(1).unwrap(),
        Grant::new(),
    );
    check(
        attempt.as_mut().now_or_never().is_none(),
        "owner initially scheduled",
    )?;
    drop(attempt);
    pause().await;
    passed.push(&"cancellation before owner startup".into());

    let endpoint = format!("{base}/revoked-owner");
    let revoked_grant = Grant::new();
    let mut attempt = BrowserConnector.connect(
        &endpoint,
        Duration::from_secs(1),
        NonZeroUsize::new(1).unwrap(),
        revoked_grant.clone(),
    );
    check(
        attempt.as_mut().now_or_never().is_none(),
        "revocable owner initially scheduled",
    )?;
    revoked_grant.revoke();
    check(
        matches!(attempt.await, Err(Error::Policy)),
        "owner rechecks original grant",
    )?;
    passed.push(&"revocation before owner startup".into());

    let endpoint = format!("{base}/expired-owner");
    let mut attempt = BrowserConnector.connect(
        &endpoint,
        Duration::from_millis(5),
        NonZeroUsize::new(1).unwrap(),
        Grant::new(),
    );
    check(
        attempt.as_mut().now_or_never().is_none(),
        "deadline owner initially scheduled",
    )?;
    let until = js_sys::Date::now() + 25.0;
    while js_sys::Date::now() < until {}
    check(
        matches!(attempt.await, Err(Error::Timeout)),
        "delayed owner cannot renew original deadline",
    )?;
    passed.push(&"original deadline before owner startup".into());

    let socket = connect(&base, "echo", 1024, grant.clone()).await?;
    let text = Frame::Text(" [\"original\",  7, \"café🌱\"] \n".into());
    socket.send(text.clone()).await.map_err(error)?;
    check(
        socket.receive().await.map_err(error)? == Some(text),
        "original text",
    )?;
    let binary = Frame::Binary(vec![0, 255, 10, 13, 128]);
    socket.send(binary.clone()).await.map_err(error)?;
    check(
        socket.receive().await.map_err(error)? == Some(binary),
        "original binary",
    )?;
    check(
        grant.frames.load(Ordering::SeqCst) == 2 && grant.bytes.load(Ordering::SeqCst) > 5,
        "original admission charges",
    )?;
    passed.push(&"original text binary and admission".into());
    for frame in [Frame::Ping(vec![]), Frame::Pong(vec![]), Frame::Close(None)] {
        check(
            socket.send(frame).await == Err(Error::Unsupported),
            "unsupported control",
        )?;
    }
    check(
        !BrowserConnector.supports_ping(),
        "browser ping unavailable",
    )?;
    check(
        socket.send(Frame::Binary(vec![0; 1025])).await == Err(Error::Protocol),
        "outgoing size",
    )?;
    passed.push(&"control and outgoing bounds".into());

    let mut pending = socket.send(Frame::Text("observer lost".into()));
    check(
        pending.as_mut().now_or_never().is_none(),
        "admitted send starts pending",
    )?;
    drop(pending);
    check(
        socket.send(Frame::Text("cannot replace".into())).await == Err(Error::Policy),
        "cancelled observer cannot replace accepted send",
    )?;
    check(
        socket.receive().await.map_err(error)? == Some(Frame::Text("observer lost".into())),
        "original admitted send retained",
    )?;
    passed.push(&"send observer cancellation".into());

    let mut pending = socket.receive();
    check(pending.as_mut().now_or_never().is_none(), "pending receive")?;
    let revoking = grant.clone();
    wasm_bindgen_futures::spawn_local(async move {
        async_utility::time::sleep(Duration::from_millis(10)).await;
        revoking.revoke();
    });
    check(
        pending.await == Err(Error::Policy),
        "revocation wakes original reader",
    )?;
    check(
        socket.send(Frame::Text("denied".into())).await.is_err(),
        "revoked send",
    )?;
    socket.close().await.map_err(error)?;
    socket.close().await.map_err(error)?;
    drop(socket);
    passed.push(&"revocation and repeated cleanup".into());

    for (route, limit, expected) in [
        ("oversize", 1024, Error::Protocol),
        ("unicode", 1024, Error::Protocol),
        ("binary-large", 1024, Error::Protocol),
        ("items", 1024, Error::Policy),
        ("bytes", 512 * 1024, Error::Policy),
    ] {
        let socket = connect(&base, route, limit, Grant::new()).await?;
        wasm_bindgen_futures::JsFuture::from(wait_for_close(route)).await?;
        check(socket.receive().await == Err(expected), route)?;
        socket.close().await.map_err(error)?;
        passed.push(&JsValue::from_str(route));
    }
    let grant = Grant::new();
    grant.charge.store(false, Ordering::SeqCst);
    let socket = connect(&base, "echo", 1024, grant.clone()).await?;
    socket
        .send(Frame::Text("original refusal".into()))
        .await
        .map_err(error)?;
    check(
        socket.receive().await == Err(Error::Policy),
        "original budget refuses callback",
    )?;
    check(
        grant.bytes.load(Ordering::SeqCst) == 16,
        "refused original bytes charged",
    )?;
    socket.close().await.map_err(error)?;
    passed.push(&"original budget refusal".into());

    let socket = connect(&base, "echo", 1024, Grant::new()).await?;
    force_backpressure();
    check(
        socket.send(Frame::Text("must not enqueue".into())).await == Err(Error::Policy),
        "reported browser outbound credit",
    )?;
    socket.close().await.map_err(error)?;
    passed.push(&"reported outbound buffer refusal".into());

    let socket = connect(&base, "disconnect", 1024, Grant::new()).await?;
    check(
        socket.receive().await.map_err(error)? == Some(Frame::Text("before close".into())),
        "retained before peer close",
    )?;
    check(
        socket.receive().await.map_err(error)?.is_none(),
        "peer end observed",
    )?;
    socket.close().await.map_err(error)?;
    passed.push(&"peer disconnect preserves preceding message".into());

    let socket = connect(&base, "echo", 1024, Grant::new()).await?;
    drop(socket);
    pause().await;
    passed.push(&"last socket observer cleanup".into());

    let endpoint = format!("{base}/stall");
    let mut attempt = BrowserConnector.connect(
        &endpoint,
        Duration::from_secs(2),
        NonZeroUsize::new(1).unwrap(),
        Grant::new(),
    );
    check(
        attempt.as_mut().now_or_never().is_none(),
        "connect initially pending",
    )?;
    pause().await;
    drop(attempt);
    pause().await;
    passed.push(&"cancelled handshake cleanup".into());
    let result = BrowserConnector
        .connect(
            &endpoint,
            Duration::from_millis(20),
            NonZeroUsize::new(1).unwrap(),
            Grant::new(),
        )
        .await;
    check(
        matches!(result, Err(Error::Timeout)),
        "original handshake deadline",
    )?;
    passed.push(&"handshake deadline cleanup".into());
    pause().await;
    Ok(passed)
}
