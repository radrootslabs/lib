use futures::{SinkExt, StreamExt};
use nostr_sdk::{
    client::{Client, ClientNotification},
    prelude::RelayMessage,
    relay::RelayNotification,
};
use std::{num::NonZeroUsize, time::Duration};
use tokio_tungstenite::{accept_async, tungstenite::Message};

const WATCHDOG: Duration = Duration::from_secs(5);

#[tokio::test]
async fn queued_subscription_is_sent_once_after_a_real_socket_reconnect() {
    use nostr_sdk::prelude::{ClientMessage, Filter, Kind, SubscriptionId};
    tokio::time::timeout(WATCHDOG, async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let endpoint = crate::RelayEndpoint::new(
            &url,
            crate::RelayUrlPolicy::Local,
            crate::RelayAccess::ReadOnly,
        )
        .unwrap();
        let connector = crate::session_socket::SessionWebsocketTransport::new(
            &[endpoint],
            Duration::from_secs(1),
            std::sync::Arc::new(crate::test_host::LoopbackConnector),
        );
        let client = Client::builder().websocket_transport(connector).build();
        client.add_relay(&url).await.unwrap();
        let (closed, closure) = tokio::sync::oneshot::channel();
        let (observed, frames) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut first = accept_async(tcp).await.unwrap();
            while let Some(message) = first.next().await {
                match message {
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => first.flush().await.unwrap(),
                }
            }
            closed.send(()).unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut second = accept_async(tcp).await.unwrap();
            let mut requests = Vec::new();
            while let Some(message) = second.next().await {
                match message.unwrap() {
                    Message::Text(text) => {
                        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                        if value[0] == "CLOSE" && value[1] == "sent-barrier" {
                            observed.send(requests).unwrap();
                            break;
                        }
                        requests.push(value);
                    }
                    _ => second.flush().await.unwrap(),
                }
            }
            // Keep the second connection alive until explicit shutdown.
            while let Some(message) = second.next().await {
                if matches!(message, Ok(Message::Close(_)) | Err(_)) {
                    break;
                }
                second.flush().await.unwrap();
            }
        });
        let relay = super::connect_relay(&client, &url, Duration::from_secs(1))
            .await
            .unwrap();
        relay.disconnect();
        closure.await.unwrap();
        let id = SubscriptionId::new("retained-live");
        let filter = Filter::new().kind(Kind::TextNote);
        relay
            .subscribe(filter.clone())
            .with_id(id.clone())
            .await
            .unwrap();
        super::connect_relay(&client, &url, Duration::from_secs(1))
            .await
            .unwrap();
        relay
            .send_msg(ClientMessage::close(SubscriptionId::new("sent-barrier")))
            .wait_until_sent(Duration::from_secs(1))
            .await
            .unwrap();
        let frames = frames.await.unwrap();
        assert_eq!(
            frames,
            vec![
                serde_json::from_str::<serde_json::Value>(
                    &ClientMessage::req(id, filter).as_json()
                )
                .unwrap()
            ]
        );
        client.shutdown().await;
        server.await.unwrap();
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn actual_native_receivers_report_local_repeated_gaps_and_continue() {
    tokio::time::timeout(WATCHDOG, async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let endpoint = crate::RelayEndpoint::new(
            &url,
            crate::RelayUrlPolicy::Local,
            crate::RelayAccess::ReadOnly,
        )
        .unwrap();
        let connector = crate::session_socket::SessionWebsocketTransport::new(
            &[endpoint],
            Duration::from_secs(1),
            std::sync::Arc::new(crate::test_host::LoopbackConnector),
        );
        let client = Client::builder()
            .websocket_transport(connector)
            .notification_channel_size(NonZeroUsize::new(2).unwrap())
            .build();
        client
            .add_relay(&url)
            .notification_channel_size(2)
            .await
            .unwrap();
        let (send, mut receive) = tokio::sync::mpsc::channel::<usize>(1);
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(tcp).await.unwrap();
            while let Some(sequence) = receive.recv().await {
                let frame = serde_json::json!(["NOTICE", sequence.to_string()]).to_string();
                socket.send(Message::Text(frame.into())).await.unwrap();
            }
            socket.close(None).await.unwrap();
        });
        let relay = super::connect_relay(&client, &url, Duration::from_secs(1))
            .await
            .unwrap();
        let mut fast_relay = relay.notifications().with_gaps();
        let mut slow_relay = relay.notifications().with_gaps();
        let mut fast_client = client.notifications().with_gaps();
        let mut slow_client = client.notifications().with_gaps();

        for round in 0..2 {
            for sequence in round * 8..(round + 1) * 8 {
                send.send(sequence).await.unwrap();
                let RelayNotification::Message { message } =
                    fast_relay.next().await.unwrap().unwrap()
                else {
                    panic!("relay message")
                };
                assert!(
                    matches!(*message, RelayMessage::Notice(value) if value == sequence.to_string())
                );
                let ClientNotification::Message { relay_url, message } =
                    fast_client.next().await.unwrap().unwrap()
                else {
                    panic!("client message")
                };
                assert_eq!(relay_url.as_str(), url);
                assert!(
                    matches!(*message, RelayMessage::Notice(value) if value == sequence.to_string())
                );
            }
            assert_eq!(slow_relay.next().await.unwrap().unwrap_err().skipped, 6);
            assert_eq!(slow_client.next().await.unwrap().unwrap_err().skipped, 6);
            for sequence in (round + 1) * 8 - 2..(round + 1) * 8 {
                let RelayNotification::Message { message } =
                    slow_relay.next().await.unwrap().unwrap()
                else {
                    panic!("retained relay message")
                };
                assert!(
                    matches!(*message, RelayMessage::Notice(value) if value == sequence.to_string())
                );
                let ClientNotification::Message { message, .. } =
                    slow_client.next().await.unwrap().unwrap()
                else {
                    panic!("retained client message")
                };
                assert!(
                    matches!(*message, RelayMessage::Notice(value) if value == sequence.to_string())
                );
            }
        }
        drop(send);
        server.await.unwrap();
        client.shutdown().await;
    })
    .await
    .unwrap();
}
