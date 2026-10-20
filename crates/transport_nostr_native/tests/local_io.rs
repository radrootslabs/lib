use futures::{SinkExt, StreamExt};
use radroots_transport::{
    EventSource, EventSubscriber, FetchRequest, SubscriptionNext, SubscriptionRequest, TargetSet,
    capability::Availability,
    outcome::FetchTargetState,
    source::{FetchBounds, SubscriptionBounds, SubscriptionEndReason},
};
use radroots_transport_nostr::{
    Config, RelayAccess, RelayAggregateState, RelayEndpoint, RelayEvidenceState, RelayProfile,
    RelayProfileKind, RelayUrlPolicy,
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
    let transport = radroots_transport_nostr_native::create_transport(config);
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
    use nostr_sdk::prelude::{EventBuilder, FinalizeEvent, Keys, Kind, Timestamp};

    let event = EventBuilder::new(Kind::TextNote, "bounded live event")
        .custom_created_at(Timestamp::from_secs(1_800_000_100))
        .finalize(&Keys::parse(FIXTURE_SECRET_KEY).expect("fixture keys"))
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
    let transport = radroots_transport_nostr_native::create_transport(config);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_disconnects_the_shared_live_pool_and_refuses_new_effects() {
    use nostr_sdk::prelude::{EventBuilder, FinalizeEvent, Keys, Kind};
    use radroots_transport::{
        DeliveryRequest, EventSink,
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        sink::DeliveryPayload,
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_url = format!("ws://{}", listener.local_addr().unwrap());
    let mut server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        while let Some(message) = socket.next().await {
            match message {
                Ok(Message::Text(message)) => {
                    let value: Value = serde_json::from_str(&message).unwrap();
                    if value[0] == "REQ" {
                        socket
                            .send(Message::Text(
                                serde_json::to_string(&("EOSE", &value[1])).unwrap().into(),
                            ))
                            .await
                            .unwrap();
                    }
                }
                Ok(Message::Close(_)) | Err(_) => return listener,
                _ => {}
            }
        }
        listener
    });
    let profile = RelayProfile::explicit(
        RelayProfileKind::Simulator,
        [RelayEndpoint::new(&relay_url, RelayUrlPolicy::Local, RelayAccess::ReadWrite).unwrap()],
    )
    .unwrap();
    let config = Config::from_profile(profile)
        .with_timeouts(1000, 2000, 500)
        .unwrap();
    let relay = config.relays()[0].clone();
    let targets = TargetSet::new(vec![relay.to_target().unwrap()]).unwrap();
    let fetch = FetchRequest::new(
        "shutdown-fetch",
        targets.clone(),
        FetchBounds::new(1, unix_time_ms() + 5000).unwrap(),
    )
    .unwrap();
    let subscription = SubscriptionRequest::new(
        "shutdown-subscription",
        targets.clone(),
        SubscriptionBounds::new(1, unix_time_ms() + 5000).unwrap(),
    )
    .unwrap();
    let event = EventBuilder::new(Kind::TextNote, "synthetic shutdown fixture")
        .finalize(&Keys::parse(FIXTURE_SECRET_KEY).unwrap())
        .unwrap();
    let delivery = DeliveryRequest::new(
        "shutdown-delivery",
        DeliveryPayload::new(radroots_event_codec::decode::signed_event(&event.as_json()).unwrap()),
        targets,
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        unix_time_ms() + 5000,
    )
    .unwrap();
    let transport = radroots_transport_nostr_native::create_transport(config);
    let prepared = transport.prepare_delivery(delivery.clone()).unwrap();
    transport
        .begin_authentication(&relay, "shutdown challenge", 1000, 2000)
        .unwrap();
    let page = tokio::time::timeout(Duration::from_secs(5), transport.fetch(fetch.clone()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        page.target_outcomes()[0].state(),
        FetchTargetState::Complete
    );

    let mut live = transport.subscribe(subscription.clone()).await.unwrap();
    assert!(
        !server.is_finished(),
        "the pool must still own a live socket before shutdown"
    );

    drop(transport.shutdown());
    assert!(EventSource::status(&transport).await.is_ok());
    let clone = transport.clone();
    clone.shutdown().await;
    transport.shutdown().await;
    let joined = tokio::time::timeout(Duration::from_secs(2), &mut server).await;
    if joined.is_err() {
        server.abort();
        let _ = server.await;
        panic!("shutdown did not terminate the live loopback socket");
    }
    let listener = joined.unwrap().unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(2), live.next())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(ended, SubscriptionNext::End(ref end) if end.reason() == SubscriptionEndReason::SourceClosed)
    );
    assert_eq!(
        transport.fetch(fetch).await.unwrap_err(),
        radroots_transport::Error::TransportClosed
    );
    assert!(matches!(
        transport.subscribe(subscription).await,
        Err(radroots_transport::Error::TransportClosed)
    ));
    assert_eq!(
        EventSource::status(&clone).await.unwrap_err(),
        radroots_transport::Error::TransportClosed
    );
    assert_eq!(
        EventSink::status(&clone).await.unwrap_err(),
        radroots_transport::Error::TransportClosed
    );
    assert_eq!(
        transport
            .begin_authentication(&relay, "new challenge", 1000, 2000)
            .unwrap_err(),
        radroots_transport_nostr::Error::TransportClosed
    );
    assert_eq!(
        transport
            .complete_authentication(&relay, "shutdown challenge", None, 1500)
            .await
            .unwrap_err(),
        radroots_transport_nostr::Error::TransportClosed
    );
    assert_eq!(
        transport
            .reject_authentication(&relay, "shutdown challenge")
            .unwrap_err(),
        radroots_transport_nostr::Error::TransportClosed
    );
    for failure in [
        transport
            .execute_prepared_delivery(prepared)
            .await
            .unwrap_err(),
        transport.deliver(delivery.clone()).await.unwrap_err(),
        transport
            .deliver_selected(delivery.clone(), delivery.target_set().clone())
            .await
            .unwrap_err(),
    ] {
        assert_eq!(failure.code(), "transport_closed");
        failure.validate_for_request(&delivery).unwrap();
        assert_eq!(
            failure.partial_evidence().len(),
            delivery.target_set().len()
        );
        assert!(
            failure
                .partial_evidence()
                .iter()
                .all(|row| !row.was_attempted())
        );
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(150), listener.accept())
            .await
            .is_err()
    );
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}
