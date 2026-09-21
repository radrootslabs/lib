use futures::{FutureExt, SinkExt, StreamExt};
use nostr_sdk::prelude::{EventBuilder, JsonUtil, Keys, Kind, Tag, Timestamp};
use radroots_transport::{
    DeliveryRequest, EventSink, EventSource, FetchRequest, TargetSet,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryPayload,
    source::FetchBounds,
};
use radroots_transport_nostr::{
    Config, NostrTransport, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind,
    RelayUrlPolicy,
};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};

const KEY: &str = "0000000000000000000000000000000000000000000000000000000000000001";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap()
}

fn raw_event() -> String {
    let event = EventBuilder::text_note("harvest café\n🌱")
        .custom_created_at(Timestamp::from_secs(1_800_000_000))
        .sign_with_keys(&Keys::parse(KEY).unwrap())
        .unwrap();
    let mut value: Value = serde_json::from_str(&event.as_json()).unwrap();
    value["extension"] = json!({"retained": true});
    format!(" \n{} \t", serde_json::to_string_pretty(&value).unwrap())
}

fn config(url: &str, timeout: u64) -> Config {
    Config::from_profile(
        RelayProfile::explicit(
            RelayProfileKind::Simulator,
            [RelayEndpoint::new(url, RelayUrlPolicy::Local, RelayAccess::ReadWrite).unwrap()],
        )
        .unwrap(),
    )
    .with_timeouts(1_000, timeout, 500)
    .unwrap()
}

fn targets(config: &Config) -> TargetSet {
    TargetSet::new(
        config
            .relays()
            .iter()
            .map(|relay| relay.to_target().unwrap())
            .collect(),
    )
    .unwrap()
}

fn request(config: &Config, raw: &str) -> DeliveryRequest {
    DeliveryRequest::new(
        "exact-wire",
        DeliveryPayload::new(radroots_event_codec::decode::signed_event(raw).unwrap()),
        targets(config),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        now() + 5_000,
    )
    .unwrap()
}

async fn text(socket: &mut WebSocketStream<TcpStream>) -> String {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
                return text.to_string();
            }
        }
    })
    .await
    .unwrap()
}

async fn reply(socket: &mut WebSocketStream<TcpStream>, value: Value) {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

#[tokio::test]
async fn exact_signed_bytes_read_requests_and_auth_use_the_same_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = config(&format!("ws://{}", listener.local_addr().unwrap()), 2_000);
    let raw = raw_event();
    let expected = format!("[\"EVENT\",{raw}]");
    let (auth_sent, auth_seen) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(tcp).await.unwrap();
        let req: Value = serde_json::from_str(&text(&mut socket).await).unwrap();
        assert_eq!(req[0], "REQ");
        reply(&mut socket, json!(["EOSE", req[1]])).await;
        loop {
            let message: Value = serde_json::from_str(&text(&mut socket).await).unwrap();
            if message[0] == "AUTH" {
                assert_eq!(message[1]["kind"], 22242);
                auth_sent.send(()).unwrap();
                break;
            }
            assert_eq!(message[0], "CLOSE");
        }
        let wire = text(&mut socket).await;
        assert_eq!(wire, expected);
        let event: Value = serde_json::from_str(&wire).unwrap();
        reply(
            &mut socket,
            json!(["OK", "11".repeat(32), true, "unrelated event"]),
        )
        .await;
        reply(&mut socket, json!(["OK", event[1]["id"], true, ""])).await;
    });
    let transport = NostrTransport::new(config.clone());
    let page = transport
        .fetch(
            FetchRequest::new(
                "same-socket-read",
                targets(&config),
                FetchBounds::new(1, now() + 5_000).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(page.events().is_empty());
    let relay = &config.relays()[0];
    let at = now() / 1_000 * 1_000;
    transport
        .begin_authentication(relay, "challenge", at, at + 5_000)
        .unwrap();
    let auth = EventBuilder::new(Kind::Authentication, "")
        .tags([
            Tag::parse(["relay", relay.as_str()]).unwrap(),
            Tag::parse(["challenge", "challenge"]).unwrap(),
        ])
        .custom_created_at(Timestamp::from_secs(at / 1_000))
        .sign_with_keys(&Keys::parse(KEY).unwrap())
        .unwrap();
    transport
        .complete_authentication(relay, "challenge", Some(&auth.as_json()), at)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), auth_seen)
        .await
        .unwrap()
        .unwrap();
    let receipt = transport.deliver(request(&config, &raw)).await.unwrap();
    assert!(
        receipt.target_receipts()[0]
            .outcome()
            .satisfies(SatisfactionClass::Accepted)
    );
    server.await.unwrap();
}

#[tokio::test]
async fn rejection_lost_ack_and_disconnect_never_invent_acceptance() {
    for mode in ["reject", "lost", "disconnect"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let config = config(&format!("ws://{}", listener.local_addr().unwrap()), 250);
        let raw = raw_event();
        let expected = format!("[\"EVENT\",{raw}]");
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(tcp).await.unwrap();
            let wire = text(&mut socket).await;
            assert_eq!(wire, expected);
            let event: Value = serde_json::from_str(&wire).unwrap();
            match mode {
                "reject" => {
                    reply(
                        &mut socket,
                        json!(["OK", event[1]["id"], false, "blocked: fixture"]),
                    )
                    .await
                }
                "lost" => {
                    reply(
                        &mut socket,
                        json!(["OK", "22".repeat(32), true, "wrong ID"]),
                    )
                    .await;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                "disconnect" => socket.close(None).await.unwrap(),
                _ => unreachable!(),
            }
        });
        let receipt = NostrTransport::new(config.clone())
            .deliver(request(&config, &raw))
            .await
            .unwrap();
        assert!(
            !receipt.target_receipts()[0]
                .outcome()
                .satisfies(SatisfactionClass::Accepted),
            "{mode}"
        );
        assert!(receipt.target_receipts()[0].was_attempted());
        server.await.unwrap();
    }
}

#[tokio::test]
async fn queued_delivery_targets_cannot_start_after_the_shared_deadline() {
    let first = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoints = [&first, &second].map(|listener| {
        RelayEndpoint::new(
            format!("ws://{}", listener.local_addr().unwrap()),
            RelayUrlPolicy::Local,
            RelayAccess::ReadWrite,
        )
        .unwrap()
    });
    let config = Config::from_profile(
        RelayProfile::explicit(RelayProfileKind::Simulator, endpoints).unwrap(),
    )
    .with_timeouts(1_000, 250, 500)
    .unwrap()
    .with_max_connections(1)
    .unwrap();
    let server = tokio::spawn(async move {
        let (tcp, _) = first.accept().await.unwrap();
        let mut socket = accept_async(tcp).await.unwrap();
        let _published = text(&mut socket).await;
        futures::future::pending::<()>().await;
    });
    let transport = NostrTransport::new(config.clone());
    let receipt = transport
        .deliver(request(&config, &raw_event()))
        .await
        .unwrap();
    assert_eq!(receipt.target_receipts().len(), 2);
    assert!(
        receipt
            .target_receipts()
            .iter()
            .all(|target| !target.outcome().satisfies(SatisfactionClass::Accepted))
    );
    assert!(second.accept().now_or_never().is_none());
    assert!(receipt.target_receipts()[0].was_attempted());
    assert!(
        !receipt.target_receipts()[1].was_attempted(),
        "a target expired while queued never entered remote publication"
    );
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
