use super::*;
use futures::{SinkExt, StreamExt};
use radroots_transport::host::websocket::{CloseFrame, Error, IoFuture};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn host_policy_and_timeout_keep_their_sdk_class_without_misclassifying_authority_as_auth() {
    let policy = host_error(Error::Policy);
    assert_eq!(policy.kind(), nostr_sdk::error::ErrorKind::Policy);
    assert_eq!(
        crate::status::delivery_failure(&policy.to_string()),
        crate::status::delivery_failure("rejected")
    );
    let timeout = host_error(Error::Timeout);
    assert_eq!(timeout.kind(), nostr_sdk::error::ErrorKind::Timeout);
    assert_eq!(
        crate::status::delivery_failure(&timeout.to_string()),
        crate::status::delivery_failure("timeout")
    );
    for error in [
        Error::Unavailable,
        Error::Closed,
        Error::Protocol,
        Error::Unsupported,
    ] {
        let mapped = host_error(error);
        assert_eq!(mapped.kind(), nostr_sdk::error::ErrorKind::Transport);
        assert!(mapped.to_string().len() < 128);
    }
}

#[test]
fn original_text_identifiers_are_exact_and_binary_is_never_promoted_or_reencoded() {
    use serde_json::json;
    let event = json!({"id": "ab".repeat(32), "pubkey": "cd".repeat(32),
                       "sig": "ef".repeat(64), "extension": true});
    let check = |text: String, valid| {
        for frame in [
            Frame::Text(text.clone()),
            Frame::Binary(text.clone().into_bytes()),
        ] {
            let result = from_host_frame(frame.clone());
            let accepted = valid || matches!(frame, Frame::Binary(_));
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                assert_eq!(to_host_frame(result.unwrap().unwrap()), frame);
            }
        }
    };
    check(
        format!(" \n{} \t", json!(["EVENT", "bounded", event])),
        true,
    );
    for field in ["id", "pubkey", "sig"] {
        let original = event[field].as_str().unwrap();
        for invalid in [
            json!(format!("{original}00")),
            json!(&original[2..]),
            json!(original.to_ascii_uppercase()),
            json!(format!("g{}", &original[1..])),
            json!(null),
            json!(32),
        ] {
            let mut malformed = event.clone();
            malformed[field] = invalid;
            check(json!(["EVENT", "bounded", malformed]).to_string(), false);
        }
    }
    for id in [json!("aa".repeat(33)), json!("AA".repeat(32)), json!(null)] {
        check(json!(["OK", id, true, ""]).to_string(), false);
    }
    for text in [
        "not-json".to_owned(),
        "[]".to_owned(),
        "[\"EVENT\"]".to_owned(),
        "[\"NOTICE\",\"retained\"]".to_owned(),
        json!(["OK", "aa".repeat(32), true, ""]).to_string(),
    ] {
        let valid = text != "[\"EVENT\"]";
        check(text, valid);
    }
    for frame in [
        Frame::Text("x".repeat(MAX_WIRE_MESSAGE_BYTES + 1)),
        Frame::Binary(vec![0; MAX_WIRE_MESSAGE_BYTES + 1]),
    ] {
        assert!(from_host_frame(frame).is_err());
    }
    let non_utf8 = Frame::Binary(vec![0xff, 0]);
    assert_eq!(
        to_host_frame(from_host_frame(non_utf8.clone()).unwrap().unwrap()),
        non_utf8
    );
}

#[test]
fn owned_data_control_and_close_information_round_trip_at_the_shared_sdk_boundary() {
    for frame in [
        Frame::Text("text".into()),
        Frame::Binary(vec![0, 255]),
        Frame::Ping(vec![1, 2]),
        Frame::Pong(vec![3]),
        Frame::Close(None),
        Frame::Close(Some(CloseFrame {
            code: 1000,
            reason: "finished".into(),
        })),
    ] {
        assert_eq!(
            to_host_frame(from_host_frame(frame.clone()).unwrap().unwrap()),
            frame
        );
    }
}

#[derive(Default)]
struct FakeSocket {
    sent: Mutex<Vec<Frame>>,
    received: Mutex<std::collections::VecDeque<Result<Option<Frame>, Error>>>,
    closes: AtomicUsize,
}
impl Socket for FakeSocket {
    fn send(&self, frame: Frame) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.sent.lock().unwrap().push(frame);
            Ok(())
        })
    }
    fn receive(&self) -> IoFuture<'_, Result<Option<Frame>, Error>> {
        Box::pin(async move {
            self.received
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(None))
        })
    }
    fn close(&self) -> IoFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            self.closes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}

#[derive(Debug, Default)]
struct Calls(AtomicUsize);
#[derive(Default)]
struct Host {
    calls: Arc<Calls>,
    socket: Arc<FakeSocket>,
}
impl fmt::Debug for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Host")
    }
}
impl Connector for Host {
    fn supports_ping(&self) -> bool {
        false
    }
    fn connect<'a>(
        &'a self,
        _: &'a str,
        _: Duration,
        _: NonZeroUsize,
        _: Arc<dyn Admission>,
    ) -> IoFuture<'a, Result<Arc<dyn Socket>, Error>> {
        Box::pin(async move {
            self.calls.0.fetch_add(1, Ordering::SeqCst);
            Ok(self.socket.clone() as Arc<dyn Socket>)
        })
    }
}

