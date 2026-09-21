//! Exact retained EVENT framing and connection-local acknowledgement handling.

use crate::{relay::MAX_WIRE_MESSAGE_BYTES, socket_write::WriterRegistry, status};
use nostr_relay_pool::RelayNotification;
use nostr_sdk::{EventId, RelayMessage};
use radroots_transport::{DeliveryRequest, outcome::DeliveryOutcome};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct ExactEvent {
    id: EventId,
    frame: Arc<str>,
}

impl ExactEvent {
    pub(crate) fn from_request(request: &DeliveryRequest) -> Option<Self> {
        let signed = request.payload().event();
        let raw = signed.raw_json();
        let event = radroots_nostr::event::to_nostr(signed.envelope()).ok()?;
        Some(Self {
            id: event.id,
            frame: frame(raw)?,
        })
    }

    pub(crate) async fn publish(
        &self,
        client: &nostr_sdk::Client,
        writers: &WriterRegistry,
        url: &str,
    ) -> Result<DeliveryOutcome, String> {
        let relay = client.relay(url).await.map_err(|error| error.to_string())?;
        // Subscribe before sending on the same connection; no success from enqueue alone.
        let mut notifications = relay.notifications();
        let writer = writers.get(url).map_err(|error| error.to_string())?;
        writer
            .send(async_wsocket::Message::Text(self.frame.to_string()))
            .await
            .map_err(|error| error.to_string())?;
        loop {
            let notification = notifications
                .recv()
                .await
                .map_err(|_| "relay acknowledgement unavailable".to_owned())?;
            if let Some(outcome) = acknowledgement(self.id, notification) {
                return Ok(outcome);
            }
        }
    }
}

fn frame(raw: &str) -> Option<Arc<str>> {
    // Bound before allocation; preserve whitespace, field order and extensions.
    (raw.len() <= MAX_WIRE_MESSAGE_BYTES - "[\"EVENT\",]".len())
        .then(|| format!("[\"EVENT\",{raw}]").into())
}

fn acknowledgement(id: EventId, notification: RelayNotification) -> Option<DeliveryOutcome> {
    match notification {
        RelayNotification::Message {
            message:
                RelayMessage::Ok {
                    event_id,
                    status: accepted,
                    message,
                },
        } if event_id == id => Some(if accepted {
            DeliveryOutcome::accepted()
        } else {
            status::delivery_failure(&message)
        }),
        RelayNotification::RelayStatus {
            status:
                nostr_sdk::RelayStatus::Disconnected
                | nostr_sdk::RelayStatus::Terminated
                | nostr_sdk::RelayStatus::Banned,
        } => Some(status::delivery_failure("relay disconnected")),
        RelayNotification::Shutdown => Some(status::delivery_failure("relay shutdown")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_bounds_complete_message_bytes_before_allocation() {
        let limit = MAX_WIRE_MESSAGE_BYTES - "[\"EVENT\",]".len();
        let exact = " ".repeat(limit);
        assert_eq!(frame(&exact).unwrap().len(), MAX_WIRE_MESSAGE_BYTES);
        assert!(frame(&(exact + " ")).is_none());
        let unicode = "🌱".repeat(limit / 4 + 1);
        assert!(frame(&unicode).is_none());
        assert_eq!(
            &*frame(" \n{\"extension\":true} \t").unwrap(),
            "[\"EVENT\", \n{\"extension\":true} \t]"
        );
    }

    #[test]
    fn only_matching_ok_can_accept_and_disconnects_remain_failures() {
        let id = EventId::from_byte_array([1; 32]);
        let other = EventId::from_byte_array([2; 32]);
        let ok = |event_id, status, message: &'static str| RelayNotification::Message {
            message: RelayMessage::Ok {
                event_id,
                status,
                message: message.into(),
            },
        };
        assert!(acknowledgement(id, ok(other, true, "")).is_none());
        assert_eq!(
            acknowledgement(id, ok(id, true, "")).unwrap(),
            DeliveryOutcome::accepted()
        );
        assert!(!status::delivery_succeeded(
            &acknowledgement(id, ok(id, false, "blocked: denied")).unwrap()
        ));
        for status in [
            nostr_sdk::RelayStatus::Disconnected,
            nostr_sdk::RelayStatus::Terminated,
            nostr_sdk::RelayStatus::Banned,
        ] {
            assert!(!super::status::delivery_succeeded(
                &acknowledgement(id, RelayNotification::RelayStatus { status }).unwrap()
            ));
        }
        assert!(
            acknowledgement(
                id,
                RelayNotification::RelayStatus {
                    status: nostr_sdk::RelayStatus::Connected
                }
            )
            .is_none()
        );
        assert!(!status::delivery_succeeded(
            &acknowledgement(id, RelayNotification::Shutdown).unwrap()
        ));
    }
}
