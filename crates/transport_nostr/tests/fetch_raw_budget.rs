use futures::{SinkExt, StreamExt};
use radroots_transport::{
    EventSource, FetchRequest, TargetSet, outcome::FetchTargetState, source::FetchBounds,
};
use radroots_transport_nostr::{
    Config, NostrTransport, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind,
    RelayUrlPolicy,
};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};

const WATCHDOG: Duration = Duration::from_secs(10);

fn transport(urls: &[String]) -> (NostrTransport, FetchRequest) {
    let profile = RelayProfile::explicit(
        RelayProfileKind::Simulator,
        urls.iter().map(|url| {
            RelayEndpoint::new(url, RelayUrlPolicy::Local, RelayAccess::ReadOnly).unwrap()
        }),
    )
    .unwrap();
    let config = Config::from_profile(profile)
        .with_timeouts(1000, 5000, 500)
        .unwrap()
        .with_max_connections(1)
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
        "raw-budget-loopback",
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

fn malformed_event() -> Value {
    // Canonical primitive fields pass wire admission, while absent event
    // fields still exercise malformed traffic in the aggregate ingress budget.
    let value = serde_json::json!({
        "id": "00".repeat(32),
        "pubkey": "11".repeat(32),
        "sig": "22".repeat(64),
        "invalid_event": "x".repeat(384 * 1024),
    });
    assert!(serde_json::from_value::<nostr_sdk::prelude::Event>(value.clone()).is_err());
    value
}

// A WebSocket pong is produced only after the client's receive stream has
// consumed all preceding messages. No scheduling sleep is an admission barrier.
async fn barrier(socket: &mut WebSocketStream<TcpStream>, sequence: usize) -> bool {
    let marker = sequence.to_be_bytes().to_vec();
    if socket
        .send(Message::Ping(marker.clone().into()))
        .await
        .is_err()
    {
        return false;
    }
    while let Some(message) = socket.next().await {
        match message {
            Ok(Message::Pong(value)) if value.as_ref() == marker.as_slice() => return true,
            Ok(Message::Close(_)) | Err(_) => return false,
            _ => {
                if socket.flush().await.is_err() {
                    return false;
                }
            }
        }
    }
    false
}

async fn serve_malformed(listener: TcpListener, frames: usize) {
    let (stream, _) = listener.accept().await.unwrap();
    let mut socket = accept_async(stream).await.unwrap();
    while let Some(message) = socket.next().await {
        let text = match message {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) | Err(_) => return,
            _ => {
                let _ = socket.flush().await;
                continue;
            }
        };
        let values: Value = serde_json::from_str(&text).unwrap();
        if values[0] != "REQ" {
            continue;
        }
        let malformed = serde_json::to_string(&("EVENT", &values[1], malformed_event())).unwrap();
        assert!(malformed.len() < 512 * 1024);
        for sequence in 0..frames {
            if socket
                .send(Message::Text(malformed.clone().into()))
                .await
                .is_err()
                || !barrier(&mut socket, sequence).await
            {
                return;
            }
        }
        if socket
            .send(Message::Text(
                serde_json::to_string(&("EOSE", &values[1])).unwrap().into(),
            ))
            .await
            .is_err()
        {
            return;
        }
    }
}

async fn fetch_malformed(frames: usize) -> FetchTargetState {
    let (listener, url) = listener().await;
    let server = tokio::spawn(serve_malformed(listener, frames));
    let (transport, request) = transport(&[url]);
    let result = tokio::time::timeout(WATCHDOG, transport.fetch(request)).await;
    server.abort();
    let joined = server.await;
    assert!(joined.is_ok() || joined.unwrap_err().is_cancelled());
    let page = result.unwrap().unwrap();
    assert!(page.events().is_empty());
    assert_eq!(page.target_outcomes().len(), 1);
    page.target_outcomes()[0].state()
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_eose_remains_complete_with_raw_ingress_accounting() {
    assert_eq!(fetch_malformed(0).await, FetchTargetState::Complete);
}

#[tokio::test(flavor = "multi_thread")]
async fn malformed_event_traffic_exhausts_raw_budget_before_eose() {
    assert_eq!(fetch_malformed(24).await, FetchTargetState::Partial);
}

async fn finish_server(mut server: tokio::task::JoinHandle<()>) {
    let result = tokio::time::timeout(WATCHDOG, &mut server).await;
    if result.is_err() {
        server.abort();
        let _ = server.await;
    }
    result
        .expect("client must close the bounded relay connection")
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn completed_batches_do_not_refund_raw_bytes_for_later_relays() {
    let (one, one_url) = listener().await;
    let (two, two_url) = listener().await;
    let servers = [
        tokio::spawn(serve_malformed(one, 12)),
        tokio::spawn(serve_malformed(two, 12)),
    ];
    let (transport, request) = transport(&[one_url, two_url]);
    let result = tokio::time::timeout(WATCHDOG, transport.fetch(request)).await;
    for server in servers {
        finish_server(server).await;
    }
    let page = result.unwrap().unwrap();
    assert!(page.events().is_empty());
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
            .filter(|outcome| outcome.state() == FetchTargetState::Partial)
            .count(),
        1
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unsolicited_malformed_traffic_is_bounded_before_the_request() {
    let (listener, url) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let malformed =
            serde_json::to_string(&("EVENT", "unsolicited", malformed_event())).unwrap();
        for sequence in 0..24 {
            if socket
                .send(Message::Text(malformed.clone().into()))
                .await
                .is_err()
                || !barrier(&mut socket, sequence).await
            {
                return;
            }
        }
        panic!("oversized unsolicited inventory must close before admission");
    });
    let (transport, request) = transport(&[url]);
    let result = tokio::time::timeout(WATCHDOG, transport.fetch(request)).await;
    finish_server(server).await;
    assert_eq!(
        result.unwrap().unwrap().target_outcomes()[0].state(),
        FetchTargetState::Partial
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn zero_payload_continuation_frames_consume_work_before_message_decoding() {
    use tokio_tungstenite::tungstenite::protocol::frame::{
        Frame,
        coding::{Data, OpCode},
    };
    let (listener, url) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        while let Some(message) = socket.next().await {
            let text = match message {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) | Err(_) => return,
                _ => {
                    let _ = socket.flush().await;
                    continue;
                }
            };
            let values: Value = serde_json::from_str(&text).unwrap();
            if values[0] != "REQ" {
                continue;
            }
            socket
                .send(Message::Frame(Frame::message(
                    Vec::new(),
                    OpCode::Data(Data::Text),
                    false,
                )))
                .await
                .unwrap();
            for sequence in 0..8193 {
                if socket
                    .send(Message::Frame(Frame::message(
                        Vec::new(),
                        OpCode::Data(Data::Continue),
                        false,
                    )))
                    .await
                    .is_err()
                {
                    return;
                }
                if sequence % 128 == 0 && !barrier(&mut socket, sequence).await {
                    return;
                }
            }
            if socket
                .send(Message::Frame(Frame::message(
                    b"[]".to_vec(),
                    OpCode::Data(Data::Continue),
                    true,
                )))
                .await
                .is_err()
            {
                return;
            }
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&("EOSE", &values[1])).unwrap().into(),
                ))
                .await;
        }
    });
    let (transport, request) = transport(&[url]);
    let result = tokio::time::timeout(WATCHDOG, transport.fetch(request)).await;
    finish_server(server).await;
    let page = result.unwrap().unwrap();
    assert!(page.events().is_empty());
    assert_eq!(page.target_outcomes()[0].state(), FetchTargetState::Partial);
}