#[tokio::test]
async fn exact_writer_and_sdk_messages_use_one_host_socket_and_close_it_once() {
    let host = Arc::new(Host::default());
    host.socket.received.lock().unwrap().extend([
        Ok(Some(Frame::Text("[\"NOTICE\",\"first\"]".into()))),
        Err(Error::Closed),
    ]);
    let endpoint = crate::RelayEndpoint::new(
        "ws://127.0.0.1:1234",
        RelayUrlPolicy::Local,
        crate::RelayAccess::ReadWrite,
    )
    .unwrap();
    let bridge = SessionWebsocketTransport::new(&[endpoint], Duration::from_secs(1), host.clone());
    assert!(!bridge.support_ping());
    let url = Url::parse("ws://127.0.0.1:1234").unwrap();
    let pending = bridge.connect(&url, None);
    drop(pending);
    assert_eq!(host.calls.0.load(Ordering::SeqCst), 0);
    let (mut sink, mut receiver) = bridge.connect(&url, None).await.unwrap();
    let writer = bridge.writers.get("ws://127.0.0.1:1234").unwrap();
    let original = " \n[\"EVENT\", { \"z\":1, \"a\":2 }] \t";
    writer.send(Message::Text(original.into())).await.unwrap();
    sink.send(Message::Text("[\"REQ\",\"read\",{}]".into()))
        .await
        .unwrap();
    assert_eq!(
        *host.socket.sent.lock().unwrap(),
        vec![
            Frame::Text(original.into()),
            Frame::Text("[\"REQ\",\"read\",{}]".into())
        ]
    );
    assert_eq!(
        receiver.next().await.unwrap().unwrap(),
        Message::Text("[\"NOTICE\",\"first\"]".into())
    );
    assert!(receiver.next().await.unwrap().is_err());
    assert!(receiver.next().await.is_none());
    sink.close().await.unwrap();
    sink.close().await.unwrap();
    assert_eq!(host.socket.closes.load(Ordering::SeqCst), 1);
    assert!(writer.send(Message::Text("denied".into())).await.is_err());
    assert_eq!(host.calls.0.load(Ordering::SeqCst), 1);
}

#[test]
fn host_sink_enforces_pending_ready_and_closed_states_without_discarding_close() {
    let socket = Arc::new(FakeSocket::default());
    let mut sink = HostSink::new(socket.clone());
    let mut context = Context::from_waker(futures::task::noop_waker_ref());
    Pin::new(&mut sink)
        .start_send(Message::Text("first".into()))
        .unwrap();
    assert!(
        Pin::new(&mut sink)
            .start_send(Message::Text("second".into()))
            .is_err()
    );
    assert!(matches!(
        Pin::new(&mut sink).poll_ready(&mut context),
        Poll::Ready(Ok(()))
    ));
    assert!(matches!(
        Pin::new(&mut sink).poll_close(&mut context),
        Poll::Ready(Ok(()))
    ));
    assert!(matches!(
        Pin::new(&mut sink).poll_ready(&mut context),
        Poll::Ready(Err(_))
    ));
    assert!(
        Pin::new(&mut sink)
            .start_send(Message::Text("denied".into()))
            .is_err()
    );
    assert!(matches!(
        Pin::new(&mut sink).poll_close(&mut context),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(socket.closes.load(Ordering::SeqCst), 1);
    assert_eq!(
        *socket.sent.lock().unwrap(),
        vec![Frame::Text("first".into())]
    );
}

#[tokio::test]
async fn proxy_or_unconfigured_endpoint_never_calls_the_host() {
    let host = Arc::new(Host::default());
    let endpoint = crate::RelayEndpoint::new(
        "ws://127.0.0.1:1234",
        RelayUrlPolicy::Local,
        crate::RelayAccess::ReadOnly,
    )
    .unwrap();
    let bridge = SessionWebsocketTransport::new(&[endpoint], Duration::from_secs(1), host.clone());
    let url = Url::parse("ws://127.0.0.1:1234").unwrap();
    assert!(
        bridge
            .connect(&url, Some("127.0.0.1:9000".parse().unwrap()))
            .await
            .is_err()
    );
    assert!(
        bridge
            .connect(&Url::parse("ws://127.0.0.1:1235").unwrap(), None)
            .await
            .is_err()
    );
    assert_eq!(host.calls.0.load(Ordering::SeqCst), 0);
    for (url, expected) in [
        ("wss://relay.example/", "wss://relay.example"),
        ("wss://relay.example/path/", "wss://relay.example/path/"),
        (
            "wss://relay.example/?scope=write",
            "wss://relay.example/?scope=write",
        ),
        ("wss://relay.example/#status", "wss://relay.example/#status"),
    ] {
        assert_eq!(configured_policy_key(&Url::parse(url).unwrap()), expected);
    }
}

#[test]
fn connection_admission_preserves_policy_original_budget_and_generation() {
    let key = "ws://127.0.0.1:1234";
    let registry = crate::source_ingress::IngressRegistry::new([key.to_owned()].into_iter());
    let budget = Arc::new(crate::source::budget::FetchBudget::default());
    let registration = registry
        .register([key.to_owned()].into_iter(), budget.clone())
        .unwrap();
    let admission = ConnectionAdmission {
        policy: RelayUrlPolicy::Local,
        ingress: registry.connection(key).unwrap(),
    };
    assert!(admission.permits_address("127.0.0.1".parse().unwrap()));
    assert!(!admission.permits_address("8.8.8.8".parse().unwrap()));
    let context = Context::from_waker(futures::task::noop_waker_ref());
    assert!(admission.admitted_read(&context) && admission.admitted_write(&context));
    assert!(admission.charge(crate::source::budget::MAX_FETCH_BYTES, 0, 0));
    assert!(!admission.charge(1, 0, 0));
    assert!(budget.exhausted());
    assert!(!admission.admitted_read(&context));
    drop(registration);
    assert!(!admission.admitted_write(&context));
    assert!(!admission.charge(0, 0, 0));
    assert_eq!(format!("{admission:?}"), "ConnectionAdmission([redacted])");
}
