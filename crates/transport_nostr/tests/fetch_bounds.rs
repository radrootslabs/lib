use futures::{SinkExt, StreamExt};
use nostr_sdk::prelude::{EventBuilder, JsonUtil, Keys};
use radroots_transport::{
    EventSource, FetchRequest, TargetSet, outcome::FetchTargetState, source::FetchBounds,
};
use radroots_transport_nostr::{
    Config, NostrTransport, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind,
    RelayUrlPolicy,
};
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, sync::oneshot};
use tokio_tungstenite::{accept_async, tungstenite::Message};

fn transport(urls: &[String], connections: usize) -> (NostrTransport, FetchRequest) {
    let endpoints = urls
        .iter()
        .map(|url| RelayEndpoint::new(url, RelayUrlPolicy::Local, RelayAccess::ReadOnly).unwrap());
    let profile = RelayProfile::explicit(RelayProfileKind::Simulator, endpoints).unwrap();
    let config = Config::from_profile(profile)
        .with_timeouts(1000, 1000, 500)
        .unwrap()
        .with_max_connections(connections)
        .unwrap();
    let targets = TargetSet::new(
        config
            .read_relays()
            .map(|relay| relay.to_target().unwrap())
            .collect(),
    )
    .unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let request = FetchRequest::new(
        "bounded-loopback",
        targets,
        FetchBounds::new(10, now + 5000).unwrap(),
    )
    .unwrap();
    (NostrTransport::new(config), request)
}

async fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    (listener, url)
}

fn event(content: &str) -> Value {
    let keys =
        Keys::parse("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
    let event = EventBuilder::text_note(content)
        .sign_with_keys(&keys)
        .unwrap();
    serde_json::from_str(&event.as_json()).unwrap()
}

async fn serve(
    listener: TcpListener,
    payload: Option<Value>,
    eose: bool,
    requests: Arc<AtomicUsize>,
) {
    let (stream, _) = listener.accept().await.unwrap();
    let mut socket = accept_async(stream).await.unwrap();
    while let Some(message) = socket.next().await {
        let Ok(Message::Text(message)) = message else {
            continue;
        };
        let values: Value = serde_json::from_str(&message).unwrap();
        if values[0] != "REQ" {
            continue;
        }
        requests.fetch_add(1, Ordering::SeqCst);
        if let Some(payload) = &payload {
            socket
                .send(Message::Text(
                    serde_json::to_string(&("EVENT", &values[1], payload))
                        .unwrap()
                        .into(),
                ))
                .await
                .unwrap();
        }
        if eose {
            socket
                .send(Message::Text(
                    serde_json::to_string(&("EOSE", &values[1])).unwrap().into(),
                ))
                .await
                .unwrap();
        }
        std::future::pending::<()>().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn queued_relays_do_not_receive_a_fresh_timeout_after_a_stalled_relay() {
    let (first, first_url) = listener().await;
    let (second, second_url) = listener().await;
    // Adapter scheduling is canonical URL order, independent of profile order.
    let mut relays = [(first_url, first), (second_url, second)];
    relays.sort_by(|left, right| left.0.cmp(&right.0));
    let [(first_url, first), (second_url, second)] = relays;
    let first_requests = Arc::new(AtomicUsize::new(0));
    let second_requests = Arc::new(AtomicUsize::new(0));
    let first_task = tokio::spawn(serve(first, None, false, Arc::clone(&first_requests)));
    let second_task = tokio::spawn(serve(second, None, true, Arc::clone(&second_requests)));
    let (transport, request) = transport(&[first_url, second_url], 1);
    let page = tokio::time::timeout(Duration::from_secs(5), transport.fetch(request))
        .await
        .unwrap()
        .unwrap();
    first_task.abort();
    second_task.abort();
    assert_eq!(first_requests.load(Ordering::SeqCst), 1);
    assert_eq!(second_requests.load(Ordering::SeqCst), 0);
    assert_eq!(page.target_outcomes().len(), 2);
    assert!(
        page.target_outcomes()
            .iter()
            .all(|outcome| outcome.state() == FetchTargetState::Cancelled)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_relay_preserves_the_other_relays_completed_evidence_and_collected_events() {
    let (good, good_url) = listener().await;
    let (slow, slow_url) = listener().await;
    let good_event = event("complete relay");
    let slow_event = event("partial relay");
    let expected = [
        good_event["id"].as_str().unwrap().to_owned(),
        slow_event["id"].as_str().unwrap().to_owned(),
    ];
    let calls = Arc::new(AtomicUsize::new(0));
    let good_task = tokio::spawn(serve(good, Some(good_event), true, Arc::clone(&calls)));
    let slow_task = tokio::spawn(serve(slow, Some(slow_event), false, Arc::clone(&calls)));
    let (transport, request) = transport(&[good_url, slow_url], 2);
    let page = tokio::time::timeout(Duration::from_secs(5), transport.fetch(request))
        .await
        .unwrap()
        .unwrap();
    good_task.abort();
    slow_task.abort();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(page.events().len(), 2);
    for id in expected {
        assert!(
            page.events()
                .iter()
                .any(|event| event.event().id_str() == id)
        );
    }
    assert_eq!(
        page.target_outcomes()
            .iter()
            .filter(|outcome| outcome.state() == FetchTargetState::Complete)
            .count(),
        1
    );
    assert_eq!(
        page.target_outcomes()
            .iter()
            .filter(|outcome| outcome.state() == FetchTargetState::Cancelled)
            .count(),
        1
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn oversized_wire_messages_never_become_completed_fetch_evidence() {
    let (listener, url) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        while let Some(message) = socket.next().await {
            let message = match message {
                Ok(Message::Text(message)) => message,
                Ok(Message::Close(_)) | Err(_) => return,
                _ => continue,
            };
            let values: Value = serde_json::from_str(&message).unwrap();
            if values[0] == "REQ" {
                let _ = socket
                    .send(Message::Text(
                        serde_json::to_string(&("NOTICE", "x".repeat(512 * 1024)))
                            .unwrap()
                            .into(),
                    ))
                    .await;
            }
        }
    });
    let (transport, request) = transport(&[url], 1);
    let page = tokio::time::timeout(Duration::from_secs(5), transport.fetch(request))
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    assert!(page.events().is_empty());
    assert_eq!(page.target_outcomes().len(), 1);
    assert_ne!(
        page.target_outcomes()[0].state(),
        FetchTargetState::Complete
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_a_polled_fetch_retains_the_original_remote_auto_close_bound() {
    let (listener, url) = listener().await;
    let (started, observed) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let mut started = Some(started);
        while let Some(message) = socket.next().await {
            let Ok(Message::Text(message)) = message else {
                continue;
            };
            let values: Value = serde_json::from_str(&message).unwrap();
            if values[0] == "REQ" {
                started.take().unwrap().send(()).unwrap();
            } else if values[0] == "CLOSE" {
                return;
            }
        }
        panic!("the published subscription must receive CLOSE");
    });
    let (transport, request) = transport(&[url], 1);
    let mut fetch = Box::pin(transport.fetch(request));
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            _ = &mut fetch => panic!("fetch completed before its relay response"),
            result = observed => { result.unwrap(); }
        }
    })
    .await
    .unwrap();
    drop(fetch);
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
}
