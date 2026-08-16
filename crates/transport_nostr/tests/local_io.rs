use futures::{SinkExt, StreamExt};
use radroots_transport::{
    EventSource, EventSubscriber, FetchRequest, SubscriptionNext, SubscriptionRequest, TargetSet,
    capability::Availability,
    outcome::FetchTargetState,
    source::{FetchBounds, SubscriptionBounds, SubscriptionEndReason},
};
use radroots_transport_nostr::{
    Config, NostrTransport, RelayAccess, RelayAggregateState, RelayEndpoint, RelayEvidenceState,
    RelayProfile, RelayProfileKind, RelayUrlPolicy,
};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

const FIXTURE_SECRET_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000001";

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

    let endpoint = RelayEndpoint::new(
        relay_url.as_str(),
        RelayUrlPolicy::Local,
        RelayAccess::ReadWrite,
    )
    .expect("loopback endpoint");
    let profile =
        RelayProfile::explicit(RelayProfileKind::Simulator, [endpoint]).expect("simulator profile");
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

#[tokio::test(flavor = "multi_thread")]
async fn simulator_profile_streams_and_cancels_a_real_live_subscription() {
    use nostr_sdk::prelude::{EventBuilder, JsonUtil, Keys, Timestamp};

    let event = EventBuilder::text_note("bounded live event")
        .custom_created_at(Timestamp::from_secs(1_800_000_100))
        .sign_with_keys(&Keys::parse(FIXTURE_SECRET_KEY).expect("fixture keys"))
        .expect("signed event");
    let expected_id = event.id.to_hex();
    let event: Value = serde_json::from_str(event.as_json().as_str()).expect("event JSON");
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind relay");
    let address = listener.local_addr().expect("relay address");
    let relay_url = format!("ws://{address}");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept relay client");
        let mut websocket = accept_async(stream).await.expect("websocket handshake");
        let mut subscription_id = None;
        while let Some(message) = websocket.next().await {
            let Message::Text(message) = message.expect("client message") else {
                continue;
            };
            let parsed: Value = serde_json::from_str(message.as_str()).expect("Nostr message");
            let Some(values) = parsed.as_array() else {
                continue;
            };
            match values.as_slice() {
                [Value::String(kind), Value::String(subscription), ..] if kind == "REQ" => {
                    subscription_id = Some(subscription.clone());
                    websocket
                        .send(Message::Text(
                            serde_json::to_string(&("EVENT", subscription, &event))
                                .expect("EVENT message")
                                .into(),
                        ))
                        .await
                        .expect("send EVENT");
                }
                [Value::String(kind), Value::String(subscription)]
                    if kind == "CLOSE" && subscription_id.as_ref() == Some(subscription) =>
                {
                    return;
                }
                _ => {}
            }
        }
        panic!("client disconnected without CLOSE");
    });

    let endpoint = RelayEndpoint::new(
        relay_url.as_str(),
        RelayUrlPolicy::Local,
        RelayAccess::ReadWrite,
    )
    .expect("loopback endpoint");
    let profile =
        RelayProfile::explicit(RelayProfileKind::Simulator, [endpoint]).expect("simulator profile");
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
    let request = SubscriptionRequest::new(
        "local-live-io",
        targets,
        SubscriptionBounds::new(2, unix_time_ms() + 5_000).expect("bounds"),
    )
    .expect("request");
    let transport = NostrTransport::new(config);
    let mut subscription =
        tokio::time::timeout(Duration::from_secs(5), transport.subscribe(request))
            .await
            .expect("bounded subscribe")
            .expect("subscription");
    let SubscriptionNext::Event(observed) =
        tokio::time::timeout(Duration::from_secs(5), subscription.next())
            .await
            .expect("bounded event")
            .expect("event")
    else {
        panic!("event expected");
    };
    assert_eq!(observed.observed().event().id_str(), expected_id);
    assert_eq!(
        subscription.cancel().await.expect("cancel").reason(),
        SubscriptionEndReason::Cancelled
    );
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server deadline")
        .expect("relay task");
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
