use super::*;
use crate::RelayUrlPolicy;
use futures::SinkExt;
use radroots_transport::{
    EventSubscriber, FetchRequest, SubscriptionNext, SubscriptionRequest, Target, TargetSet,
    source::{FetchBounds, FetchSelector, SubscriptionBounds},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

async fn observed_wire(original: String, live: bool) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let endpoint =
        crate::RelayEndpoint::new(&url, RelayUrlPolicy::Local, crate::RelayAccess::ReadOnly)
            .unwrap();
    let profile =
        crate::RelayProfile::explicit(crate::RelayProfileKind::Simulator, [endpoint]).unwrap();
    let transport = crate::test_host::transport(crate::Config::from_profile(profile));
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(tcp).await.unwrap();
        while let Some(Ok(frame)) = socket.next().await {
            match frame {
                Message::Text(text) => {
                    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                    if value[0] == "REQ" {
                        socket
                            .send(Message::Text(
                                format!("[\"EVENT\",{},{}]", value[1], original).into(),
                            ))
                            .await
                            .unwrap();
                        socket
                            .send(Message::Text(
                                serde_json::json!(["EOSE", value[1]]).to_string().into(),
                            ))
                            .await
                            .unwrap();
                    }
                }
                Message::Close(_) => break,
                _ => {
                    if socket.flush().await.is_err() {
                        break;
                    }
                }
            }
        }
    });
    let targets = TargetSet::new(vec![Target::nostr_relay(&url).unwrap()]).unwrap();
    let obtained = if live {
        let request = SubscriptionRequest::new(
            "original-protected-live",
            targets,
            SubscriptionBounds::new(10, crate::host_time::unix_time_ms() + 5000).unwrap(),
        )
        .unwrap();
        let mut subscription = transport.subscribe(request).await.unwrap();
        let SubscriptionNext::Event(observed) = subscription.next().await.unwrap() else {
            panic!("Expected original live event");
        };
        let original = observed.observed().event().raw_json().to_owned();
        subscription.cancel().await.unwrap();
        original
    } else {
        let request = FetchRequest::new(
            "original-protected-caller",
            targets,
            FetchBounds::new(10, crate::host_time::unix_time_ms() + 5000).unwrap(),
        )
        .unwrap();
        let page = transport.fetch(request).await.unwrap();
        assert_eq!(page.events().len(), 1);
        page.events()[0].event().raw_json().to_owned()
    };
    transport.shutdown().await;
    server.await.unwrap();
    obtained
}

