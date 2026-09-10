use futures::{SinkExt, StreamExt};
use nostr_sdk::prelude::{EventBuilder, JsonUtil, Keys, Timestamp};
use radroots_transport::{
    EventSource, FetchRequest, TargetSet,
    outcome::FetchTargetState,
    source::{FetchBounds, NextPage},
};
use radroots_transport_nostr::{
    Config, NostrTransport, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind,
    RelayUrlPolicy,
};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[tokio::test(flavor = "multi_thread")]
async fn real_capped_eose_preserves_received_ties_and_yields_explicit_older_history() {
    let keys =
        Keys::parse("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
    let mut history = (0..1001)
        .map(|index| {
            let event = EventBuilder::text_note(format!("equal-time {index}"))
                .custom_created_at(Timestamp::from_secs(100))
                .sign_with_keys(&keys)
                .unwrap();
            serde_json::from_str::<Value>(&event.as_json()).unwrap()
        })
        .collect::<Vec<_>>();
    history.sort_by(|left, right| right["id"].as_str().cmp(&left["id"].as_str()));
    let expected = history[..1000]
        .iter()
        .map(|event| event["id"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    let older = EventBuilder::text_note("older")
        .custom_created_at(Timestamp::from_secs(99))
        .sign_with_keys(&keys)
        .unwrap();
    history.push(serde_json::from_str(&older.as_json()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let server_calls = calls.clone();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        while let Some(Ok(message)) = socket.next().await {
            let Message::Text(message) = message else {
                continue;
            };
            let request: Value = serde_json::from_str(&message).unwrap();
            if request[0] != "REQ" {
                continue;
            }
            server_calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request[2]["limit"], 1000);
            let until = request[2]["until"].as_u64().unwrap_or(u64::MAX);
            for event in history
                .iter()
                .filter(|event| event["created_at"].as_u64().unwrap() <= until)
                .take(1000)
            {
                socket
                    .send(Message::Text(
                        serde_json::to_string(&("EVENT", &request[1], event))
                            .unwrap()
                            .into(),
                    ))
                    .await
                    .unwrap();
            }
            socket
                .send(Message::Text(
                    serde_json::to_string(&("EOSE", &request[1]))
                        .unwrap()
                        .into(),
                ))
                .await
                .unwrap();
        }
    });
    let profile = RelayProfile::explicit(
        RelayProfileKind::Simulator,
        [RelayEndpoint::new(&url, RelayUrlPolicy::Local, RelayAccess::ReadOnly).unwrap()],
    )
    .unwrap();
    let config = Config::from_profile(profile)
        .with_timeouts(5000, 5000, 500)
        .unwrap();
    let targets = TargetSet::new(
        config
            .read_relays()
            .map(|relay| relay.to_target().unwrap())
            .collect(),
    )
    .unwrap();
    let transport = NostrTransport::new(config);
    let deadline = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        + 20000;
    let request = FetchRequest::new(
        "live-capped-window",
        targets,
        FetchBounds::new(500, deadline).unwrap(),
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        let first = transport.fetch(request.clone()).await.unwrap();
        assert_eq!(first.events().len(), 500);
        assert_eq!(
            first.target_outcomes()[0].state(),
            FetchTargetState::Partial
        );
        let NextPage::Cursor(cursor) = first.next_page() else {
            panic!("received peers remain")
        };
        let second = transport
            .fetch(request.clone().with_cursor(cursor.clone()))
            .await
            .unwrap();
        assert_eq!(second.events().len(), 500);
        assert_eq!(
            second.target_outcomes()[0].state(),
            FetchTargetState::Partial
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let received = first
            .events()
            .iter()
            .chain(second.events())
            .map(|e| e.event().id_str().to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(received, expected);
        let NextPage::Cancelled {
            resume_from: Some(older),
        } = second.next_page()
        else {
            panic!("explicit partial window yield")
        };
        let third = transport
            .fetch(request.with_cursor(older.clone()))
            .await
            .unwrap();
        assert_eq!(third.events().len(), 1);
        assert_eq!(third.events()[0].event().created_at(), 99);
        assert!(matches!(third.next_page(), NextPage::Complete));
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    })
    .await
    .unwrap();
    server.abort();
}
