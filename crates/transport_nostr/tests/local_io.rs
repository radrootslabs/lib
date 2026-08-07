use futures::{SinkExt, StreamExt};
use radroots_transport::{
    EventSource, FetchRequest, TargetSet, capability::Availability, outcome::FetchTargetState,
    source::FetchBounds,
};
use radroots_transport_nostr::{
    Config, NostrTransport, RelayAggregateState, RelayEvidenceState, RelayProfile,
};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[tokio::test(flavor = "multi_thread")]
async fn simulator_profile_proves_read_capability_against_a_real_loopback_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind relay");
    let address = listener.local_addr().expect("relay address");
    let relay_url = format!("ws://{address}");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept relay client");
        let mut websocket = accept_async(stream).await.expect("websocket handshake");
        while let Some(message) = websocket.next().await {
            let Message::Text(message) = message.expect("client message") else {
                continue;
            };
            let parsed: Value = serde_json::from_str(message.as_str()).expect("Nostr message");
            let Some(values) = parsed.as_array() else {
                continue;
            };
            let [Value::String(kind), Value::String(subscription), ..] = values.as_slice() else {
                continue;
            };
            if kind == "REQ" {
                websocket
                    .send(Message::Text(
                        serde_json::to_string(&("EOSE", subscription))
                            .expect("EOSE message")
                            .into(),
                    ))
                    .await
                    .expect("send EOSE");
                break;
            }
        }
    });

    let profile = RelayProfile::simulator([relay_url.as_str()]).expect("simulator profile");
    let config = Config::from_profile(profile)
        .with_timeouts(1_000, 2_000, 500)
        .expect("timeouts");
    let targets = TargetSet::new(
        config
            .read_relays()
            .map(|relay| relay.to_target())
            .collect::<Result<Vec<_>, _>>()
            .expect("targets"),
    )
    .expect("target set");
    let request = FetchRequest::new(
        "local-real-io",
        targets,
        FetchBounds::new(1, unix_time_ms() + 5_000).expect("bounds"),
    )
    .expect("request");
    let transport = NostrTransport::new(config);
    let page = tokio::time::timeout(Duration::from_secs(5), transport.fetch(request))
        .await
        .expect("bounded fetch")
        .expect("fetch page");

    assert!(page.events().is_empty());
    assert_eq!(page.target_outcomes().len(), 1);
    assert_eq!(
        page.target_outcomes()[0].state(),
        FetchTargetState::Complete
    );
    let status = transport.relay_status();
    assert_eq!(status.state(), RelayAggregateState::ReadOnly);
    assert_eq!(status.read_availability(), Availability::Available);
    assert_eq!(status.write_availability(), Availability::Unavailable);
    assert_eq!(
        status.relays()[0].read().state(),
        RelayEvidenceState::Available
    );
    assert_eq!(
        status.relays()[0].write().state(),
        RelayEvidenceState::Unobserved
    );
    server.await.expect("relay task");
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