#[tokio::test]
async fn ordinary_source_preserves_originals_through_protected_commerce_admission() {
    use radroots_nostr::{
        commerce::{self, RecordOpening},
        key::SecretKey,
        private_ingress::OpeningError,
        signing::LocalSigner,
    };
    use radroots_signing::provider::ProviderCall;
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/source_original.v1.json"))
                .unwrap();
        let base: serde_json::Value =
            serde_json::from_str(fixture["raw_wrap"].as_str().unwrap()).unwrap();
        let scalar = fixture["recipient_scalar"].as_u64().unwrap();
        let signer =
            LocalSigner::with_clock(SecretKey::parse(&format!("{scalar:064x}")).unwrap(), || {
                Ok(1000)
            })
            .unwrap();
        for (live, extension) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut body = base.clone();
            if extension {
                body["future_transport_extension"] = serde_json::json!(true);
            }
            let original = serde_json::to_string_pretty(&body).unwrap();
            let obtained = observed_wire(original.clone(), live).await;
            assert!(
                obtained == original,
                "Ordinary observed SignedEvent must retain exact original object bytes"
            );
            let call = ProviderCall::new(
                signer.public_key(),
                core::num::NonZeroU64::new(2000).unwrap(),
                radroots_signing::request::CancellationSignal::new(),
            );
            let result = commerce::open(&signer, call, &obtained).await;
            if extension {
                assert_eq!(result.unwrap_err(), OpeningError::InvalidInput);
            } else {
                assert!(matches!(result.unwrap(), RecordOpening::Known(_)));
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn ordinary_source_delivers_large_original_profiles_to_authenticated_public_heads() {
    use radroots_nostr::public_ingress::{Interpretation, ObservedHead};
    use radroots_protocol::advertisement::UnixSeconds;
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/public_large.v1.json")).unwrap();
        let raw = fixture["raw"].as_str().unwrap();
        assert_eq!(raw.len(), 65536);
        for live in [false, true] {
            let obtained = observed_wire(raw.into(), live).await;
            assert!(
                obtained == raw,
                "Original large profile must survive ordinary transport observation"
            );
            let mut head = ObservedHead::listing(
                fixture["author"].as_str().unwrap().parse().unwrap(),
                fixture["d"].as_str().unwrap(),
            )
            .unwrap();
            head.observe(&obtained).unwrap();
            let Some(Interpretation::Listing(value)) =
                head.interpret(UnixSeconds::new(150).unwrap())
            else {
                panic!("Expected complete authenticated observed listing");
            };
            let terms = serde_json::to_value(value.listing().terms()).unwrap();
            assert_eq!(
                terms["product"]["description"].as_str().unwrap().len(),
                4000
            );
            assert_eq!(head.raw_json(), Some(raw));
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn live_fetch_retains_original_signed_outer_object_bytes() {
    tokio::time::timeout(Duration::from_secs(10), async {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/source_original.v1.json"))
                .unwrap();
        let value: serde_json::Value =
            serde_json::from_str(fixture["raw_wrap"].as_str().unwrap()).unwrap();
        let original = serde_json::to_string_pretty(&value).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let endpoint =
            crate::RelayEndpoint::new(&url, RelayUrlPolicy::Local, crate::RelayAccess::ReadOnly)
                .unwrap();
        let connector = crate::session_socket::SessionWebsocketTransport::new(
            &[endpoint],
            Duration::from_secs(1),
            Arc::new(crate::test_host::LoopbackConnector),
        );
        let ingress = connector.ingress.clone();
        let client = nostr_sdk::client::Client::builder()
            .websocket_transport(connector)
            .build();
        let server_original = original.clone();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(tcp).await.unwrap();
            while let Some(frame) = socket.next().await {
                let Ok(frame) = frame else { break };
                match frame {
                    Message::Text(text) => {
                        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                        if value[0] == "REQ" {
                            let frame = format!("[\"EVENT\",{},{}]", value[1], server_original);
                            socket.send(Message::Text(frame.into())).await.unwrap();
                            socket
                                .send(Message::Text(
                                    serde_json::json!(["EOSE", value[1]]).to_string().into(),
                                ))
                                .await
                                .unwrap();
                        }
                    }
                    Message::Close(_) => break,
                    _ => {
                        socket.flush().await.unwrap();
                    }
                }
            }
        });
        let source = LiveRelaySourceClient::new(
            client.clone(),
            ingress,
            Arc::new(crate::connections::Connections::new(1)),
        );
        let batches = source
            .fetch(SourceQuery {
                relays: vec![RelayUrl::parse(&url, RelayUrlPolicy::Local).unwrap()],
                selector: FetchSelector::all(),
                until_unix_seconds: None,
                connect_timeout: Duration::from_secs(1),
                deadline: crate::host_time::Instant::now() + Duration::from_secs(5),
                max_connections: 1,
            })
            .await;
        client.shutdown().await;
        server.await.unwrap();
        assert_eq!(batches.len(), 1);
        let RelayFetchResult::Complete(events) = &batches[0].result else {
            panic!("{:?}", batches[0].result)
        };
        assert_eq!(events.len(), 1);
        assert!(
            events[0] == original,
            "The actual source must retain original event bytes before SDK reserialization"
        );
    })
    .await
    .unwrap();
}
