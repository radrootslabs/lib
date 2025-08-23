use core::time::Duration;
use nostr::{event::Event, event::EventBuilder, event::EventId, filter::Filter};
use nostr_sdk::{Client, prelude::*};

use crate::error::NostrUtilsError;

pub async fn nostr_send_event(
    client: &Client,
    event: EventBuilder,
) -> Result<Output<EventId>, NostrUtilsError> {
    Ok(client.send_event_builder(event).await?)
}

pub async fn nostr_fetch_event_by_id(client: Client, id: &str) -> Result<Event, NostrUtilsError> {
    let event_id = EventId::parse(id)?;
    let filter = Filter::new().id(event_id);
    let events = client.fetch_events(filter, Duration::from_secs(10)).await?;
    let event = events
        .first()
        .ok_or_else(|| NostrUtilsError::EventNotFound(event_id.to_hex()))?;
    Ok(event.clone())
}
